use crate::clients::{
    linear::LinearClient,
    motion::{MotionClient, MotionTask},
};
use crate::config::{AppConfig, SyncRules, SyncSource};
use crate::db::{SyncDatabase, TaskMapping};
use crate::{Error, Result};
use tracing::{debug, error, info, warn};

pub struct SyncOrchestrator {
    pub database: SyncDatabase,
    pub motion_client: MotionClient,
}

impl SyncOrchestrator {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        let database = SyncDatabase::new(config.database_path()).await?;

        // Initialize Motion client
        let motion_client = MotionClient::new(config.motion_api_key.clone())?;

        Ok(Self {
            database,
            motion_client,
        })
    }

    pub async fn run_full_sync(&mut self, config: &AppConfig) -> Result<()> {
        info!(
            "Starting full sync for {} sources",
            config.sync_sources.len()
        );

        // Test Motion connection first
        match self.motion_client.get_current_user().await {
            Ok(user) => info!("✅ Connected to Motion as: {}", user.name),
            Err(e) => {
                error!("❌ Failed to connect to Motion: {}", e);
                return Err(e);
            }
        }

        // Process each sync source
        for source in &config.sync_sources {
            match self.sync_source(source, &config.global_sync_rules).await {
                Ok(count) => info!("✅ Synced {} issues from source '{}'", count, source.name),
                Err(e) => {
                    error!("❌ Failed to sync source '{}': {}", source.name, e);
                    // Continue with other sources even if one fails
                    self.database
                        .status
                        .update_source_stats(&source.name, false, Some(e.to_string()))
                        .await?;
                }
            }
        }

        // Flush database changes
        self.database.flush().await?;

        info!("Full sync completed");
        Ok(())
    }

    async fn sync_source(
        &mut self,
        source: &SyncSource,
        global_rules: &SyncRules,
    ) -> Result<usize> {
        info!("Syncing source: {}", source.name);

        // Get effective sync rules for this source
        let sync_rules = source.effective_sync_rules(global_rules);

        // Initialize Linear client for this source
        let linear_client = LinearClient::new(source.linear_api_key.clone())?;

        // Test Linear connection
        match linear_client.get_viewer().await {
            Ok(user) => info!("✅ Connected to Linear as: {}", user.name),
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
                    info!("Empty project list for source '{}' - fetching all assigned issues", source.name);
                } else {
                    info!("Fetching issues from {} specific projects: {:?}", project_ids.len(), project_ids);
                }
            }
            None => {
                info!("No projects specified for source '{}' - fetching all assigned issues", source.name);
            }
        }

        // Fetch assigned issues from Linear
        let issues = linear_client.get_assigned_issues(projects).await?;
        info!("Found {} assigned issues in Linear", issues.len());

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
            if let Some(_mapping) = self
                .database
                .mappings
                .get_mapping_by_linear_id(&source.name, &issue.id)
                .await?
            {
                debug!("Issue {} already synced, skipping", issue.identifier);
                continue;
            }

            debug!(
                "Processing new issue: {} - {}",
                issue.identifier, issue.title
            );

            // Create status entry for tracking
            let status_entry = self
                .database
                .status
                .create_status_entry(source.name.clone(), issue.id.clone())
                .await?;

            // Convert Linear issue to Motion task
            match self.create_motion_task_from_issue(issue, &sync_rules).await {
                Ok(motion_task) => {
                    let motion_task_id = motion_task.id.clone().unwrap_or_default();

                    // Store the mapping
                    let mapping = TaskMapping::new(
                        issue.id.clone(),
                        motion_task_id.clone(),
                        source.name.clone(),
                    );

                    self.database.mappings.store_mapping(mapping).await?;

                    // Update status as completed
                    self.database
                        .status
                        .mark_completed(&status_entry.id, motion_task_id)
                        .await?;

                    synced_count += 1;
                    info!("✅ Synced: {} → Motion task", issue.identifier);
                }
                Err(e) => {
                    error!(
                        "❌ Failed to create Motion task for {}: {}",
                        issue.identifier, e
                    );
                    self.database
                        .status
                        .mark_failed(&status_entry.id, e.to_string())
                        .await?;
                }
            }
        }

        // Update source statistics
        self.database
            .status
            .update_source_stats(&source.name, true, None)
            .await?;

        Ok(synced_count)
    }

    async fn create_motion_task_from_issue(
        &self,
        issue: &crate::clients::linear::LinearIssue,
        sync_rules: &SyncRules,
    ) -> Result<MotionTask> {
        // Get Motion workspaces to determine where to create the task
        let workspaces = self.motion_client.list_workspaces().await?;

        // For now, use the first workspace (in a real implementation, you might want to map teams to workspaces)
        let workspace = workspaces.first().ok_or_else(|| Error::MotionApi {
            message: "No Motion workspaces found".to_string(),
        })?;

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

        // Create Motion task
        let motion_task = MotionTask {
            id: None,
            name: format!("[{}] {}", issue.identifier, issue.title),
            description: issue.description.clone(),
            workspace_id: workspace.id.clone(),
            assignee_id: None, // We'll assign to the current user
            project_id: None,  // Could map Linear projects to Motion projects
            priority,
            due_date,
            duration: Some(duration_mins.to_string()),
            status: None,
            completed: Some(false),
            labels: Some(vec!["linear-sync".to_string()]),
            created_time: None,
            updated_time: None,
        };

        // Create the task in Motion
        let created_task = self.motion_client.create_task(&motion_task).await?;
        info!(
            "Created Motion task: {} ({})",
            created_task.name,
            created_task.id.as_ref().unwrap_or(&"unknown".to_string())
        );

        Ok(created_task)
    }
}
