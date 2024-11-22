mod debuggee;

use std::{thread::sleep, time::Duration};

use anyhow::anyhow;
use clap::Parser;
use libc::pid_t;
use nix::unistd::Pid;
use nonempty::NonEmpty;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

#[derive(Debug, clap::Parser)]
struct Cli {
    #[arg(long, short = 'p')]
    pid: Option<pid_t>,

    #[arg(long, short = 'v')]
    verbose: bool,

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

    let debuggee_config = match (cli.pid, cli.child_args.len()) {
        (Some(pid), 0) => debuggee::Config::Existing(Pid::from_raw(pid)),
        (None, _) => {
            let child_args =
                NonEmpty::from_vec(cli.child_args).ok_or(anyhow!("no child argument provided"))?;
            debuggee::Config::SpawnChild(child_args)
        }
        (Some(_), _) => Err(anyhow!("ambiguous debuggee config"))?,
    };

    let mut debuggee = debuggee::Debuggee::new(debuggee_config)?;
    debuggee.resume()?;
    debuggee.wait_for_state_change()?;

    sleep(Duration::from_secs(10));

    return Ok(());
}
