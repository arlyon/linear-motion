use crate::{Error, Result};
use chrono::{DateTime, Utc};
use governor::{Quota, RateLimiter};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub enum TaskDuration {
    None,
    Reminder,
    Minutes(u32),
}

impl Serialize for TaskDuration {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            TaskDuration::None => serializer.serialize_str("NONE"),
            TaskDuration::Reminder => serializer.serialize_str("REMINDER"),
            TaskDuration::Minutes(m) => serializer.serialize_u32(*m),
        }
    }
}

impl<'de> Deserialize<'de> for TaskDuration {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::String(s) => match s.as_str() {
                "NONE" => Ok(TaskDuration::None),
                "REMINDER" => Ok(TaskDuration::Reminder),
                _ => Err(D::Error::custom(format!("Invalid duration string: {}", s))),
            },
            serde_json::Value::Number(n) => {
                if let Some(minutes) = n.as_u64() {
                    Ok(TaskDuration::Minutes(minutes as u32))
                } else {
                    Err(D::Error::custom("Invalid duration number"))
                }
            }
            _ => Err(D::Error::custom("Duration must be a string or number")),
        }
    }
}

impl TaskDuration {
    pub fn from_minutes(minutes: u32) -> Self {
        if minutes == 0 {
            TaskDuration::None
        } else {
            TaskDuration::Minutes(minutes)
        }
    }
}

impl From<u32> for TaskDuration {
    fn from(minutes: u32) -> Self {
        TaskDuration::from_minutes(minutes)
    }
}

