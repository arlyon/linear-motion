use crate::clients::{
    linear::LinearClient,
    motion::{AutoScheduled, Label, MotionClient, MotionTask, MotionWorkspace, Status},
};
use crate::config::{AppConfig, SyncRules, SyncSource};
use crate::db::SyncDatabase;
use crate::{Error, Result};
use chrono::Days;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub struct SyncOrchestrator {
    pub database: Arc<SyncDatabase>,
    pub motion_client: Arc<MotionClient>,
}

impl SyncOrchestrator {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        let database = Arc::new(SyncDatabase::new(config.database_path()).await?);

        // Initialize Motion client
        let motion_client = Arc::new(MotionClient::new(config.motion_api_key.clone())?);

        Ok(Self {
            database,
            motion_client,
        })
    }

    pub async fn run_full_sync(&self, config: &AppConfig, force_update: bool) -> Result<()> {
        debug!(
            "starting full sync for {} sources",
            config.sync_sources.len()
        );

        // Test Motion connection first
        match self.motion_client.get_current_user().await {
            Ok(_) => {}
            Err(e) => {
                error!("❌ Failed to connect to Motion: {}", e);
                return Err(e);
            }
        }

        // Process each sync source in parallel
        use futures::future::join_all;

        let sync_futures: Vec<_> = config
            .sync_sources
            .iter()
            .map(|source| {
                let database = Arc::clone(&self.database);
                let motion_client = Arc::clone(&self.motion_client);
                let source = source.clone();
                let global_rules = config.global_sync_rules.clone();

                async move {
                    let result = Self::sync_source(
                        database.clone(),
                        motion_client,
                        &source,
                        &global_rules,
                        force_update,
                    )
                    .await;
                    (source.name.clone(), result)
                }
            })
            .collect();

        let results = join_all(sync_futures).await;

        for (source_name, result) in results {
            match result {
                Ok(count) => debug!("synced {} issues from '{}'", count, source_name),
                Err(e) => {
                    error!("failed to sync from '{}': {}", source_name, e);
                    // Continue with other sources even if one fails
                    self.database
                        .status
                        .update_source_stats(&source_name, false, Some(e.to_string()))
                        .await?;
                }
            }
        }

        // Flush database changes
        self.database.flush().await?;

        // Check for completed Motion tasks and tag corresponding Linear issues
        if let Err(e) = self.sync_completed_tasks(config).await {
            error!("Failed to sync completed tasks: {}", e);
            // Don't fail the whole sync if completion sync fails
        }

        // Flush database changes again after completion sync
        self.database.flush().await?;

        info!("Full sync completed");
        Ok(())
    }

    // write source name to span
    #[tracing::instrument(skip(database, motion_client, source, global_rules), fields(source = source.name.as_str()))]
    async fn sync_source(
        database: Arc<SyncDatabase>,
        motion_client: Arc<MotionClient>,
        source: &SyncSource,
        global_rules: &SyncRules,
        force_update: bool,
    ) -> Result<usize> {
        debug!("syncing source: {}", source.name);

        // Get effective sync rules for this source
        let sync_rules = source.effective_sync_rules(global_rules);

        // Initialize Linear client for this source
        let linear_client = LinearClient::new(source.linear_api_key.clone())?;

        // Test Linear connection
        match linear_client.get_viewer().await {
            Ok(_) => {}
            Err(e) => {
                error!("❌ Failed to connect to Linear: {}", e);
                return Err(e);
            }
        }

        // Get projects to sync from (None means fetch all assigned issues)
        let projects = source.projects.clone();

        match &projects {
            Some(project_ids) => {
                if project_ids.is_empty() {
                    info!(
                        "Empty project list for source '{}' - fetching all assigned issues",
                        source.name
                    );
                } else {
                    info!(
                        "Fetching issues from {} specific projects: {:?}",
                        project_ids.len(),
                        project_ids
                    );
                }
            }
            None => {
                info!(
                    "no projects specified for source '{}' - fetching all assigned issues",
                    source.name
                );
            }
        }

        // Fetch assigned issues from Linear
        let issues = linear_client.get_assigned_issues(projects).await?;
        info!("Found {} assigned issues in Linear", issues.len());
        debug!("found issues: {:?}", issues);

        let mut synced_count = 0;

        // Process each issue
        for issue in &issues {
            // Check if issue already has the completion tag
            if linear_client
                .check_issue_has_label(&issue.id, &sync_rules.completed_linear_tag)
                .await?
            {
                debug!(
                    "Issue {} already has completion tag, skipping",
                    issue.identifier
                );
                continue;
            }

            // Check if we already have a mapping for this issue
            let existing_mapping = database
                .mappings
                .get_mapping_by_linear_id(&source.name, &issue.id)
                .await?;

            let _mapping = match existing_mapping {
                Some(mapping) => {
                    debug!(
                        "Issue {} already tracked with status: {:?}",
                        issue.identifier, mapping.status
                    );
                    // If it's already synced successfully, check if it needs updating
                    if matches!(mapping.status, crate::db::MappingStatus::Synced) {
                        // Check if the issue has been updated since last sync or if force update is enabled
                        if force_update || Self::issue_needs_update(&mapping, issue)? {
                            if force_update {
                                debug!(
                                    "force update enabled, updating Motion task for {}",
                                    issue.identifier
                                );
                            } else {
                                debug!(
                                    "issue {} has changes, updating Motion task",
                                    issue.identifier
                                );
                            }
                            // Continue with update process
                        } else {
                            debug!("issue {} unchanged, skipping", issue.identifier);
                            continue;
                        }
                    }
                    // If it failed or is pending, we'll retry
                    mapping
                }
                None => {
                    debug!(
                        "Processing new issue: {} - {}",
                        issue.identifier, issue.title
                    );

                    // Create pending mapping immediately when we fetch the issue
                    database
                        .mappings
                        .create_pending_mapping(&source.name, issue)
                        .await?
                }
            };

            // Create status entry for tracking
            let status_entry = database
                .status
                .create_status_entry(source.name.clone(), issue.id.clone())
                .await?;

            // Determine if this is an update or create operation
            let is_update = _mapping.motion_task_id.is_some();

            if is_update {
                // Update existing Motion task
                let motion_task_id = _mapping.motion_task_id.as_ref().unwrap();
                match Self::update_motion_task_from_issue(
                    &motion_client,
                    motion_task_id,
                    issue,
                    &sync_rules,
                    &source.name,
                )
                .await
                {
                    Ok(_) => {
                        // Update the stored Linear issue data in the mapping
                        database
                            .mappings
                            .update_issue_data(&source.name, &issue.id, issue)
                            .await?;

                        // Update status as completed
                        database
                            .status
                            .mark_completed(&status_entry.id, motion_task_id.clone())
                            .await?;

                        synced_count += 1;
                        info!(
                            "✅ Updated: {} → Motion task {}",
                            issue.identifier, motion_task_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "❌ Failed to update Motion task for {}: {}",
                            issue.identifier, e
                        );

                        database
                            .status
                            .mark_failed(&status_entry.id, e.to_string())
                            .await?;
                    }
                }
            } else {
                // Create new Motion task
                match Self::create_motion_task_from_issue(
                    &motion_client,
                    issue,
                    &sync_rules,
                    &source.name,
                )
                .await
                {
                    Ok(motion_task) => {
                        let motion_task_id = motion_task.id.clone().unwrap_or_default();

                        // Mark mapping as synced with Motion task ID
                        database
                            .mappings
                            .mark_synced(&source.name, &issue.id, motion_task_id.clone())
                            .await?;

                        // Update status as completed
                        database
                            .status
                            .mark_completed(&status_entry.id, motion_task_id)
                            .await?;

                        synced_count += 1;
                        info!("✅ Created: {} → Motion task", issue.identifier);
                    }
                    Err(e) => {
                        error!(
                            "❌ Failed to create Motion task for {}: {}",
                            issue.identifier, e
                        );

                        // Mark mapping as failed
                        database
                            .mappings
                            .mark_failed(&source.name, &issue.id, e.to_string())
                            .await?;

                        database
                            .status
                            .mark_failed(&status_entry.id, e.to_string())
                            .await?;
                    }
                }
            }
        }

        // Update source statistics
        database
            .status
            .update_source_stats(&source.name, true, None)
            .await?;

        Ok(synced_count)
    }

    /// Check for completed tasks in Motion and tag corresponding Linear issues
    #[tracing::instrument(skip(self, config))]
    pub async fn sync_completed_tasks(&self, config: &AppConfig) -> Result<()> {
        info!("Checking for completed Motion tasks to tag in Linear");

        // Get Motion workspaces
        let workspaces = self.motion_client.list_workspaces().await?;

        for workspace in &workspaces {
            info!("Checking workspace: {}", workspace.name);

            // Get completed tasks from this workspace
            let completed_tasks = self
                .motion_client
                .list_completed_tasks(&workspace.id)
                .await?;

            debug!(
                "Found {} completed tasks in workspace {}",
                completed_tasks.len(),
                workspace.name
            );

            for task in &completed_tasks {
                // Skip tasks without IDs or without the linear-sync label
                let task_id = match &task.id {
                    Some(id) => id,
                    None => continue,
                };

                // Check if this task has the linear-sync label
                let has_linear_label = task
                    .labels
                    .as_ref()
                    .map(|labels| labels.iter().any(|l| l.name == "linear-sync"))
                    .unwrap_or(false);

                if !has_linear_label {
                    continue;
                }

                // Find the corresponding Linear issue in our database
                if let Some(mapping) = self
                    .database
                    .mappings
                    .get_mapping_by_motion_id(task_id)
                    .await?
                {
                    // Check if this issue is already tagged to avoid re-tagging
                    if let Some(sync_source_config) = config
                        .sync_sources
                        .iter()
                        .find(|s| s.name == mapping.sync_source)
                    {
                        let linear_client = crate::clients::linear::LinearClient::new(
                            sync_source_config.linear_api_key.clone(),
                        )?;

                        let completion_tag = &config.global_sync_rules.completed_linear_tag;

                        // Check if issue already has the completion tag
                        if linear_client
                            .check_issue_has_label(&mapping.linear_issue_id, completion_tag)
                            .await?
                        {
                            debug!(
                                "Linear issue {} already has completion tag, skipping",
                                mapping.linear_issue_id
                            );
                            continue;
                        }

                        info!(
                            "Motion task {} completed, tagging Linear issue {}",
                            task_id, mapping.linear_issue_id
                        );

                        // Add the completion tag to the Linear issue
                        match linear_client
                            .add_label_to_issue(&mapping.linear_issue_id, completion_tag)
                            .await
                        {
                            Ok(()) => {
                                info!(
                                    "✅ Successfully tagged Linear issue {} with '{}'",
                                    mapping.linear_issue_id, completion_tag
                                );

                                // Remove the mapping from the database as per PRD
                                self.database
                                    .mappings
                                    .remove_mapping(&mapping.sync_source, &mapping.linear_issue_id)
                                    .await?;

                                info!(
                                    "🗑️  Removed mapping for completed task: {} -> {}",
                                    mapping.linear_issue_id, task_id
                                );
                            }
                            Err(e) => {
                                error!(
                                    "❌ Failed to tag Linear issue {}: {}",
                                    mapping.linear_issue_id, e
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Format the description with a link back to the Linear issue
    fn format_description_with_link(
        issue: &crate::clients::linear::LinearIssue,
        sync_source_name: &str,
    ) -> Option<String> {
        let base_description = issue.description.as_ref().cloned().unwrap_or_default();
        let linear_link = format!(
            "https://linear.app/{}/issue/{}",
            sync_source_name, issue.identifier
        );

        if base_description.is_empty() {
            Some(format!("Linear: {}", linear_link))
        } else {
            Some(format!("{}\n\nLinear: {}", base_description, linear_link))
        }
    }

    /// Check if a Linear issue has changes that require updating the Motion task
    fn issue_needs_update(
        mapping: &crate::db::mapping::TaskMapping,
        current_issue: &crate::clients::linear::LinearIssue,
    ) -> Result<bool> {
        // Parse the stored issue data from the mapping
        let stored_issue: crate::clients::linear::LinearIssue =
            serde_json::from_value(mapping.linear_issue_data.clone())
                .map_err(|e| Error::Json(e))?;

        // Compare key fields that affect Motion tasks
        let needs_update = stored_issue.title != current_issue.title
            || stored_issue.description != current_issue.description
            || stored_issue.estimate != current_issue.estimate
            || stored_issue.priority != current_issue.priority
            || stored_issue.due_date != current_issue.due_date
            || stored_issue.updated_at != current_issue.updated_at;

        if needs_update {
            debug!(
                "Issue {} changes detected: title={}, description={}, estimate={}, priority={}, due_date={}, updated_at={}",
                current_issue.identifier,
                stored_issue.title != current_issue.title,
                stored_issue.description != current_issue.description,
                stored_issue.estimate != current_issue.estimate,
                stored_issue.priority != current_issue.priority,
                stored_issue.due_date != current_issue.due_date,
                stored_issue.updated_at != current_issue.updated_at,
            );
        }

        Ok(needs_update)
    }

    async fn update_motion_task_from_issue(
        motion_client: &MotionClient,
        motion_task_id: &str,
        issue: &crate::clients::linear::LinearIssue,
        sync_rules: &SyncRules,
        sync_source_name: &str,
    ) -> Result<MotionTask> {
        // Calculate duration from Linear estimate
        let duration_mins = if let Some(estimate) = issue.estimate {
            sync_rules
                .time_estimate_strategy
                .convert_estimate_by_value(estimate)
                .unwrap_or(sync_rules.default_task_duration_mins)
        } else {
            sync_rules.default_task_duration_mins
        };

        // Convert priority (Linear uses 0-4, Motion uses ASAP/HIGH/MEDIUM/LOW)
        let priority = match issue.priority {
            Some(1) => Some("ASAP".to_string()),
            Some(2) => Some("HIGH".to_string()),
            Some(3) => Some("MEDIUM".to_string()),
            Some(4) | Some(_) => Some("LOW".to_string()),
            None => Some("MEDIUM".to_string()),
        };

        // Convert due_date string to DateTime if present
        let due_date = issue.due_date.as_ref().and_then(|date_str| {
            // Linear sends dates as YYYY-MM-DD, convert to DateTime
            chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .map(|date| date.and_hms_opt(23, 59, 59).unwrap().and_utc())
        });

        // Create Motion task object with the updates
        let motion_task = MotionTask {
            id: Some(motion_task_id.to_string()),
            name: format!("[{}] {}", issue.identifier, issue.title),
            description: Self::format_description_with_link(issue, sync_source_name),
            duration: Some(crate::clients::motion::TaskDuration::from_minutes(
                duration_mins,
            )),
            priority,
            due_date,
            labels: Some(vec![Label {
                name: "linear-sync".to_string(),
            }]),
            ..Default::default()
        };

        // Update the task in Motion
        let updated_task = motion_client
            .update_task(motion_task_id, &motion_task)
            .await?;
        debug!(
            "Updated Motion task: {} ({})",
            updated_task.name, motion_task_id
        );

        Ok(updated_task)
    }

    async fn create_motion_task_from_issue(
        motion_client: &MotionClient,
        issue: &crate::clients::linear::LinearIssue,
        sync_rules: &SyncRules,
        sync_source_name: &str,
    ) -> Result<MotionTask> {
        // Get Motion workspaces to determine where to create the task
        let workspaces = motion_client.list_workspaces().await?;

        // Find "My Private Workspace" or use the first available workspace
        let workspace = workspaces
            .iter()
            .find(|w| w.name == "My Private Workspace")
            .or_else(|| workspaces.first())
            .ok_or_else(|| Error::MotionApi {
                message: "No Motion workspaces found".to_string(),
            })?;

        // Ensure the "linear-sync" label exists in the workspace
        // self.motion_client
        //     .ensure_label_exists(&workspace.id, "linear-sync")
        //     .await?;

        // Calculate duration from Linear estimate
        let duration_mins = if let Some(estimate) = issue.estimate {
            sync_rules
                .time_estimate_strategy
                .convert_estimate_by_value(estimate)
                .unwrap_or(sync_rules.default_task_duration_mins)
        } else {
            sync_rules.default_task_duration_mins
        };

        // Convert priority (Linear uses 0-4, Motion uses ASAP/HIGH/MEDIUM/LOW)
        let priority = match issue.priority {
            Some(1) => Some("ASAP".to_string()),
            Some(2) => Some("HIGH".to_string()),
            Some(3) => Some("MEDIUM".to_string()),
            Some(4) | Some(_) => Some("LOW".to_string()),
            None => Some("MEDIUM".to_string()),
        };

        // Convert due_date string to DateTime if present
        let due_date = issue
            .due_date
            .as_ref()
            .and_then(|date_str| {
                // Linear sends dates as YYYY-MM-DD, convert to DateTime
                chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                    .ok()
                    .map(|date| date.and_hms_opt(23, 59, 59).unwrap().and_utc())
            })
            .unwrap_or(
                chrono::Utc::now()
                    .checked_add_days(Days::new(1))
                    .expect("valid"),
            );

        // Create Motion task
        let motion_task = MotionTask {
            id: None,
            name: format!("[{}] {}", issue.identifier, issue.title),
            description: Self::format_description_with_link(issue, sync_source_name),
            duration: Some(crate::clients::motion::TaskDuration::from_minutes(
                duration_mins,
            )),
            workspace: Some(MotionWorkspace {
                id: workspace.id.clone(),
                name: workspace.name.clone(),
                team_id: None,
                workspace_type: "INDIVIDUAL".to_string(),
            }),
            auto_scheduled: Some(AutoScheduled {
                deadline_type: "SOFT".to_string(),
                schedule: "Work hours".to_string(),
                start_date: Some(chrono::Utc::now()),
            }),
            status: Some(Status {
                name: "Todo".to_string(),
                ..Default::default()
            }),
            priority,
            due_date: Some(due_date),

            completed: Some(false),
            labels: Some(vec![Label {
                name: "linear-sync".to_string(),
            }]),
            ..Default::default()
        };

        // Create the task in Motion
        let created_task = motion_client.create_task(&motion_task).await?;
        info!(
            "Created Motion task: {} ({})",
            created_task.name,
            created_task.id.as_ref().unwrap_or(&"unknown".to_string())
        );

        Ok(created_task)
    }
}
