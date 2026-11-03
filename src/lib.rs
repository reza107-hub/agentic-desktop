//! The command-line program as a library, so the C library can run the same commands.

use agentic_desktop_core::fixture::FixtureAdapter;
use agentic_desktop_core::{
    Action, AppError, Envelope, ErrorCode, PlatformAdapter, SnapshotStore, commands,
};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::ffi::OsString;

#[derive(Parser)]
#[command(
    name = "agentic-desktop",
    version,
    about = "Let AI agents see and operate desktop apps"
)]
struct Cli {
    /// Allow commands that move the real mouse or press real keys
    #[arg(long, global = true)]
    headed: bool,
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
    /// Take a PNG screenshot of the whole screen
    Screenshot {
        /// Where to save it. Defaults to the state folder. An existing file is never overwritten
        #[arg(long)]
        out: Option<String>,
    },
    /// Print the clipboard text (it may be private)
    ClipboardGet,
    /// Replace the clipboard text
    ClipboardSet { text: String },
    /// Empty the clipboard
    ClipboardClear,
    /// Click an element by ref
    Click { target: String },
    /// Type text into an element by ref
    Type { target: String, text: String },
    /// Replace an element's value by ref
    SetValue { target: String, value: String },
    /// Press a key combination on the real keyboard, like ctrl+c (needs --headed)
    Press { combo: String },
    /// Move the real mouse pointer to a screen position (needs --headed)
    #[command(allow_negative_numbers = true)]
    MouseMove { x: f64, y: f64 },
    /// Click the real mouse at a screen position (needs --headed)
    #[command(allow_negative_numbers = true)]
    MouseClick {
        x: f64,
        y: f64,
        #[arg(long, default_value = "left")]
        button: String,
        #[arg(long, default_value_t = 1)]
        count: u8,
    },
    /// Scroll the real mouse wheel; positive --dy scrolls down (needs --headed)
    #[command(allow_negative_numbers = true)]
    MouseWheel {
        #[arg(long, default_value_t = 0)]
        dx: i32,
        #[arg(long, default_value_t = 0)]
        dy: i32,
    },
}

impl Commands {
    fn name(&self) -> &'static str {
        match self {
            Commands::Version => "version",
            Commands::Status => "status",
            Commands::ListApps => "list-apps",
            Commands::Snapshot { .. } => "snapshot",
            Commands::Screenshot { .. } => "screenshot",
            Commands::ClipboardGet => "clipboard-get",
            Commands::ClipboardSet { .. } => "clipboard-set",
            Commands::ClipboardClear => "clipboard-clear",
            Commands::Click { .. } => "click",
            Commands::Type { .. } => "type",
            Commands::SetValue { .. } => "set-value",
            Commands::Press { .. } => "press",
            Commands::MouseMove { .. } => "mouse-move",
            Commands::MouseClick { .. } => "mouse-click",
            Commands::MouseWheel { .. } => "mouse-wheel",
        }
    }

    /// Commands that act on the real keyboard or mouse, and so can hit whatever is on screen.
    fn needs_headed(&self) -> bool {
        matches!(
            self,
            Commands::Press { .. }
                | Commands::MouseMove { .. }
                | Commands::MouseClick { .. }
                | Commands::MouseWheel { .. }
        )
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

fn check_policy(command: &Commands, headed: bool) -> Result<(), AppError> {
    if command.needs_headed() && !headed {
        return Err(AppError::new(
            ErrorCode::PolicyDenied,
            format!(
                "'{}' controls the real keyboard or mouse, so it needs --headed",
                command.name()
            ),
        )
        .with_suggestion("Add --headed only when you want input sent to whatever is on screen"));
    }
    Ok(())
}

fn act(adapter: &dyn PlatformAdapter, target: &str, action: Action) -> Result<Value, AppError> {
    commands::act_on_ref(adapter, &SnapshotStore::from_env()?, target, action)
}

fn run_command(command: &Commands) -> Result<Value, AppError> {
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
        Commands::Screenshot { out } => {
            commands::screenshot(adapter, &SnapshotStore::from_env()?, out.as_deref())
        }
        Commands::ClipboardGet => commands::clipboard_get(adapter),
        Commands::ClipboardSet { text } => commands::clipboard_set(adapter, text),
        Commands::ClipboardClear => commands::clipboard_clear(adapter),
        Commands::Click { target } => act(adapter, target, Action::Click),
        Commands::Type { target, text } => act(adapter, target, Action::Type(text.clone())),
        Commands::SetValue { target, value } => {
            act(adapter, target, Action::SetValue(value.clone()))
        }
        Commands::Press { combo } => commands::press(adapter, combo),
        Commands::MouseMove { x, y } => commands::mouse_move(adapter, *x, *y),
        Commands::MouseClick {
            x,
            y,
            button,
            count,
        } => commands::mouse_click(adapter, *x, *y, button, *count),
        Commands::MouseWheel { dx, dy } => commands::mouse_wheel(adapter, *dx, *dy),
    }
}

/// Run one command line (`args[0]` is the program name) and return its JSON envelope.
/// A bad command line comes back as a clap error, so the caller decides how to show it.
pub fn run<I, T>(args: I) -> Result<Envelope, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args)?;
    let name = cli.command.name();
    let result = check_policy(&cli.command, cli.headed).and_then(|()| run_command(&cli.command));
    Ok(Envelope::from_result(name, result))
}

/// Like `run`, but a bad command line is also an envelope. Used by the C library.
pub fn run_to_envelope<I, T>(args: I) -> Envelope
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match run(args) {
        Ok(envelope) => envelope,
        Err(error) => Envelope::failure(
            "parse",
            &AppError::invalid_args(error.to_string().trim().to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_fixture(args: &[&str]) -> Envelope {
        // Safe in tests: the fixture variable is only ever set, never unset, and is read-only after.
        unsafe { std::env::set_var("AGENTIC_DESKTOP_FIXTURE", "1") };
        run_to_envelope(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn input_commands_need_headed() {
        for args in [
            vec!["agentic-desktop", "press", "enter"],
            vec!["agentic-desktop", "mouse-move", "1", "2"],
            vec!["agentic-desktop", "mouse-click", "1", "2"],
            vec!["agentic-desktop", "mouse-wheel", "--dy", "3"],
        ] {
            let envelope = run_fixture(&args);
            assert!(!envelope.ok, "{args:?} should be refused");
            assert_eq!(
                envelope.error.as_ref().unwrap().code,
                ErrorCode::PolicyDenied
            );
        }
    }

    #[test]
    fn headed_input_commands_run() {
        let envelope = run_fixture(&["agentic-desktop", "--headed", "press", "ctrl+c"]);
        assert!(envelope.ok);
        let envelope = run_fixture(&["agentic-desktop", "mouse-move", "--headed", "-0", "5"]);
        assert!(envelope.ok);
    }

    #[test]
    fn bad_command_lines_become_envelopes() {
        let envelope = run_fixture(&["agentic-desktop", "no-such-command"]);
        assert!(!envelope.ok);
        assert_eq!(envelope.error.unwrap().code, ErrorCode::InvalidArgs);
    }

    #[test]
    fn read_only_commands_need_no_flag() {
        assert!(run_fixture(&["agentic-desktop", "version"]).ok);
        assert!(run_fixture(&["agentic-desktop", "list-apps"]).ok);
    }
}