#[derive(Debug)]
pub struct MotionClient {
    client: ClientWithMiddleware,
    api_key: String,
    base_url: String,
    rate_limiter: RateLimiter<
        governor::state::direct::NotKeyed,
        governor::state::InMemoryState,
        governor::clock::DefaultClock,
    >,
    cached_workspaces: Arc<Mutex<Option<Vec<MotionWorkspace>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Status {
    pub name: String,
    #[serde(rename = "isDefaultStatus")]
    pub is_default_status: bool,
    #[serde(rename = "isResolvedStatus")]
    pub is_resolved_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Label {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoScheduled {
    #[serde(rename = "startDate")]
    pub start_date: Option<DateTime<Utc>>,
    /// HARD SOFT NONE
    #[serde(rename = "deadlineType")]
    pub deadline_type: String,
    /// Work Hours
    pub schedule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MotionTask {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub assignee_id: Option<String>,
    pub project: Option<MotionProject>,
    pub priority: Option<String>, // "ASAP", "HIGH", "MEDIUM", "LOW"
    #[serde(rename = "dueDate")]
    pub due_date: Option<DateTime<Utc>>,
    pub duration: Option<TaskDuration>,
    pub status: Option<Status>,
    pub completed: Option<bool>,
    pub labels: Option<Vec<Label>>,
    #[serde(rename = "createdTime")]
    pub created_time: Option<DateTime<Utc>>,
    #[serde(rename = "updatedTime")]
    pub updated_time: Option<DateTime<Utc>>,
    // Additional fields from the API response that we don't currently use
    pub creator: Option<MotionUser>,
    pub workspace: Option<MotionWorkspace>,
    pub assignees: Option<Vec<MotionUser>>,
    #[serde(rename = "autoScheduled")]
    pub auto_scheduled: Option<AutoScheduled>,
    pub chunks: Option<Vec<TaskChunk>>,
    #[serde(rename = "parentRecurringTaskId")]
    pub parent_recurring_task_id: Option<String>,
    #[serde(rename = "deadlineType")]
    pub deadline_type: Option<String>,
    #[serde(rename = "startOn")]
    pub start_on: Option<String>,
    #[serde(rename = "scheduledStart")]
    pub scheduled_start: Option<DateTime<Utc>>,
    #[serde(rename = "scheduledEnd")]
    pub scheduled_end: Option<DateTime<Utc>>,
    #[serde(rename = "schedulingIssue")]
    pub scheduling_issue: Option<bool>,
    #[serde(rename = "lastInteractedTime")]
    pub last_interacted_time: Option<DateTime<Utc>>,
    #[serde(rename = "completedTime")]
    pub completed_time: Option<DateTime<Utc>>,
    #[serde(rename = "customFieldValues")]
    pub custom_field_values: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionWorkspace {
    pub id: String,
    pub name: String,
    #[serde(rename = "teamId")]
    pub team_id: Option<String>,
    #[serde(rename = "type")]
    pub workspace_type: String, // "team" or "individual"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionUser {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionProject {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "workspaceId")]
    pub workspace_id: String,
    #[serde(rename = "createdTime")]
    pub created_time: Option<DateTime<Utc>>,
    #[serde(rename = "updatedTime")]
    pub updated_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskChunk {
    pub id: String,
    pub duration: u32,
    #[serde(rename = "scheduledStart")]
    pub scheduled_start: Option<DateTime<Utc>>,
    #[serde(rename = "scheduledEnd")]
    pub scheduled_end: Option<DateTime<Utc>>,
    #[serde(rename = "completedTime")]
    pub completed_time: Option<DateTime<Utc>>,
    #[serde(rename = "isFixed")]
    pub is_fixed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionLabel {
    pub id: String,
    pub name: String,
    #[serde(rename = "colorHex")]
    pub color_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MotionResponse<T> {
    #[serde(flatten)]
    data: Option<T>,
    meta: Option<MotionMeta>,
}

#[derive(Debug, Deserialize)]
struct MotionMeta {
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
    #[serde(rename = "pageSize")]
    page_size: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct TaskListResponse {
    tasks: Vec<MotionTask>,
    meta: MotionMeta,
}

#[derive(Debug, Deserialize)]
struct WorkspaceListResponse {
    workspaces: Vec<MotionWorkspace>,
    meta: Option<MotionMeta>,
}

#[derive(Debug, Deserialize)]
struct LabelListResponse {
    labels: Vec<MotionLabel>,
    meta: Option<MotionMeta>,
}

impl MotionClient {
    pub fn new(api_key: String) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_str(&api_key)?);

        let base_client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        // Configure exponential backoff retry policy
        // Start with 10 seconds as requested, with exponential backoff
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(
                std::time::Duration::from_secs(10), // Start with 10 seconds as requested
                std::time::Duration::from_secs(60), // Cap at 60 seconds
            )
            .build_with_max_retries(3);

        // Create client with retry middleware specifically for 429 errors
        let client = ClientBuilder::new(base_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        // Motion rate limit: 12 requests per minute for individuals, 120 for teams
        // We'll use exactly 12 requests per minute for individuals
        let quota = Quota::per_minute(NonZeroU32::new(12).unwrap());
        let rate_limiter = RateLimiter::direct(quota);

        Ok(Self {
            client,
            api_key,
            base_url: "https://api.usemotion.com/v1".to_string(),
            rate_limiter,
            cached_workspaces: Arc::new(Mutex::new(None)),
        })
    }

    async fn rate_limit(&self) -> Result<()> {
        self.rate_limiter.until_ready().await;
        Ok(())
    }

    async fn make_request<T: for<'de> Deserialize<'de>>(&self, endpoint: &str) -> Result<T> {
        self.rate_limit().await?;

        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));
        debug!("Making Motion API request: GET {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::MotionApi {
                message: format!("Request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            error!("Motion API error: {} - {}", status, text);
            return Err(Error::MotionApi {
                message: format!("HTTP {}: {}", status, text),
            });
        }

        let text = response.text().await?;
        debug!("Motion API response: {}", text);

        let response_data: T = {
            let deserializer = &mut serde_json::Deserializer::from_str(&text);
            serde_path_to_error::deserialize(deserializer)?
        };
        Ok(response_data)
    }

    async fn make_post_request<T: for<'de> Deserialize<'de>, B: Serialize + std::fmt::Debug>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        self.rate_limit().await?;

        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));
        debug!(
            "Making Motion API request: POST {} {:?}",
            url,
            serde_json::to_string(&body)
        );

        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| Error::MotionApi {
                message: format!("Request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            error!("Motion API error: {} - {}", status, text);
            return Err(Error::MotionApi {
                message: format!("HTTP {}: {}", status, text),
            });
        }

        let text = response.text().await?;
        debug!("Motion API response: {}", text);

        let response_data: T = {
            let deserializer = &mut serde_json::Deserializer::from_str(&text);
            match serde_path_to_error::deserialize(deserializer) {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to parse Motion POST response: {}", e);
                    error!("Response body was: {}", text);
                    return Err(Error::Json(e.into_inner()));
                }
            }
        };

        Ok(response_data)
    }

    async fn make_patch_request<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        self.rate_limit().await?;

        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));
        debug!("Making Motion API request: PATCH {}", url);

        let response = self
            .client
            .patch(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| Error::MotionApi {
                message: format!("Request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            error!("Motion API error: {} - {}", status, text);
            return Err(Error::MotionApi {
                message: format!("HTTP {}: {}", status, text),
            });
        }

        let response_data: T = response.json().await?;
        Ok(response_data)
    }

    pub async fn get_current_user(&self) -> Result<MotionUser> {
        let user: MotionUser = self.make_request("users/me").await?;
        debug!("connected to Motion as: {} ({})", user.name, user.email);
        Ok(user)
    }

    pub async fn list_workspaces(&self) -> Result<Vec<MotionWorkspace>> {
        // Check cache first
        {
            let cache = self.cached_workspaces.lock().unwrap();
            if let Some(ref workspaces) = *cache {
                debug!("Using cached Motion workspaces ({})", workspaces.len());
                return Ok(workspaces.clone());
            }
        }

        // Cache miss - fetch from API
        let response: WorkspaceListResponse = self.make_request("workspaces").await?;
        info!("Found {} Motion workspaces", response.workspaces.len());

        // Store in cache
        {
            let mut cache = self.cached_workspaces.lock().unwrap();
            *cache = Some(response.workspaces.clone());
        }

        Ok(response.workspaces)
    }

    pub async fn create_task(&self, task: &MotionTask) -> Result<MotionTask> {
        debug!("Creating Motion task: {}", task.name);

        #[derive(Serialize, Debug)]
        struct CreateTaskRequest {
            name: String,
            #[serde(rename = "workspaceId")]
            workspace_id: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<String>,
            #[serde(rename = "assigneeId", skip_serializing_if = "Option::is_none")]
            assignee_id: Option<String>,
            #[serde(rename = "projectId", skip_serializing_if = "Option::is_none")]
            project_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            priority: Option<String>,
            #[serde(rename = "dueDate", skip_serializing_if = "Option::is_none")]
            due_date: Option<DateTime<Utc>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            duration: Option<TaskDuration>,
            #[serde(skip_serializing_if = "Option::is_none")]
            labels: Option<Vec<String>>,
            // Auto-scheduling fields (sent as individual fields, not nested object)
            #[serde(rename = "autoScheduled", skip_serializing_if = "Option::is_none")]
            auto_scheduled: Option<serde_json::Value>,
        }

        // Convert auto_scheduled to the correct format for Motion API
        let auto_scheduled_value = task.auto_scheduled.as_ref().map(|auto_sched| {
            serde_json::json!({
                "startDate": auto_sched.start_date,
                "deadlineType": auto_sched.deadline_type,
                "schedule": auto_sched.schedule
            })
        });

        let request = CreateTaskRequest {
            name: task.name.clone(),
            workspace_id: task
                .workspace
                .as_ref()
                .map(|w| w.id.to_owned())
                .expect("workspace"),
            description: task.description.clone(),
            assignee_id: task.assignee_id.clone(),
            project_id: task.project.as_ref().map(|p| p.id.clone()),
            priority: task.priority.clone(),
            due_date: task.due_date,
            duration: task.duration.clone(),
            labels: task
                .labels
                .clone()
                .map(|l| l.into_iter().map(|l| l.name).collect()),
            auto_scheduled: auto_scheduled_value,
        };

        let created_task: MotionTask = self.make_post_request("tasks", &request).await?;
        info!(
            "Created Motion task: {} (ID: {})",
            created_task.name,
            created_task.id.as_ref().unwrap_or(&"unknown".to_string())
        );
        Ok(created_task)
    }

    pub async fn update_task(&self, task_id: &str, task: &MotionTask) -> Result<MotionTask> {
        debug!("Updating Motion task: {}", task_id);

        #[derive(Serialize)]
        struct UpdateTaskRequest {
            #[serde(skip_serializing_if = "Option::is_none")]
            name: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            priority: Option<String>,
            #[serde(rename = "dueDate", skip_serializing_if = "Option::is_none")]
            due_date: Option<DateTime<Utc>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            duration: Option<TaskDuration>,
            #[serde(skip_serializing_if = "Option::is_none")]
            status: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            labels: Option<Vec<String>>,
        }

        let request = UpdateTaskRequest {
            name: Some(task.name.clone()),
            description: task.description.clone(),
            priority: task.priority.clone(),
            due_date: task.due_date,
            duration: task.duration.clone(),
            status: task.status.as_ref().map(|s| s.name.clone()),
            labels: task
                .labels
                .clone()
                .map(|l| l.into_iter().map(|l| l.name).collect()),
        };

        let endpoint = format!("tasks/{}", task_id);
        let updated_task: MotionTask = self.make_patch_request(&endpoint, &request).await?;
        debug!("Updated Motion task: {}", task_id);
        Ok(updated_task)
    }

    pub async fn get_task(&self, task_id: &str) -> Result<MotionTask> {
        let endpoint = format!("tasks/{}", task_id);
        let task: MotionTask = self.make_request(&endpoint).await?;
        Ok(task)
    }

    pub async fn list_completed_tasks(&self, workspace_id: &str) -> Result<Vec<MotionTask>> {
        let endpoint = format!("tasks?workspaceId={}&includeAllStatuses=true", workspace_id);
        let response: TaskListResponse = self.make_request(&endpoint).await?;

        let completed_tasks: Vec<MotionTask> = response
            .tasks
            .into_iter()
            .filter(|task| task.completed.unwrap_or(false))
            .collect();

        debug!(
            "Found {} completed tasks in workspace {}",
            completed_tasks.len(),
            workspace_id
        );
        Ok(completed_tasks)
    }

    pub async fn mark_task_completed(&self, task_id: &str) -> Result<MotionTask> {
        debug!("Marking Motion task as completed: {}", task_id);

        #[derive(Serialize)]
        struct CompleteTaskRequest {
            status: String,
        }

        let request = CompleteTaskRequest {
            status: "Completed".to_string(), // Motion's default completed status name
        };

        let endpoint = format!("tasks/{}", task_id);
        let updated_task: MotionTask = self.make_patch_request(&endpoint, &request).await?;
        info!("Marked Motion task as completed: {}", task_id);
        Ok(updated_task)
    }

    pub async fn delete_task(&self, task_id: &str) -> Result<()> {
        debug!("Deleting Motion task: {}", task_id);
        
        self.rate_limit().await?;

        let url = format!("{}/tasks/{}", self.base_url, task_id);
        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| Error::MotionApi {
                message: format!("Request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            error!("Motion API error: {} - {}", status, text);
            return Err(Error::MotionApi {
                message: format!("HTTP {}: {}", status, text),
            });
        }

        info!("Successfully deleted Motion task: {}", task_id);
        Ok(())
    }

    pub async fn list_labels(&self, workspace_id: &str) -> Result<Vec<MotionLabel>> {
        let endpoint = format!("labels?workspaceId={}", workspace_id);
        let response: LabelListResponse = self.make_request(&endpoint).await?;
        debug!(
            "Found {} labels in workspace {}",
            response.labels.len(),
            workspace_id
        );
        Ok(response.labels)
    }

    pub async fn create_label(
        &self,
        workspace_id: &str,
        name: &str,
        color_hex: Option<&str>,
    ) -> Result<MotionLabel> {
        debug!(
            "Creating Motion label: {} in workspace {}",
            name, workspace_id
        );

        #[derive(Serialize, Debug)]
        struct CreateLabelRequest {
            name: String,
            #[serde(rename = "workspaceId")]
            workspace_id: String,
            #[serde(rename = "colorHex", skip_serializing_if = "Option::is_none")]
            color_hex: Option<String>,
        }

        let request = CreateLabelRequest {
            name: name.to_string(),
            workspace_id: workspace_id.to_string(),
            color_hex: color_hex.map(|c| c.to_string()),
        };

        let label: MotionLabel = self.make_post_request("labels", &request).await?;
        debug!("Created Motion label: {} (ID: {})", label.name, label.id);
        Ok(label)
    }

    pub async fn ensure_label_exists(
        &self,
        workspace_id: &str,
        label_name: &str,
    ) -> Result<MotionLabel> {
        // First, check if the label already exists
        let labels = self.list_labels(workspace_id).await?;

        if let Some(existing_label) = labels.iter().find(|l| l.name == label_name) {
            debug!(
                "Label '{}' already exists with ID: {}",
                label_name, existing_label.id
            );
            return Ok(existing_label.clone());
        }

        // If it doesn't exist, create it
        debug!(
            "Label '{}' does not exist in workspace {}, creating it",
            label_name, workspace_id
        );
        let color = match label_name {
            "linear-sync" => Some("#4F46E5"), // Indigo color for sync labels
            _ => None,
        };

        self.create_label(workspace_id, label_name, color).await
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deserialize() {
        let x = r#"{"id":"tk_2gkqzra9Lwy5VsMJmoEL1S","name":"[CAR-126] Reach out to dentist","description":"","duration":30,"dueDate":null,"deadlineType":"SOFT","parentRecurringTaskId":null,"completed":false,"completedTime":null,"updatedTime":"2025-09-07T20:48:54.036Z","creator":{"id":"vDP3xJXG3kSNEq2kBsU1yAoCc872","name":"Alexander Lyon","email":"alyon.ipride@gmail.com"},"workspace":{"id":"S01pxiftfAAQzD3JjeA3T","name":"My Tasks (Private)","teamId":null,"statuses":[{"name":"Backlog","isDefaultStatus":false,"isResolvedStatus":false},{"name":"Canceled","isDefaultStatus":false,"isResolvedStatus":false},{"name":"Completed","isDefaultStatus":false,"isResolvedStatus":true},{"name":"Not Started","isDefaultStatus":false,"isResolvedStatus":false},{"name":"Todo","isDefaultStatus":true,"isResolvedStatus":false}],"labels":[{"name":"linear-sync"}],"type":"INDIVIDUAL"},"project":null,"status":{"name":"Todo","isDefaultStatus":true,"isResolvedStatus":false},"priority":"LOW","labels":[{"name":"linear-sync"}],"assignees":[{"id":"vDP3xJXG3kSNEq2kBsU1yAoCc872","name":"Alexander Lyon","email":"alyon.ipride@gmail.com"}],"scheduledStart":null,"createdTime":"2025-09-07T20:48:54.036Z","scheduledEnd":null,"schedulingIssue":false,"lastInteractedTime":"2025-09-07T20:48:54.055Z","customFieldValues":{},"chunks":[]}"#;

        // Some Deserializer.
        let jd = &mut serde_json::Deserializer::from_str(x);

        // deserialize with serde_json
        let task: MotionTask = serde_path_to_error::deserialize(jd).unwrap();
    }

    #[test]
    fn test_task_duration_serialization() {
        // Test that TaskDuration serializes to the correct format for Motion API
        let duration_none = TaskDuration::None;
        let duration_reminder = TaskDuration::Reminder;
        let duration_30_min = TaskDuration::Minutes(30);
        let duration_120_min = TaskDuration::Minutes(120);

        // Serialize to JSON
        let json_none = serde_json::to_string(&duration_none).unwrap();
        let json_reminder = serde_json::to_string(&duration_reminder).unwrap();
        let json_30 = serde_json::to_string(&duration_30_min).unwrap();
        let json_120 = serde_json::to_string(&duration_120_min).unwrap();

        // Check correct formats
        assert_eq!(json_none, "\"NONE\"");
        assert_eq!(json_reminder, "\"REMINDER\"");
        assert_eq!(json_30, "30");
        assert_eq!(json_120, "120");

        // Test round-trip deserialization
        let parsed_none: TaskDuration = serde_json::from_str(&json_none).unwrap();
        let parsed_reminder: TaskDuration = serde_json::from_str(&json_reminder).unwrap();
        let parsed_30: TaskDuration = serde_json::from_str(&json_30).unwrap();
        let parsed_120: TaskDuration = serde_json::from_str(&json_120).unwrap();

        match parsed_none {
            TaskDuration::None => (),
            _ => panic!("Expected TaskDuration::None"),
        }

        match parsed_reminder {
            TaskDuration::Reminder => (),
            _ => panic!("Expected TaskDuration::Reminder"),
        }

        match parsed_30 {
            TaskDuration::Minutes(30) => (),
            _ => panic!("Expected TaskDuration::Minutes(30)"),
        }

        match parsed_120 {
            TaskDuration::Minutes(120) => (),
            _ => panic!("Expected TaskDuration::Minutes(120)"),
        }
    }
}
