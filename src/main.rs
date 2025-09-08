use clap::Parser;
use linear_motion::cli::commands::{Cli, Commands};
use linear_motion::{Error, Result};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| Error::Other(format!("Failed to set tracing subscriber: {}", e)))?;

    info!("Starting sync-tool");

    match cli.command {
        Commands::Init { output, force } => {
            handle_init(output.as_deref(), force).await?;
        }
        Commands::Sync { watch, pid_file } => {
            handle_sync(cli.config.as_deref(), watch, &pid_file).await?;
        }
        Commands::Status => {
            handle_status().await?;
        }
        Commands::Stop => {
            handle_stop().await?;
        }
        Commands::List { verbose, source } => {
            handle_list(cli.config.as_deref(), verbose, source.as_deref()).await?;
        }
    }

    Ok(())
}

async fn handle_init(output: Option<&str>, force: bool) -> Result<()> {
    use linear_motion::config::{
        AppConfig, ConfigLoader, ScheduleOverride, SyncRules, SyncSource, TimeEstimateStrategy,
    };
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    let output_path = match output {
        Some(path) => Path::new(path).to_path_buf(),
        None => {
            ConfigLoader::ensure_config_dir()?;
            ConfigLoader::get_default_config_path()?
        }
    };

    if output_path.exists() && !force {
        return Err(Error::Other(format!(
            "Config file {} already exists. Use --force to overwrite.",
            output_path.display()
        )));
    }

    // Create Fibonacci mapping
    let mut fibonacci_mappings = HashMap::new();
    fibonacci_mappings.insert("1".to_string(), 30);
    fibonacci_mappings.insert("2".to_string(), 60);
    fibonacci_mappings.insert("3".to_string(), 120);
    fibonacci_mappings.insert("5".to_string(), 240);
    fibonacci_mappings.insert("8".to_string(), 480);

    // Create T-shirt mapping
    let mut tshirt_mappings = HashMap::new();
    tshirt_mappings.insert("XS".to_string(), 30);
    tshirt_mappings.insert("S".to_string(), 60);
    tshirt_mappings.insert("M".to_string(), 120);
    tshirt_mappings.insert("L".to_string(), 240);
    tshirt_mappings.insert("XL".to_string(), 480);

    let time_estimate_strategy = TimeEstimateStrategy {
        fibonacci: Some(fibonacci_mappings),
        tshirt: Some(tshirt_mappings),
        linear: None,
        points: None,
        default_duration_mins: Some(60),
    };

    let sync_rules = SyncRules {
        default_task_duration_mins: 60,
        completed_linear_tag: "motioned".to_string(),
        time_estimate_strategy: time_estimate_strategy.clone(),
    };

    let sync_source = SyncSource {
        name: "my-linear-workspace".to_string(),
        linear_api_key: "your_linear_api_key_here".to_string(),
        projects: Some(vec!["project-id-1".to_string(), "project-id-2".to_string()]),
        webhook_base_url: None,
        sync_rules: Some(sync_rules.clone()),
    };

    let schedule_override = ScheduleOverride {
        name: "work_hours".to_string(),
        interval_seconds: 60,
        start_time: "09:00".to_string(),
        end_time: "17:00".to_string(),
        days: vec![
            "mon".to_string(),
            "tue".to_string(),
            "wed".to_string(),
            "thu".to_string(),
            "fri".to_string(),
        ],
    };

    let template_config = AppConfig {
        motion_api_key: "your_motion_api_key_here".to_string(),
        sync_sources: vec![sync_source],
        global_sync_rules: sync_rules,
        database_path: None,
        polling_interval_seconds: 300,
        schedule_overrides: Some(vec![schedule_override]),
    };

    // Serialize to pretty JSON
    let config_json = serde_json::to_string_pretty(&template_config)?;

    fs::write(&output_path, config_json)?;
    info!(
        "Configuration template written to {}",
        output_path.display()
    );
    println!(
        "✅ Configuration template created at {}",
        output_path.display()
    );
    println!("📝 Please edit the file to add your API keys and configure sync sources.");

    Ok(())
}

