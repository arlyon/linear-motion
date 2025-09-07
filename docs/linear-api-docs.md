# Linear API Documentation

Complete documentation for the Linear GraphQL API and webhooks for the linear-motion sync project.

## Table of Contents

- [Authentication](#authentication)
- [GraphQL API](#graphql-api)
- [Webhooks](#webhooks)
- [Key Data Structures](#key-data-structures)
- [Sync Strategy](#sync-strategy)
- [Rate Limits](#rate-limits)
- [Error Handling](#error-handling)

---

## Authentication

### API Key Authentication

Linear uses API key authentication for GraphQL API access:

```http
Authorization: Bearer YOUR_API_KEY
```

### OAuth 2.0

For applications requiring user authorization:

- **Authorization URL**: `https://linear.app/oauth/authorize`
- **Token URL**: `https://api.linear.app/oauth/token`
- **Scopes**: Various scopes available for different resource access

---

## GraphQL API

### Base URL

```
https://api.linear.app/graphql
```

### Key Endpoints

All API interactions use a single GraphQL endpoint with different queries and mutations.

#### Example Query

```graphql
query {
  issues(first: 50) {
    nodes {
      id
      identifier
      title
      description
      state {
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
        state
      }
      labels {
        nodes {
          id
          name
          color
        }
      }
      priority
      estimate
      createdAt
      updatedAt
      dueDate
      completedAt
    }
  }
}
```

---

## Webhooks

### Webhook Configuration

Linear supports webhooks for real-time synchronization:

#### Creating a Webhook

```graphql
mutation {
  webhookCreate(input: {
    url: "https://your-app.com/webhooks/linear"
    resourceTypes: ["Issue", "Project", "Comment", "IssueLabel"]
    enabled: true
    secret: "your-webhook-secret"
    allPublicTeams: true
  }) {
    success
    webhook {
      id
      url
      resourceTypes
      enabled
    }
  }
}
```

#### Supported Resource Types

- `Issue` - Issue creation, updates, state changes
- `Project` - Project creation, updates, status changes
- `Comment` - Comments on issues and projects
- `IssueLabel` - Label creation, updates, assignments
- `User` - User updates (limited)
- `Team` - Team changes
- `Cycle` - Development cycle changes

#### Webhook Payload Structure

```json
{
  "action": "create|update|delete",
  "type": "Issue|Project|Comment|IssueLabel",
  "data": {
    "id": "issue-id",
    "identifier": "ENG-123",
    "title": "Issue Title",
    "description": "Issue description in markdown",
    "state": {
      "id": "state-id",
      "name": "In Progress",
      "type": "started"
    },
    "assignee": {
      "id": "user-id",
      "name": "John Doe",
      "email": "john@company.com"
    },
    "team": {
      "id": "team-id",
      "name": "Engineering",
      "key": "ENG"
    },
    "project": {
      "id": "project-id",
      "name": "Project Name",
      "state": "inProgress"
    },
    "labels": [
      {
        "id": "label-id",
        "name": "bug",
        "color": "#ff6b6b"
      }
    ],
    "priority": 2,
    "estimate": 3,
    "createdAt": "2024-01-01T00:00:00.000Z",
    "updatedAt": "2024-01-01T12:00:00.000Z",
    "dueDate": "2024-01-15T00:00:00.000Z",
    "completedAt": null
  },
  "updatedFrom": {
    // Previous values for update events
  },
  "createdAt": "2024-01-01T12:00:00.000Z"
}
```

#### Webhook Security

Webhooks are signed with HMAC-SHA256:

```rust
// Example verification in Rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn verify_webhook(payload: &str, signature: &str, secret: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    let expected = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    signature == expected
}
```

---

## Key Data Structures

### Issue

The primary entity for tasks/tickets:

```graphql
type Issue {
  id: ID!
  identifier: String!  # Human-readable ID like "ENG-123"
  title: String!
  description: String  # Markdown format
  
  # Status and workflow
  state: WorkflowState!
  priority: Int        # 0=No priority, 1=Urgent, 2=High, 3=Medium, 4=Low
  estimate: Float      # Story points or time estimate
  
  # Relationships
  assignee: User
  team: Team!
  project: Project
  cycle: Cycle
  parent: Issue        # For sub-issues
  labels: [IssueLabel!]!
  
  # Attachments and comments
  attachments: [Attachment!]!
  comments: [Comment!]!
  
  # Dates
  createdAt: DateTime!
  updatedAt: DateTime!
  startedAt: DateTime
  completedAt: DateTime
  canceledAt: DateTime
  dueDate: DateTime
  
  # Metadata
  url: String!
  branchName: String
  customerTicketCount: Int
}
```

### Project

Container for organizing issues:

```graphql
type Project {
  id: ID!
  name: String!
  description: String
  slugId: String!
  
  # Visual
  color: String!
  icon: String
  
  # Status
  state: String!      # "planned", "started", "paused", "completed", "canceled"
  health: String      # "onTrack", "atRisk", "offTrack"
  
  # Relationships
  lead: User
  creator: User!
  teams: [Team!]!
  issues: [Issue!]!
  members: [User!]!
  
  # Progress
  progress: Float!    # 0.0 to 1.0
  scope: Int!         # Total story points
  
  # Dates
  createdAt: DateTime!
  updatedAt: DateTime!
  startDate: DateTime
  targetDate: DateTime
  completedAt: DateTime
  canceledAt: DateTime
  
  # Metadata
  url: String!
  sortOrder: Float!
  
  # Content
  updates: [ProjectUpdate!]!
  documents: [Document!]!
  links: [ProjectLink!]!
  milestones: [ProjectMilestone!]!
}
```

### Team

Organizational unit:

```graphql
type Team {
  id: ID!
  name: String!
  key: String!         # Short identifier like "ENG"
  description: String
  
  # Visual
  color: String
  icon: String
  
  # Configuration
  private: Boolean!
  
  # Relationships
  organization: Organization!
  members: [User!]!
  issues: [Issue!]!
  projects: [Project!]!
  cycles: [Cycle!]!
  labels: [IssueLabel!]!
  states: [WorkflowState!]!
  templates: [Template!]!
  
  # Dates
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Settings
  issueEstimationType: String  # "notUsed", "exponential", "fibonacci", "linear", "tShirt"
  issueOrderingNoPriorityFirst: Boolean!
  issueGenerationEnabled: Boolean!
  cyclesEnabled: Boolean!
  
  # Integrations
  integrationsSettings: IntegrationsSettings
  webhooks: [Webhook!]!
}
```

### User

Team member:

```graphql
type User {
  id: ID!
  name: String!
  displayName: String!
  email: String!
  
  # Profile
  avatarUrl: String
  timezone: String
  
  # Status
  active: Boolean!
  admin: Boolean!
  guest: Boolean!
  
  # Relationships
  organization: Organization!
  teams: [Team!]!
  assignedIssues: [Issue!]!
  createdIssues: [Issue!]!
  
  # Dates
  createdAt: DateTime!
  updatedAt: DateTime!
  lastSeenAt: DateTime
  
  # Settings
  isMe: Boolean!
  url: String!
}
```

### WorkflowState

Issue status:

```graphql
type WorkflowState {
  id: ID!
  name: String!
  description: String
  
  # Visual
  color: String!
  type: String!        # "backlog", "unstarted", "started", "completed", "canceled"
  
  # Configuration
  position: Float!     # Sort order
  
  # Relationships
  team: Team!
  issues: [Issue!]!
  
  # Dates
  createdAt: DateTime!
  updatedAt: DateTime!
}
```

---

## Sync Strategy

### Recommended Approach for linear-motion

1. **Initial Sync**
   - Fetch all projects and their issues
   - Create corresponding Motion workspaces/projects and tasks
   - Store mapping between Linear IDs and Motion IDs

2. **Webhook-Driven Updates**
   - Set up webhooks for real-time sync
   - Handle create, update, delete events
   - Implement conflict resolution

3. **Bidirectional Sync**
   - Linear → Motion: Primary direction via webhooks
   - Motion → Linear: Periodic sync or via Motion webhooks (if available)

### Key Queries for Sync

#### Get All Projects with Issues

```graphql
query GetProjectsWithIssues($first: Int!, $after: String) {
  projects(first: $first, after: $after) {
    nodes {
      id
      name
      description
      state
      color
      targetDate
      startDate
      lead { id name email }
      issues(first: 100) {
        nodes {
          id
          identifier
          title
          description
          state { name type }
          assignee { id name email }
          priority
          estimate
          dueDate
          createdAt
          updatedAt
          completedAt
        }
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
```

#### Get Team Information

```graphql
query GetTeams {
  teams {
    nodes {
      id
      name
      key
      members {
        id
        name
        email
      }
      states {
        id
        name
        type
        color
      }
    }
  }
}
```

#### Create Issue

```graphql
mutation CreateIssue($input: IssueCreateInput!) {
  issueCreate(input: $input) {
    success
    issue {
      id
      identifier
      title
    }
  }
}
```

#### Update Issue

```graphql
mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) {
    success
    issue {
      id
      identifier
      title
      updatedAt
    }
  }
}
```

---

## Rate Limits

Linear implements rate limiting:

- **Default**: 1000 requests per hour per API key
- **Burst**: Up to 100 requests per minute
- **Headers**: Rate limit information in response headers
  - `x-ratelimit-limit`
  - `x-ratelimit-remaining`
  - `x-ratelimit-reset`

### Best Practices

- Use GraphQL field selection to minimize data transfer
- Implement exponential backoff for rate limit errors
- Cache frequently accessed data
- Use webhooks instead of polling

---

## Error Handling

### Common Errors

#### Authentication Errors

```json
{
  "errors": [
    {
      "message": "Authentication required",
      "extensions": {
        "code": "FORBIDDEN"
      }
    }
  ]
}
```

#### Rate Limiting

```json
{
  "errors": [
    {
      "message": "Rate limited",
      "extensions": {
        "code": "RATE_LIMITED"
      }
    }
  ]
}
```

#### Validation Errors

```json
{
  "errors": [
    {
      "message": "Variable '$input' of required type 'IssueCreateInput!' was not provided.",
      "extensions": {
        "code": "BAD_USER_INPUT"
      }
    }
  ]
}
```

### Error Recovery

1. **Retry Logic**: Implement exponential backoff
2. **Partial Failures**: Handle GraphQL partial success scenarios  
3. **Webhook Failures**: Linear retries failed webhooks with exponential backoff
4. **Data Integrity**: Implement checksums or version tracking

---

## Implementation Notes for Rust

### Recommended Crates

```toml
[dependencies]
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
graphql_client = "0.13"
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

### GraphQL Client Setup

```rust
use graphql_client::GraphQLQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/linear_schema.graphql",
    query_path = "src/queries/get_issues.graphql",
)]
pub struct GetIssues;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/linear_schema.graphql", 
    query_path = "src/mutations/create_issue.graphql",
)]
pub struct CreateIssue;
```

### Webhook Handler Example

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde_json::Value;

async fn linear_webhook_handler(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<StatusCode, StatusCode> {
    // Verify webhook signature
    let signature = headers
        .get("linear-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    if !verify_webhook_signature(&payload.to_string(), signature, &app_state.webhook_secret) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // Process webhook
    match payload.get("type").and_then(|t| t.as_str()) {
        Some("Issue") => handle_issue_webhook(payload, &app_state).await?,
        Some("Project") => handle_project_webhook(payload, &app_state).await?,
        _ => return Err(StatusCode::BAD_REQUEST),
    }
    
    Ok(StatusCode::OK)
}
```

### Data Mapping

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub state: WorkflowState,
    pub assignee: Option<User>,
    pub team: Team,
    pub project: Option<Project>,
    pub priority: i32,
    pub estimate: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<LinearIssue> for motion_api::Task {
    fn from(issue: LinearIssue) -> Self {
        motion_api::Task {
            name: issue.title,
            description: issue.description,
            assignee_id: issue.assignee.map(|a| a.id),
            workspace_id: map_team_to_workspace(&issue.team.id),
            project_id: issue.project.map(|p| map_linear_project_to_motion(&p.id)),
            priority: map_linear_priority_to_motion(issue.priority),
            due_date: issue.due_date,
            // ... other mappings
        }
    }
}
```

---

*This documentation covers the essential Linear API features needed for building the linear-motion synchronization tool.*