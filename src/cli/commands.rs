use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sync-tool")]
#[command(about = "Linear-Motion sync tool for automating task synchronization")]
#[command(version = "1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, help = "Enable verbose logging")]
    pub verbose: bool,

    #[arg(
        short,
        long,
        help = "Path to configuration file (defaults to user config dir)"
    )]
    pub config: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize configuration file
    Init {
        #[arg(
            short,
            long,
            help = "Output path for config file (defaults to user config dir)"
        )]
        output: Option<String>,

        #[arg(short, long, help = "Force overwrite existing config")]
        force: bool,
    },

    /// Run sync operation
    Sync {
        #[arg(short, long, help = "Run as daemon in watch mode")]
        watch: bool,

        #[arg(
            short,
            long,
            help = "Daemon PID file path",
            default_value = ".sync-tool.pid"
        )]
        pid_file: String,
    },

    /// Query daemon status
    Status,

    /// Stop running daemon
    Stop,

    /// List all tracked issues and metadata in local database
    List {
        #[arg(short, long, help = "Show detailed information for each entry")]
        verbose: bool,

        #[arg(short, long, help = "Filter by sync source name")]
        source: Option<String>,
    },
}