async fn handle_sync(config_path: Option<&str>, watch: bool, pid_file: &str) -> Result<()> {
    use linear_motion::config::ConfigLoader;
    use linear_motion::sync::orchestrator::SyncOrchestrator;
    use std::fs;

    let config_path = match config_path {
        Some(path) => path.to_string(),
        None => ConfigLoader::get_default_config_path()?
            .to_string_lossy()
            .to_string(),
    };

    // Load and validate configuration
    let config = ConfigLoader::load_from_file(&config_path).await?;
    info!(
        "Configuration loaded: {} sync sources configured",
        config.sync_sources.len()
    );

    if watch {
        info!("Starting daemon mode");

        // Write PID file
        let pid = std::process::id();
        fs::write(pid_file, pid.to_string())?;
        info!("Daemon PID {} written to {}", pid, pid_file);

        // TODO: Start daemon loop
        println!("🚀 Daemon started in watch mode (PID: {})", pid);
        println!("📄 PID file: {}", pid_file);
        println!("📊 {} sync sources configured", config.sync_sources.len());
        println!("💾 Database: {:?}", config.database_path());
        println!("⚠️  Daemon functionality not yet implemented");
    } else {
        info!("Running one-time sync");
        println!("🔄 Running one-time sync...");
        println!("📊 {} sync sources configured", config.sync_sources.len());
        println!("💾 Database: {:?}", config.database_path());

        // Initialize and run sync orchestrator
        let mut orchestrator = SyncOrchestrator::new(&config).await?;
        match orchestrator.run_full_sync(&config).await {
            Ok(()) => {
                println!("✅ Sync completed successfully!");
            }
            Err(e) => {
                println!("❌ Sync failed: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

async fn handle_status() -> Result<()> {
    // TODO: Implement IPC client to query daemon
    println!("📊 Status functionality not yet implemented");
    println!("⚠️  Will query daemon via IPC when implemented");
    Ok(())
}

async fn handle_stop() -> Result<()> {
    // TODO: Implement daemon shutdown via IPC or signal
    println!("🛑 Stop functionality not yet implemented");
    println!("⚠️  Will send shutdown signal to daemon when implemented");
    Ok(())
}

async fn handle_list(
    config_path: Option<&str>,
    verbose: bool,
    source_filter: Option<&str>,
) -> Result<()> {
    use linear_motion::config::ConfigLoader;
    use linear_motion::db::SyncDatabase;

    let config_path = match config_path {
        Some(path) => path.to_string(),
        None => ConfigLoader::get_default_config_path()?
            .to_string_lossy()
            .to_string(),
    };

    // Load configuration to get database path
    let config = ConfigLoader::load_from_file(&config_path).await?;

    // Initialize database
    let database = SyncDatabase::new(config.database_path()).await?;

    println!("📊 Linear-Motion Database Contents");
    println!("💾 Database: {}", config.database_path().display());
    println!();

    // Get and display task mappings
    let mappings = match source_filter {
        Some(source) => database.mappings.list_mappings_by_source(source).await?,
        None => database.mappings.list_all_mappings().await?,
    };

    if mappings.is_empty() {
        println!("📭 No task mappings found");
        if let Some(source) = source_filter {
            println!("   (filtered by source: {})", source);
        }
    } else {
        println!("🔗 Task Mappings ({} total)", mappings.len());
        if let Some(source) = source_filter {
            println!("   (filtered by source: {})", source);
        }
        println!();

        for mapping in &mappings {
            // Parse Linear issue data from the mapping
            let issue_title = if let Ok(issue) = serde_json::from_value::<
                linear_motion::clients::linear::LinearIssue,
            >(mapping.linear_issue_data.clone())
            {
                format!("{} - {}", issue.identifier, issue.title)
            } else {
                mapping.linear_issue_id.clone()
            };

            let status_icon = match mapping.status {
                linear_motion::db::MappingStatus::Pending => "⏳",
                linear_motion::db::MappingStatus::Synced => "✅",
                linear_motion::db::MappingStatus::Failed => "❌",
                linear_motion::db::MappingStatus::Stale => "🔄",
            };

            let status_str = match mapping.status {
                linear_motion::db::MappingStatus::Pending => "Pending",
                linear_motion::db::MappingStatus::Synced => "Synced",
                linear_motion::db::MappingStatus::Failed => "Failed",
                linear_motion::db::MappingStatus::Stale => "Stale",
            };

            println!("  {} {}", status_icon, issue_title);
            println!("    Source: {}", mapping.sync_source);
            println!("    Status: {}", status_str);
            if let Some(motion_id) = &mapping.motion_task_id {
                println!("    Motion Task: {}", motion_id);
            }
            println!(
                "    Created: {}",
                mapping.created_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            println!(
                "    Updated: {}",
                mapping.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
            );

            if verbose {
                println!("    Linear ID: {}", mapping.linear_issue_id);
                if let Some(attempt) = &mapping.last_sync_attempt {
                    println!("    Last Sync: {}", attempt.format("%Y-%m-%d %H:%M:%S UTC"));
                }
                if let Some(error) = &mapping.sync_error {
                    println!("    Sync Error: {}", error);
                }

                // Show issue details if we have them
                if let Ok(issue) = serde_json::from_value::<
                    linear_motion::clients::linear::LinearIssue,
                >(mapping.linear_issue_data.clone())
                {
                    if let Some(desc) = &issue.description {
                        let truncated_desc = if desc.len() > 100 {
                            format!("{}...", &desc[..100])
                        } else {
                            desc.clone()
                        };
                        println!("    Description: {}", truncated_desc);
                    }
                    println!(
                        "    State: {} ({})",
                        issue.state.name, issue.state.state_type
                    );
                    println!("    Team: {} ({})", issue.team.name, issue.team.key);
                    if let Some(due_date) = &issue.due_date {
                        println!("    Due Date: {}", due_date);
                    }
                }
            }
            println!();
        }
    }

    // Get and display sync status entries
    let status_entries = match source_filter {
        Some(source) => database.status.list_statuses_by_source(source).await?,
        None => {
            // Get all failed entries as an example - in a real implementation,
            // we might want to add a list_all_entries method
            let mut all_entries = Vec::new();
            let source_stats = database.status.list_all_source_stats().await?;
            for source_stat in source_stats {
                let source_entries = database
                    .status
                    .list_statuses_by_source(&source_stat.source_name)
                    .await?;
                all_entries.extend(source_entries);
            }
            all_entries
        }
    };

    if status_entries.is_empty() {
        println!("📭 No sync status entries found");
    } else {
        println!("📈 Sync Status Entries ({} total)", status_entries.len());
        println!();

        let mut by_status = std::collections::HashMap::new();
        for entry in &status_entries {
            let status_str = match entry.status {
                linear_motion::db::status::SyncStatus::InProgress => "InProgress",
                linear_motion::db::status::SyncStatus::Completed => "Completed",
                linear_motion::db::status::SyncStatus::Failed => "Failed",
                linear_motion::db::status::SyncStatus::Paused => "Paused",
            };
            *by_status.entry(status_str).or_insert(0) += 1;
        }

        for (status, count) in &by_status {
            let icon = match *status {
                "InProgress" => "🔄",
                "Completed" => "✅",
                "Failed" => "❌",
                "Paused" => "⏸️",
                _ => "❓",
            };
            println!("  {} {}: {}", icon, status, count);
        }
        println!();

        if verbose {
            println!("📝 Detailed Status Entries:");
            for entry in status_entries.iter().take(10) {
                // Limit to first 10 for readability
                let (icon, status_str) = match entry.status {
                    linear_motion::db::status::SyncStatus::InProgress => ("🔄", "InProgress"),
                    linear_motion::db::status::SyncStatus::Completed => ("✅", "Completed"),
                    linear_motion::db::status::SyncStatus::Failed => ("❌", "Failed"),
                    linear_motion::db::status::SyncStatus::Paused => ("⏸️", "Paused"),
                };

                println!(
                    "  {} {} - {}",
                    icon, entry.sync_source, entry.linear_issue_id
                );
                println!("     Status: {}", status_str);
                println!(
                    "     Created: {}",
                    entry.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                );
                println!(
                    "     Last Sync: {}",
                    entry.last_sync_attempt.format("%Y-%m-%d %H:%M:%S UTC")
                );
                if let Some(error) = &entry.error_message {
                    println!("     Error: {}", error);
                }
                if let Some(motion_id) = &entry.motion_task_id {
                    println!("     Motion Task: {}", motion_id);
                }
                println!("     Retry Count: {}", entry.retry_count);
                if verbose {
                    println!("     Entry ID: {}", entry.id);
                }
                println!();
            }

            if status_entries.len() > 10 {
                println!(
                    "  ... and {} more entries (use --source filter to see specific source)",
                    status_entries.len() - 10
                );
                println!();
            }
        }
    }

    // Display source statistics
    let source_stats = database.status.list_all_source_stats().await?;
    if !source_stats.is_empty() {
        println!("📊 Source Statistics:");
        for stat in &source_stats {
            println!("  • {}", stat.source_name);
            println!(
                "    Last sync: {}",
                if let Some(last_sync) = &stat.last_sync {
                    last_sync.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                } else {
                    "Never".to_string()
                }
            );
            println!("    Total processed: {}", stat.total_issues_processed);
            println!("    Successful syncs: {}", stat.successful_syncs);
            println!("    Failed syncs: {}", stat.failed_syncs);
            if !stat.errors.is_empty() {
                println!("    Recent errors: {}", stat.errors.len());
                if verbose {
                    for error in stat.errors.iter().take(3) {
                        println!("      - {}", error);
                    }
                }
            }
            println!();
        }
    }

    Ok(())
}
