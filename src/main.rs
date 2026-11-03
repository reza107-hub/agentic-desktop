use agentic_desktop_core::fixture::FixtureAdapter;
use agentic_desktop_core::{Action, AppError, Envelope, PlatformAdapter, SnapshotStore, commands};
use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Parser)]
#[command(
    name = "agentic-desktop",
    version,
    about = "Let AI agents see and operate desktop apps"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print the version
    Version,
    /// Show the platform and backend in use
    Status,
    /// List apps that can be observed
    ListApps,
    /// Read an app's accessibility tree and give each actionable element a ref
    Snapshot {
        #[arg(long)]
        app: String,
        /// Only show elements with refs, and the path to them
        #[arg(short = 'i', long)]
        interactive: bool,
    },
    /// Click an element by ref
    Click { target: String },
    /// Type text into an element by ref
    Type { target: String, text: String },
    /// Replace an element's value by ref
    SetValue { target: String, value: String },
}

impl Commands {
    fn name(&self) -> &'static str {
        match self {
            Commands::Version => "version",
            Commands::Status => "status",
            Commands::ListApps => "list-apps",
            Commands::Snapshot { .. } => "snapshot",
            Commands::Click { .. } => "click",
            Commands::Type { .. } => "type",
            Commands::SetValue { .. } => "set-value",
        }
    }
}

fn build_adapter() -> Box<dyn PlatformAdapter> {
    if std::env::var_os("AGENTIC_DESKTOP_FIXTURE").is_some() {
        return Box::new(FixtureAdapter::new());
    }
    #[cfg(target_os = "linux")]
    return Box::new(agentic_desktop_linux::LinuxAdapter::new());
    #[cfg(target_os = "macos")]
    return Box::new(agentic_desktop_macos::MacOsAdapter::new());
    #[cfg(target_os = "windows")]
    return Box::new(agentic_desktop_windows::WindowsAdapter::new());
}

fn run(command: &Commands) -> Result<Value, AppError> {
    if matches!(command, Commands::Version) {
        return commands::version();
    }
    let adapter = build_adapter();
    let adapter = adapter.as_ref();
    match command {
        Commands::Version => commands::version(),
        Commands::Status => commands::status(adapter),
        Commands::ListApps => commands::list_apps(adapter),
        Commands::Snapshot { app, interactive } => {
            commands::take_snapshot(adapter, &SnapshotStore::from_env()?, app, *interactive)
        }
        Commands::Click { target } => act(adapter, target, Action::Click),
        Commands::Type { target, text } => act(adapter, target, Action::Type(text.clone())),
        Commands::SetValue { target, value } => {
            act(adapter, target, Action::SetValue(value.clone()))
        }
    }
}

fn act(adapter: &dyn PlatformAdapter, target: &str, action: Action) -> Result<Value, AppError> {
    commands::act_on_ref(adapter, &SnapshotStore::from_env()?, target, action)
}

fn main() {
    let cli = Cli::parse();
    let envelope = Envelope::from_result(cli.command.name(), run(&cli.command));
    match serde_json::to_string(&envelope) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("failed to encode output: {error}"),
    }
    std::process::exit(envelope.exit_code());
}
