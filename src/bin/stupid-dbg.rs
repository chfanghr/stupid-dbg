use std::path::PathBuf;

use anyhow::anyhow;
use clap::Parser;
use libc::pid_t;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

use stupid_dbg::debugger::{self, Debugger};

#[derive(Debug, clap::Parser)]
struct Cli {
    #[arg(long, short = 'p')]
    pid: Option<pid_t>,

    #[arg(long, short = 'v')]
    verbose: bool,

    #[arg(long)]
    history_file: Option<PathBuf>,

    child_args: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let collector = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .with_span_events(FmtSpan::CLOSE | FmtSpan::ENTER)
        .finish();
    tracing::subscriber::set_global_default(collector)
        .map_err(|err| anyhow!("unable to setup logging subscriber: {}", err))?;

    let mut debugger = Debugger::new();

    if let debugger::CommandExecutionResult::Quit(result) = match (cli.pid, cli.child_args.len()) {
        (Some(pid), 0) => debugger.handle_command(debugger::Command::Attach { pid }),
        (None, len) => {
            if len > 0 {
                debugger.handle_command(debugger::Command::Run {
                    args: cli.child_args,
                })
            } else {
                debugger::CommandExecutionResult::Continue(Ok(()))
            }
        }
        (Some(_), _) => Err(anyhow!("ambiguous debuggee config"))?,
    } {
        return result;
    }

    debugger.repl(cli.history_file)?;

    return Ok(());
}
