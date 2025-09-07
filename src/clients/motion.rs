use crate::{Error, Result};
use chrono::{DateTime, Utc};
use governor::{Quota, RateLimiter};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use tracing::{debug, error, info};

#[derive(Debug)]
pub struct MotionClient {
    client: Client,
    api_key: String,
    base_url: String,
    rate_limiter: RateLimiter<
        governor::state::direct::NotKeyed,
        governor::state::InMemoryState,
        governor::clock::DefaultClock,
    >,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionTask {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub workspace_id: String,
    pub assignee_id: Option<String>,
    pub project_id: Option<String>,
    pub priority: Option<String>, // "ASAP", "HIGH", "MEDIUM", "LOW"
    pub due_date: Option<DateTime<Utc>>,
    pub duration: Option<String>, // "NONE", "REMINDER", or minutes as string
    pub status: Option<String>,
    pub completed: Option<bool>,
    pub labels: Option<Vec<String>>,
    pub created_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionWorkspace {
    pub id: String,
    pub name: String,
    pub team_id: String,
    pub workspace_type: String, // "team" or "individual"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionUser {
    pub id: String,
    pub name: String,
    pub email: String,
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
    meta: MotionMeta,
}

impl MotionClient {
    pub fn new(api_key: String) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_str(&api_key)?);

        let client = Client::builder().default_headers(headers).build()?;

        // Motion rate limit: 12 requests per minute for individuals, 120 for teams
        // We'll be conservative and use 10 requests per minute
        let quota = Quota::per_minute(NonZeroU32::new(10).unwrap());
        let rate_limiter = RateLimiter::direct(quota);

        Ok(Self {
            client,
            api_key,
            base_url: "https://api.usemotion.com/v1".to_string(),
            rate_limiter,
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

        let response = self.client.get(&url).send().await?;

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

    async fn make_post_request<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        self.rate_limit().await?;

        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));
        debug!("Making Motion API request: POST {}", url);

        let response = self.client.post(&url).json(body).send().await?;

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

    async fn make_patch_request<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        self.rate_limit().await?;

        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));
        debug!("Making Motion API request: PATCH {}", url);

        let response = self.client.patch(&url).json(body).send().await?;

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
        info!("Connected to Motion as: {} ({})", user.name, user.email);
        Ok(user)
    }

    pub async fn list_workspaces(&self) -> Result<Vec<MotionWorkspace>> {
        let response: WorkspaceListResponse = self.make_request("workspaces").await?;
        info!("Found {} Motion workspaces", response.workspaces.len());
        Ok(response.workspaces)
    }

    pub async fn create_task(&self, task: &MotionTask) -> Result<MotionTask> {
        debug!("Creating Motion task: {}", task.name);

        #[derive(Serialize)]
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
            duration: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            labels: Option<Vec<String>>,
        }

        let request = CreateTaskRequest {
            name: task.name.clone(),
            workspace_id: task.workspace_id.clone(),
            description: task.description.clone(),
            assignee_id: task.assignee_id.clone(),
            project_id: task.project_id.clone(),
            priority: task.priority.clone(),
            due_date: task.due_date,
            duration: task.duration.clone(),
            labels: task.labels.clone(),
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
            duration: Option<String>,
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
            status: task.status.clone(),
            labels: task.labels.clone(),
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
}
