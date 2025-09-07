use crate::{Result, Error};
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LinearClient {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub state: WorkflowState,
    pub assignee: Option<User>,
    pub team: Team,
    pub project: Option<Project>,
    pub priority: Option<u32>,
    pub estimate: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub due_date: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub labels: Vec<IssueLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub id: String,
    pub name: String,
    pub state_type: String, // "backlog", "unstarted", "started", "completed", "canceled"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueLabel {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    variables: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
    locations: Option<Vec<GraphQLLocation>>,
    path: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLLocation {
    line: u32,
    column: u32,
}

#[derive(Deserialize)]
struct LabelsConnection {
    nodes: Vec<IssueLabel>,
}

impl LinearClient {
    pub fn new(api_key: String) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&api_key)?
        );
        
        let client = Client::builder()
            .default_headers(headers)
            .build()?;
        
        Ok(Self {
            client,
            api_key,
            base_url: "https://api.linear.app/graphql".to_string(),
        })
    }

    async fn execute_query<T: for<'de> Deserialize<'de>>(&self, query: &str, variables: Option<Value>) -> Result<T> {
        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
        };

        debug!("Executing Linear GraphQL query: {}", query);
        
        let response = self.client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            error!("Linear API error: {} - {}", status, text);
            return Err(Error::LinearApi { 
                message: format!("HTTP {}: {}", status, text) 
            });
        }

        let response_text = response.text().await?;
        debug!("Linear GraphQL response: {}", response_text);
        let response_json: GraphQLResponse<T> = match serde_json::from_str(&response_text) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to parse Linear GraphQL response: {}", e);
                error!("Full response text: {}", response_text);
                return Err(Error::Json(e));
            }
        };

        if let Some(errors) = response_json.errors {
            let error_messages: Vec<String> = errors.into_iter()
                .map(|e| e.message)
                .collect();
            error!("Linear GraphQL errors: {:?}", error_messages);
            return Err(Error::LinearApi { 
                message: error_messages.join(", ") 
            });
        }

        response_json.data.ok_or_else(|| Error::LinearApi {
            message: "No data in response".to_string()
        })
    }

    pub async fn get_viewer(&self) -> Result<User> {
        let query = r#"
            query {
                viewer {
                    id
                    name
                    email
                }
            }
        "#;

        #[derive(Deserialize)]
        struct ViewerResponse {
            viewer: User,
        }

        let response: ViewerResponse = self.execute_query(query, None).await?;
        info!("Connected to Linear as: {} ({})", response.viewer.name, response.viewer.email);
        
        Ok(response.viewer)
    }

    pub async fn get_assigned_issues(&self, project_ids: Option<Vec<String>>) -> Result<Vec<LinearIssue>> {
        let viewer = self.get_viewer().await?;
        
        let (query, variables) = if let Some(project_ids) = project_ids {
            let query = r#"
                query GetAssignedIssues($assigneeId: ID!, $projectIds: [String!]) {
                    issues(
                        filter: {
                            assignee: { id: { eq: $assigneeId } }
                            project: { id: { in: $projectIds } }
                            state: { type: { nin: ["completed", "canceled"] } }
                        }
                        first: 100
                    ) {"#;
            
            let variables = Some(serde_json::json!({
                "assigneeId": viewer.id,
                "projectIds": project_ids
            }));
            (query, variables)
        } else {
            let query = r#"
                query GetAssignedIssues($assigneeId: ID!) {
                    issues(
                        filter: {
                            assignee: { id: { eq: $assigneeId } }
                            state: { type: { nin: ["completed", "canceled"] } }
                        }
                        first: 100
                    ) {"#;
            
            let variables = Some(serde_json::json!({
                "assigneeId": viewer.id
            }));
            (query, variables)
        };
        
        let full_query = format!("{}{}", query, r#"
                        nodes {
                            id
                            identifier
                            title
                            description
                            state {
                                id
                                name
                                type
                            }
                            assignee {
                                id
                                name
                                email
                            }
                            team {
                                id
                                name
                                key
                            }
                            project {
                                id
                                name
                                description
                                state
                            }
                            priority
                            estimate
                            createdAt
                            updatedAt
                            dueDate
                            completedAt
                            labels {
                                nodes {
                                    id
                                    name
                                    color
                                }
                            }
                        }
                    }
                }
            "#);

        #[derive(Deserialize)]
        struct IssuesResponse {
            issues: IssuesConnection,
        }

        #[derive(Deserialize)]
        struct IssuesConnection {
            nodes: Vec<LinearIssueRaw>,
        }

        #[derive(Deserialize)]
        struct LinearIssueRaw {
            id: String,
            identifier: String,
            title: String,
            description: Option<String>,
            state: WorkflowStateRaw,
            assignee: Option<User>,
            team: Team,
            project: Option<Project>,
            priority: Option<u32>,
            estimate: Option<f64>,
            #[serde(rename = "createdAt")]
            created_at: DateTime<Utc>,
            #[serde(rename = "updatedAt")]
            updated_at: DateTime<Utc>,
            #[serde(rename = "dueDate")]
            due_date: Option<String>,
            #[serde(rename = "completedAt")]
            completed_at: Option<DateTime<Utc>>,
            labels: LabelsConnection,
        }

        #[derive(Deserialize)]
        struct WorkflowStateRaw {
            id: String,
            name: String,
            #[serde(rename = "type")]
            state_type: String,
        }

        let response: IssuesResponse = self.execute_query(&full_query, variables).await?;
        
        let issues: Vec<LinearIssue> = response.issues.nodes.into_iter().map(|raw| LinearIssue {
            id: raw.id,
            identifier: raw.identifier,
            title: raw.title,
            description: raw.description,
            state: WorkflowState {
                id: raw.state.id,
                name: raw.state.name,
                state_type: raw.state.state_type,
            },
            assignee: raw.assignee,
            team: raw.team,
            project: raw.project,
            priority: raw.priority,
            estimate: raw.estimate,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            due_date: raw.due_date,
            completed_at: raw.completed_at,
            labels: raw.labels.nodes,
        }).collect();

        info!("Found {} assigned issues", issues.len());
        Ok(issues)
    }

    pub async fn add_label_to_issue(&self, issue_id: &str, label_name: &str) -> Result<()> {
        // First, we need to find the label ID
        let label_id = self.get_or_create_label(label_name).await?;
        
        let query = r#"
            mutation AddLabelToIssue($issueId: String!, $labelId: String!) {
                issueLabelCreate(input: {
                    issueId: $issueId,
                    labelId: $labelId
                }) {
                    success
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("issueId", Value::String(issue_id.to_string()));
        variables.insert("labelId", Value::String(label_id));

        #[derive(Deserialize)]
        struct AddLabelResponse {
            #[serde(rename = "issueLabelCreate")]
            issue_label_create: MutationResponse,
        }

        #[derive(Deserialize)]
        struct MutationResponse {
            success: bool,
        }

        let response: AddLabelResponse = self.execute_query(query, Some(serde_json::to_value(variables)?)).await?;
        
        if !response.issue_label_create.success {
            warn!("Failed to add label '{}' to issue {}", label_name, issue_id);
            return Err(Error::LinearApi {
                message: format!("Failed to add label '{}' to issue", label_name)
            });
        }

        debug!("Added label '{}' to issue {}", label_name, issue_id);
        Ok(())
    }

    pub async fn get_or_create_label(&self, label_name: &str) -> Result<String> {
        // First try to find existing label
        if let Some(label_id) = self.find_label(label_name).await? {
            return Ok(label_id);
        }

        // If not found, create it
        self.create_label(label_name).await
    }

    async fn find_label(&self, label_name: &str) -> Result<Option<String>> {
        let query = r#"
            query FindLabel($name: String!) {
                issueLabels(filter: { name: { eq: $name } }, first: 1) {
                    nodes {
                        id
                        name
                    }
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("name", Value::String(label_name.to_string()));

        #[derive(Deserialize)]
        struct FindLabelResponse {
            #[serde(rename = "issueLabels")]
            issue_labels: LabelsConnection,
        }

        let response: FindLabelResponse = self.execute_query(query, Some(serde_json::to_value(variables)?)).await?;
        
        Ok(response.issue_labels.nodes.first().map(|label| label.id.clone()))
    }

    async fn create_label(&self, label_name: &str) -> Result<String> {
        let query = r#"
            mutation CreateLabel($name: String!, $color: String!) {
                issueLabelCreate(input: {
                    name: $name,
                    color: $color
                }) {
                    success
                    issueLabel {
                        id
                        name
                    }
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("name", Value::String(label_name.to_string()));
        variables.insert("color", Value::String("#3B82F6".to_string())); // Default blue color

        #[derive(Deserialize)]
        struct CreateLabelResponse {
            #[serde(rename = "issueLabelCreate")]
            issue_label_create: CreateLabelMutation,
        }

        #[derive(Deserialize)]
        struct CreateLabelMutation {
            success: bool,
            #[serde(rename = "issueLabel")]
            issue_label: Option<IssueLabel>,
        }

        let response: CreateLabelResponse = self.execute_query(query, Some(serde_json::to_value(variables)?)).await?;
        
        if !response.issue_label_create.success {
            return Err(Error::LinearApi {
                message: format!("Failed to create label '{}'", label_name)
            });
        }

        let label = response.issue_label_create.issue_label
            .ok_or_else(|| Error::LinearApi {
                message: "Label creation succeeded but no label returned".to_string()
            })?;

        info!("Created label '{}' with ID: {}", label.name, label.id);
        Ok(label.id)
    }

    pub async fn check_issue_has_label(&self, issue_id: &str, label_name: &str) -> Result<bool> {
        let query = r#"
            query CheckIssueLabel($issueId: String!) {
                issue(id: $issueId) {
                    labels {
                        nodes {
                            name
                        }
                    }
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("issueId", Value::String(issue_id.to_string()));

        #[derive(Deserialize)]
        struct CheckLabelResponse {
            issue: IssueLabelsOnly,
        }

        #[derive(Deserialize)]
        struct IssueLabelsOnly {
            labels: LabelsConnection,
        }

        let response: CheckLabelResponse = self.execute_query(query, Some(serde_json::to_value(variables)?)).await?;
        
        let has_label = response.issue.labels.nodes.iter()
            .any(|label| label.name == label_name);

        debug!("Issue {} has label '{}': {}", issue_id, label_name, has_label);
        Ok(has_label)
    }
}