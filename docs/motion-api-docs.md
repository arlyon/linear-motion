# Motion API Documentation

Complete documentation for the Motion API, extracted from https://docs.usemotion.com/

## Table of Contents

- [Getting Started](#getting-started)
- [Cookbooks](#cookbooks)
  - [Getting Started](#getting-started-1)
  - [Description](#description)
  - [Frequency](#frequency)
  - [Rate Limits](#rate-limits)
- [API Reference](#api-reference)
  - [Comments](#comments)
  - [Custom Fields](#custom-fields)
  - [Projects](#projects)
  - [Recurring Tasks](#recurring-tasks)
  - [Schedules](#schedules)
  - [Statuses](#statuses)
  - [Tasks](#tasks)
  - [Users](#users)
  - [Workspaces](#workspaces)

---

## Getting Started

### Create API Key

1. Log into Motion and under the Settings tab, create an API key.
   - **Note**: Be sure to copy the key, as it will only be shown once for security reasons.

### Set Authorization Headers

Pass in your API key as a `X-API-Key` header.

### Test the API

Try sending a GET request to `https://api.usemotion.com/v1/workspaces` with your API key as a header!

---

## Cookbooks

### Getting Started

#### Create API Key

1. Log into Motion and under the Settings tab, create an API key.
   - **Note**: Be sure to copy the key, as it will only be shown once for security reasons.

#### Set Authorization Headers

Pass in your API key as a `X-API-Key` header.

#### Test the API

Try sending a GET request to `https://api.usemotion.com/v1/workspaces` with your API key as a header!

### Description

#### Github Flavored Markdown

Motion uses [Github Flavored Markdown](https://github.github.com/gfm/) for description fields. API users can test their code using a markdown to HTML converter like [showdown](https://www.npmjs.com/package/showdown).

#### Limitations

Currently, Motion uses Prosemirror for their text editor, which has some compatibility issues with Github Flavored Markdown. Specifically:

- Checkboxes created with `- [ ] My checkbox` or `- [x] My checkbox` do not work

#### Workaround

Until the Prosemirror limitation is resolved, use the following raw HTML string to create a checkbox:

```html
<ul data-type="taskList">
  <li data-checked="false">
    <label contenteditable="false">
      <input type="checkbox">
      <span></span>
    </label>
    <div>
      <p>YOUR SUBTASK ITEM HERE</p>
    </div>
  </li>
</ul>
```

### Frequency

#### Days

Defining days should always be used along with a specific frequency type. When picking specific week days, use an array with these values:

- `MO` - Monday
- `TU` - Tuesday
- `WE` - Wednesday
- `TH` - Thursday
- `FR` - Friday
- `SA` - Saturday
- `SU` - Sunday

Example: `[MO, FR, SU]` means Monday, Friday, and Sunday.

#### Defining a Daily Frequency

Options include:
- `daily_every_day`
- `daily_every_week_day`
- `daily_specific_days_[DAYS_ARRAY]`

Example: `daily_specific_days_[MO, TU, FR]` means every Monday, Tuesday, and Friday.

#### Defining a Weekly Frequency

Options include:
- `weekly_any_day`
- `weekly_any_week_day`
- `weekly_specific_days_[DAYS_ARRAY]`

Example: `weekly_specific_days_[MO, TU, FR]` means once a week on Monday, Tuesday, or Friday.

#### Defining a Bi-Weekly Frequency

Options include:
- `biweekly_first_week_specific_days_[DAYS_ARRAY]`
- `biweekly_first_week_any_day`
- `biweekly_first_week_any_week_day`
- `biweekly_second_week_any_day`
- `biweekly_second_week_any_week_day`

Example: `biweekly_first_week_specific_days_[MO, TU, FR]` means biweekly on first week's Monday, Tuesday, or Friday.

#### Defining a Monthly Frequency

##### Specific Week Day Options
- `monthly_first_DAY`
- `monthly_second_DAY`
- `monthly_third_DAY`
- `monthly_fourth_DAY`
- `monthly_last_DAY`

Example: `monthly_first_MO` means first Monday of the month.

### Rate Limits

The base tier for individuals is 12 requests per minute.

Teams can request up to 120 requests per minute.

For even higher rate limits, please sign up for our enterprise tier.

---

## API Reference

### Comments

#### Get Comments

**Endpoint:** `GET /v1/comments`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Query Parameters:**
- **cursor** (string) - Use for pagination through results
- **taskId** (string, required) - Task for which comments should be returned

**Response (200 - application/json):**

Meta Object:
- **nextCursor** (string) - Cursor for next page of results
- **pageSize** (number) - Number of results in response

Comments Array:
- **id** (string)
- **taskId** (string)
- **content** (string, HTML)
- **createdAt** (datetime)
- **creator** (object)
  - **id** (string)
  - **name** (string)
  - **email** (string)

#### Create Comment

**Endpoint:** `POST /v1/comments`

**Authorization:**
- **X-API-Key** (string, required) - Header containing API key

**Request Body (application/json):**
- **taskId** (string, required) - Task to comment on
- **content** (string) - Comment in Github Flavored Markdown

**Response (200 OK):**
```json
{
  "id": "string",
  "taskId": "string", 
  "content": "string",
  "createdAt": "datetime",
  "creator": {
    "id": "string",
    "name": "string",
    "email": "string"
  }
}
```

### Custom Fields

#### Add Custom Field to Project

**Endpoint:** `POST /beta/custom-field-values/project/{projectId}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **projectId** (string, required) - Project for adding custom field value

**Request Body (application/json):**
- **customFieldInstanceId** (string, required) - Custom field being set
- **value** (object, required):
  - **type** (string, required) - Custom field type
  - **value** (string or number) - Actual value being set

**Supported Types:** "text", "url", "date", "person", "multiPerson", "phone", "select", "multiSelect", "number", "email", "checkbox", "relatedTo"

#### Add Custom Field to Task

**Endpoint:** `POST /beta/custom-field-values/task/{taskId}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **taskId** (string, required) - Task for which a new custom field value will be added

**Request Body (application/json):**
- **customFieldInstanceId** (string, required) - Custom field on the workspace being set
- **value** (object, required):
  - **type** (string, required) - Custom field type
  - **value** (string or number) - Actual value being set

#### Delete Custom Field from Project

**Endpoint:** `DELETE /beta/custom-field-values/project/{projectId}/custom-fields/{valueId}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **projectId** (string, required) - Project from which custom field value will be deleted
- **valueId** (string, required) - ID of custom field value to be deleted

#### Delete Custom Field from Task

**Endpoint:** `DELETE /beta/custom-field-values/task/{taskId}/custom-fields/{valueId}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **taskId** (string, required) - The task from which a custom field value will be deleted
- **valueId** (string, required) - The ID of the custom field value that will be deleted

#### Delete Custom Field

**Endpoint:** `DELETE /beta/workspaces/{workspaceId}/custom-fields/{id}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **workspaceId** (string, required) - Workspace for deleting custom field
- **id** (string, required) - ID of custom field to delete

#### Get Custom Fields

**Endpoint:** `GET /beta/workspaces/{workspaceId}/custom-fields`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **workspaceId** (string, required) - Workspace to retrieve custom fields from

**Response (200 - application/json):**
Array with the following properties:
- **id** (string, required) - Custom field ID
- **field** (string, required) - Custom field type

**Supported Field Types:** text, url, date, person, multiPerson, phone, select, multiSelect, number, email, checkbox, relatedTo

#### Create Custom Field

**Endpoint:** `POST /beta/workspaces/{workspaceId}/custom-fields`

**Authorization:**
- **X-API-Key** (string, required) - API key in header

**Path Parameters:**
- **workspaceId** (string, required) - Workspace for creating custom field

**Request Body (application/json):**

Required Fields:
- **type** (string) - Custom field type
- **name** (string) - Custom field name

Optional Fields:
- **metadata** (object) - Advanced configuration
  - For number fields: `format` (plain/formatted/percent)
  - For checkbox: `toggle` (boolean)
  - For select fields: `options` array with `id`, `value`, `color`

**Valid Types:** "text", "url", "date", "person", "multiPerson", "phone", "select", "multiSelect", "number", "email", "checkbox", "relatedTo"

### Projects

#### Get Project

**Endpoint:** `GET /v1/projects/{id}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **id** (string, required) - Project ID to return

**Response (200 - application/json):**
- `id` - Project ID (string)
- `name` - Project name (string)
- `description` - HTML description contents (string)
- `workspaceId` - Workspace ID (string)
- `status` - Project status object
- `createdTime` - Creation timestamp
- `updatedTime` - Last update timestamp
- `customFieldValues` - Custom field values record

#### List Projects

**Endpoint:** `GET /v1/projects`

**Authorization:**
- **X-API-Key** (string, required) - API key in header

**Query Parameters:**
- `cursor` (string) - For pagination
- `workspaceId` (string) - Workspace to retrieve projects from

**Response (200 OK):**

Meta Object:
- `nextCursor` (string) - Cursor for next page
- `pageSize` (number) - Number of results

Projects Array:
- `id` (string)
- `name` (string)
- `description` (string, HTML)
- `workspaceId` (string)
- `status` (object)
- `createdTime` (datetime)
- `updatedTime` (datetime)
- `customFieldValues` (record)

#### Create Project

**Endpoint:** `POST /v1/projects`

**Authorization:**
- **X-API-Key** (string, required) - API key in header

**Request Body (application/json):**

Required Parameters:
- `name` (string) - Project name
- `workspaceId` (string) - Workspace ID

Optional Parameters:
- `dueDate` (ISO 8601 date)
- `description` (string, HTML accepted)
- `labels` (array of strings)
- `priority` (string, default: MEDIUM) - Options: ASAP, HIGH, MEDIUM, LOW
- `projectDefinitionId` (string)
- `stages` (array of stage objects)

### Recurring Tasks

#### Delete Recurring Task

**Endpoint:** `DELETE /v1/recurring-tasks/{id}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **id** (integer) - ID of the recurring task to delete

#### List Recurring Tasks

**Endpoint:** `GET /v1/recurring-tasks`

**Authorization:**
- **X-API-Key** (string, required) - API key in header

**Query Parameters:**
- `cursor` (string) - Pagination cursor
- `workspaceId` (string, required) - Workspace ID for recurring tasks

**Response (200 OK):**

Meta Object:
- `nextCursor` (string) - Cursor for next page
- `pageSize` (number) - Number of results

Tasks Array:
- `id` (string)
- `name` (string)
- `creator` (object)
- `assignee` (object)
- `project` (optional object)
- `status` (object)
- `priority` (string: ASAP, HIGH, MEDIUM, or LOW)
- `labels` (array)
- `workspace` (object)

#### Create Recurring Task

**Endpoint:** `POST /v1/recurring-tasks`

**Authorization:**
- `X-API-Key` (string, required) - API key in header

**Request Body (application/json):**

Required fields:
- `frequency` - Task scheduling frequency
- `name` - Recurring task name
- `workspaceId` - Workspace for task creation
- `assigneeId` - User assigned to task

Optional fields:
- `dueDate` (datetime) - ISO 8601 Due date on the task. REQUIRED for scheduled tasks.
- `duration` (string | number) - "NONE", "REMINDER", or integer > 0 (minutes)
- `status` (string) - Defaults to workspace default status
- `autoScheduled` (object | null) - Set values to enable auto scheduling, null to disable
  - `startDate` (datetime) - ISO 8601 date trimmed to start of day
  - `deadlineType` (string) - "HARD", "SOFT", or "NONE" (default: SOFT)
  - `schedule` (string) - Schedule name (default: "Work Hours")
- `description` (string) - Github Flavored Markdown
- `priority` (string) - "ASAP", "HIGH", "MEDIUM", "LOW"
- `labels` (array<string>) - Label names
- `assigneeId` (string) - User ID to assign task to
- `projectId` (string) - Project ID to associate with

### Schedules

#### Get Schedules

**Endpoint:** `GET /v1/schedules`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Response (200 - application/json):**

Schedule Properties:
- `name` (string) - Name of the schedule
- `isDefaultTimezone` (boolean) - Whether it's the default timezone
- `timezone` (string) - Timezone of the schedule
- `schedule` (object) - Schedule details

Daily Schedule Structure:
Each day (monday-sunday) contains an array of objects with:
- `start` (string) - Schedule start time (HH:MM)
- `end` (string) - Schedule end time (HH:MM)

### Statuses

#### Get Statuses

**Endpoint:** `GET /v1/statuses`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Query Parameters:**
- **workspaceId** (string) - Get statuses for a particular workspace

**Response (200 - application/json - array):**

Status Object:
- **name** (string) - The name of the status
- **isDefaultStatus** (boolean) - Whether this status is a default status for the workspace
- **isResolvedStatus** (boolean) - Whether this is a resolved (terminated) status for the workspace

### Tasks

#### Delete Task

**Endpoint:** `DELETE /v1/tasks/{id}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **id** (integer) - ID of the task to delete

#### Get Task

**Endpoint:** `GET /v1/tasks/{id}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- `id` (string) - ID of the task to fetch

**Response (200 OK, application/json):**

Key Response Fields:
- `id` - Task ID (string, required)
- `name` - Task name (string, required)
- `description` - HTML description contents (string, required)
- `duration` - Task duration (string/number)
- `dueDate` - Task due datetime
- `deadlineType` - "HARD", "SOFT" (default), or "NONE"
- `completed` - Whether task is completed (boolean)
- `creator` - Object with creator details
- `project` - Object with project information
- `workspace` - Object with workspace data
- `status` - Task status object
- `priority` - "ASAP", "HIGH", "MEDIUM", or "LOW"
- `assignees` - Array of assigned users
- `customFieldValues` - Custom field data record

#### List Tasks

**Endpoint:** `GET /v1/tasks`

**Authorization:**
- `X-API-Key` (string, required) - API key header

**Query Parameters:**
- `assigneeId` (string) - Limit tasks to specific assignee
- `cursor` (string) - Page through results
- `includeAllStatuses` (boolean) - Include all task statuses
- `label` (string) - Filter by task label
- `name` (string) - Search tasks by name (case-insensitive)
- `projectId` (string) - Limit tasks to specific project
- `status` (array<string>) - Filter by task statuses
- `workspaceId` (string) - Specify workspace for tasks

**Response (200 OK):**

Meta Object:
- `nextCursor` (string) - Cursor for next page of results
- `pageSize` (number) - Number of results in response

Tasks Array:
- `id` (string)
- `name` (string)
- `description` (string)
- `dueDate` (datetime)
- `completed` (boolean)
- `creator` (object)
- `project` (object)
- `workspace` (object)
- `status` (object)
- `priority` (string)
- `assignees` (array)
- `customFieldValues` (record)

#### Move Task

**Endpoint:** `PATCH /v1/tasks/{id}/move`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- **id** (string, required) - Task ID

**Request Body (application/json):**
- **workspaceId** (string, required) - Destination workspace ID
- **assigneeId** (string, optional) - User ID to assign task

#### Update Task

**Endpoint:** `PATCH /v1/tasks/{id}`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Path Parameters:**
- `id` (string) - Task ID to update

**Request Body (application/json):**

Optional Fields:
- `dueDate` (datetime) - ISO 8601 due date
- `duration` (string/number) - "NONE", "REMINDER", or minutes
- `status` (string) - Workspace default status
- `autoScheduled` (object/null)
- `name` (string, required) - Task title
- `projectId` (string) - Project association
- `workspaceId` (string, required)
- `description` (string) - GitHub Flavored Markdown
- `priority` (string) - ASAP, HIGH, MEDIUM, LOW
- `labels` (array<string>)
- `assigneeId` (string)

#### Create Task

**Endpoint:** `POST /v1/tasks`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Request Body (application/json):**

Required Parameters:
- `name` (string) - Task title
- `workspaceId` (string) - Workspace ID

Optional Parameters:
- `dueDate` (datetime) - ISO 8601 Due date. REQUIRED for scheduled tasks
- `duration` (string | number) - "NONE", "REMINDER", or integer > 0 (minutes)
- `status` (string) - Workspace default status
- `autoScheduled` (object | null) - Auto-scheduling configuration
  - `startDate` (datetime) - ISO 8601 date trimmed to start of day
  - `deadlineType` (string) - "HARD", "SOFT", or "NONE" (default: SOFT)  
  - `schedule` (string) - Schedule name (default: "Work Hours")
- `projectId` (string) - Project association
- `description` (string) - GitHub Flavored Markdown
- `priority` (string) - ASAP, HIGH, MEDIUM, LOW
- `labels` (array<string>) - Label names
- `assigneeId` (string) - User ID to assign task to

#### Unassign Task

**Endpoint:** `DELETE /v1/tasks/{id}/assignee`

**Authorization:**
- **X-API-Key** (string, required) - Header with the API key

**Path Parameters:**
- **id** (string, required) - The ID of the task

### Users

#### List Users

**Endpoint:** `GET /v1/users`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Query Parameters:**
- `cursor` (string) - Pagination cursor
- `teamId` (string) - Team ID to list members
- `workspaceId` (string) - Workspace ID to list members

**Response (200 OK):**

Meta Object:
- `nextCursor` (string) - Cursor for next page
- `pageSize` (number) - Number of results

Users Array:
- `id` (string, required) - User ID
- `name` (string) - User name
- `email` (string, required) - User email

#### Get My User

**Endpoint:** `GET /v1/users/me`

**Authorization:**
- **X-API-Key** (string, required) - Header with the API key

**Response (200 - application/json):**
- `id` (string, required) - User ID
- `name` (string) - User name
- `email` (string, required) - User email

### Workspaces

#### List Workspaces

**Endpoint:** `GET /v1/workspaces`

**Authorization:**
- **X-API-Key** (string, required) - Header with API key

**Query Parameters:**
- `cursor` (string) - Page through results
- `ids` (array<string>) - Expand details of specific workspaces

**Response (200 - application/json):**

Meta:
- `nextCursor` (string) - Cursor for next page of results
- `pageSize` (number) - Number of results in response

Workspaces (array of objects):
- `id` (string) - Workspace ID
- `name` (string) - Workspace name
- `teamId` (string) - Team ID
- `type` (string) - Workspace type (team or individual)
- `labels` (array of objects)
- `statuses` (array of objects)

---

## Base API URL

All API endpoints are accessed via: `https://api.usemotion.com`

## Authentication

All API requests require authentication via the `X-API-Key` header:

```
X-API-Key: your_api_key_here
```

## Rate Limits

- **Individual tier**: 12 requests per minute
- **Teams**: 120 requests per minute
- **Enterprise**: Higher limits available

## Common Response Patterns

### Pagination

Many endpoints support pagination using cursor-based pagination:

```json
{
  "meta": {
    "nextCursor": "string",
    "pageSize": 50
  },
  "data": []
}
```

### Error Responses

API errors follow standard HTTP status codes with JSON error messages.

---

*Documentation generated from Motion API docs at https://docs.usemotion.com/*