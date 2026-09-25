mod spec_file;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use service_kit_backends::{host_context, registry};
use service_kit_core::{Envelope, Error, ErrorCode, Operation, PriorityStrategy, Result, Scope};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    version,
    about = "Native service manager; JSON protocol v1. Inspect service plans and manage native services."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Probe {
        #[arg(long, value_enum, default_value = "user")]
        scope: ScopeArg,
    },
    Plan {
        #[command(subcommand)]
        operation: PlanCommand,
    },
    Install(Manage),
    Uninstall(Manage),
    Start(Manage),
    Stop(Manage),
    Restart(Manage),
    Status(Manage),
}
#[derive(Subcommand)]
enum PlanCommand {
    Install(Manage),
    Uninstall(Manage),
    Start(Manage),
    Stop(Manage),
    Restart(Manage),
    Status(Manage),
}
#[derive(clap::Args)]
struct Manage {
    #[arg(long, help = "Service specification (.toml, .json, .yaml or .yml)")]
    spec: PathBuf,
    #[arg(long, value_enum, default_value = "auto")]
    backend: BackendArg,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Auto-selection priority, e.g. systemd,launchd,scm"
    )]
    priority: Vec<String>,
    #[arg(long, help = "Return regenerated plan without mutations")]
    dry_run: bool,
}
#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
enum BackendArg {
    Auto,
    Systemd,
    Launchd,
    Scm,
    Openrc,
    Procd,
}
impl BackendArg {
    fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Systemd => "systemd",
            Self::Launchd => "launchd",
            Self::Scm => "scm",
            Self::Openrc => "openrc",
            Self::Procd => "procd",
        }
    }
}
#[derive(Clone, Copy, ValueEnum)]
enum ScopeArg {
    User,
    System,
}
impl From<ScopeArg> for Scope {
    fn from(v: ScopeArg) -> Self {
        match v {
            ScopeArg::User => Self::User,
            ScopeArg::System => Self::System,
        }
    }
}
fn manage(m: Manage, op: Operation, offline: bool) -> Result<Value> {
    if m.backend != BackendArg::Auto && !m.priority.is_empty() {
        return Err(Error::new(
            ErrorCode::InvalidArguments,
            "--priority requires --backend auto",
        ));
    }
    if offline && m.dry_run {
        return Err(Error::new(
            ErrorCode::InvalidArguments,
            "plan already describes actions; omit --dry-run",
        ));
    }
    let spec = spec_file::load_service_spec(&m.spec)?;
    let r = registry();
    let h = host_context();
    let backend = if m.backend != BackendArg::Auto {
        r.get(m.backend.name())?
    } else {
        let auto = if m.priority.is_empty() {
            PriorityStrategy::default()
        } else {
            PriorityStrategy {
                priorities: m.priority,
            }
        };
        r.select(&auto, op, &spec, &h)?
    };
    if offline || m.dry_run {
        return Ok(json!(backend.plan(&spec, op, &h)?));
    }
    Ok(json!(backend.apply(&spec, op, &h)?))
}
fn execute(c: Cli) -> Result<Value> {
    match c.command {
        Commands::Probe { scope } => {
            let h = host_context();
            Ok(json!({ "host":h,"candidates":registry().probes(&h,scope.into()) }))
        }
        Commands::Plan { operation } => match operation {
            PlanCommand::Install(m) => manage(m, Operation::Install, true),
            PlanCommand::Uninstall(m) => manage(m, Operation::Uninstall, true),
            PlanCommand::Start(m) => manage(m, Operation::Start, true),
            PlanCommand::Stop(m) => manage(m, Operation::Stop, true),
            PlanCommand::Restart(m) => manage(m, Operation::Restart, true),
            PlanCommand::Status(m) => manage(m, Operation::Status, true),
        },
        Commands::Install(m) => manage(m, Operation::Install, false),
        Commands::Uninstall(m) => manage(m, Operation::Uninstall, false),
        Commands::Start(m) => manage(m, Operation::Start, false),
        Commands::Stop(m) => manage(m, Operation::Stop, false),
        Commands::Restart(m) => manage(m, Operation::Restart, false),
        Commands::Status(m) => manage(m, Operation::Status, false),
    }
}
fn main() {
    let parsed = Cli::try_parse();
    let result = match parsed {
        Ok(c) => execute(c),
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                print!("{e}");
                return;
            }
            Err(Error::new(ErrorCode::InvalidArguments, e.to_string()))
        }
    };
    let failed = result.is_err();
    if let Err(e) = &result {
        eprintln!("{e}");
    }
    println!(
        "{}",
        serde_json::to_string(&Envelope::from_result(result)).expect("serializable envelope")
    );
    if failed {
        std::process::exit(1);
    }
}
