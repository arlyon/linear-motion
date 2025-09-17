# Linear-Motion Sync Tool

A command-line microservice that automatically synchronizes Linear issues with your Motion calendar, eliminating the manual effort of keeping your work items in sync between platforms.

## Overview

The Linear-Motion Sync Tool is designed for developers who use Linear for team project management and Motion for personal task and calendar management. It provides a robust, configurable pipeline to sync assigned Linear issues to your Motion calendar, ensuring your personal to-do list accurately reflects your work commitments.

### Key Features

- **Automated Task Creation**: Automatically creates Motion tasks from assigned Linear issues
- **Bidirectional Status Updates**: Marks Linear issues with completion tags when tasks are finished in Motion
- **Flexible Configuration**: Support for multiple Linear workspaces, custom time estimates, and sync schedules
- **Fault Tolerance**: Resilient to API failures with comprehensive error logging
- **Real-time Updates**: Webhook and polling support for immediate synchronization

## Installation

### Option 1: Cargo (Rust Package Manager)
```bash
cargo install linear-motion
```

### Option 2: Universal Binary Installer (ubi)
```bash
ubi --project arlyon/linear-motion
```

### Option 3: Mise with Cargo
```bash
mise use -g cargo:linear-motion
```

### Option 4: Mise with ubi
```bash
mise use -g ubi:arlyon/linear-motion
```

### Option 5: GitHub Releases
Download the latest binary from the [GitHub releases page](https://github.com/arlyon/linear-motion/releases).

## Quick Start

1. **Initialize configuration**:
   ```bash
   linear-motion init
   ```

2. **Edit the generated `config.json`** with your API keys and preferences

3. **Run a one-time sync**:
   ```bash
   linear-motion sync
   ```

4. **Start continuous background syncing**:
   ```bash
   linear-motion sync --watch
   ```

5. **Check sync status**:
   ```bash
   linear-motion status
   ```

## Configuration

The tool uses a `config.json` file for configuration. Here's the structure:

```json
{
  "motion_api_key": "your_motion_api_key",
  "sync_sources": [
    {
      "name": "My Team",
      "linear_api_key": "your_linear_api_key",
      "projects": ["PROJECT-1", "PROJECT-2"],
      "webhook_base_url": "https://your-domain.com/webhooks",
      "sync_rules": {
        "default_task_duration_mins": 60,
        "completed_linear_tag": "motioned",
        "time_estimate_strategy": {
          "fibonacci": {
            "1": 30,
            "2": 60,
            "3": 120,
            "5": 240,
            "8": 480
          },
          "default_duration_mins": 60
        }
      }
    }
  ],
  "global_sync_rules": {
    "default_task_duration_mins": 60,
    "completed_linear_tag": "motioned",
    "time_estimate_strategy": {
      "default_duration_mins": 60
    }
  },
  "polling_interval_seconds": 300,
  "schedule_overrides": [
    {
      "name": "Work Hours",
      "interval_seconds": 60,
      "start_time": "09:00",
      "end_time": "17:00",
      "days": ["mon", "tue", "wed", "thu", "fri"]
    }
  ]
}
```

### Configuration Options

- **motion_api_key**: Your Motion API key for task management
- **sync_sources**: Array of Linear workspace configurations
  - **name**: Friendly name for the sync source
  - **linear_api_key**: Linear API key for this workspace
  - **projects**: Optional list of specific Linear projects to sync
  - **webhook_base_url**: Optional webhook URL for real-time updates
  - **sync_rules**: Source-specific sync rules (overrides global rules)
- **global_sync_rules**: Default sync behavior
  - **default_task_duration_mins**: Default task duration when no estimate exists
  - **completed_linear_tag**: Tag applied to Linear issues when Motion tasks are completed
  - **time_estimate_strategy**: Mapping of Linear estimates to Motion durations
- **polling_interval_seconds**: How often to check for updates (default: 300 seconds)
- **schedule_overrides**: Different polling intervals for specific times/days

### Time Estimate Strategies

The tool supports multiple estimation systems:

- **Fibonacci**: Story point values (1, 2, 3, 5, 8, 13, etc.)
- **T-Shirt**: Size-based estimates (XS, S, M, L, XL)
- **Linear**: Linear's built-in estimation
- **Points**: Generic point-based system

## How It Works

1. **Initial Sync**: On startup, syncs all open assigned Linear issues to Motion
2. **Ongoing Updates**: Uses webhooks or polling to detect new/updated Linear issues
3. **Task Creation**: Creates Motion tasks with appropriate durations based on Linear estimates
4. **Completion Tracking**: Monitors Motion for completed tasks
5. **Bidirectional Update**: Tags completed Linear issues and removes them from sync

## Commands

- `linear-motion init` - Generate configuration template
- `linear-motion sync` - Run one-time sync
- `linear-motion sync --watch` - Start continuous background sync
- `linear-motion status` - Show current sync status and errors

## Use Cases

Perfect for developers who:
- Use Linear for sprint planning and issue tracking
- Use Motion for personal time management and calendar blocking
- Want to automate the tedious task of keeping both systems in sync
- Need accurate time blocking based on work estimates
- Want bidirectional status updates between systems

## Technical Details

- **Database**: Uses local `fjall` database for ID mappings and status tracking
- **API Integration**: Respects rate limits for both Linear and Motion APIs
- **Fault Tolerance**: Handles API failures gracefully with retry logic
- **Concurrency**: Single daemon process with IPC for status queries
- **Security**: API keys stored in local configuration file

## Contributing

Contributions are welcome! Please see the [contributing guidelines](CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.