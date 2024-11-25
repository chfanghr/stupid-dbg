use std::path::Path;

use anyhow::anyhow;
use clap::Parser as _;
use libc::pid_t;
use nix::unistd::Pid;
use nonempty::NonEmpty;
use rustyline::error::ReadlineError;
use tracing::{error, info, warn};

use crate::debuggee::{self, Debuggee};

#[derive(Debug, clap::Parser)]
#[command(multicall = true)]
struct CommandWrapper {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Attach { pid: pid_t },
    Run { args: Vec<String> },
    Detach,
    Continue,
    Quit,
}

pub enum CommandExecutionResult {
    Continue(anyhow::Result<()>),
    Quit(anyhow::Result<()>),
}

impl CommandExecutionResult {
    pub fn should_quit(&self) -> bool {
        if let Self::Quit(_) = self {
            true
        } else {
            false
        }
    }
}

pub struct Debugger {
    debuggee: Option<Debuggee>,
}

impl Debugger {
    pub fn new() -> Self {
        Self { debuggee: None }
    }

    pub fn handle_command(&mut self, command: Command) -> CommandExecutionResult {
        match command {
            Command::Attach { pid } => self.handle_attach(pid),
            Command::Run { args } => self.handle_run(args),
            Command::Detach => self.handle_detach(),
            Command::Continue => self.handle_continue(),
            Command::Quit => self.handle_quit(),
        }
    }

    fn handle_with_debuggee<F>(&mut self, action: &mut F) -> CommandExecutionResult
    where
        F: FnMut(&mut Debuggee) -> CommandExecutionResult,
    {
        match &mut self.debuggee {
            Some(debuggee) => action(debuggee),
            None => {
                warn!("no debuggee, do nothing");
                CommandExecutionResult::Continue(Ok(()))
            }
        }
    }

    fn handle_attach(&mut self, pid: pid_t) -> CommandExecutionResult {
        CommandExecutionResult::Continue(if self.debuggee.is_some() {
            warn!("use `detach` to detach from the current debuggee first");
            Ok(())
        } else {
            Debuggee::new(debuggee::Config::Existing(Pid::from_raw(pid))).map(move |debuggee| {
                self.debuggee = Some(debuggee);
            })
        })
    }

    fn handle_run(&mut self, args: Vec<String>) -> CommandExecutionResult {
        CommandExecutionResult::Continue(if self.debuggee.is_some() {
            warn!("use `detach` to detach from the current debuggee first");
            Ok(())
        } else {
            let inner = move || -> anyhow::Result<()> {
                let args = NonEmpty::from_vec(args).ok_or(anyhow!("no child argument provided"))?;
                Debuggee::new(debuggee::Config::SpawnChild(args)).map(move |debuggee| {
                    self.debuggee = Some(debuggee);
                })
            };

            inner()
        })
    }

    fn handle_detach(&mut self) -> CommandExecutionResult {
        if self.debuggee.is_none() {
            warn!("no debuggee, do nothing")
        }
        self.debuggee = None;
        CommandExecutionResult::Continue(Ok(()))
    }

    fn handle_continue(&mut self) -> CommandExecutionResult {
        self.handle_with_debuggee(&mut move |debuggee| {
            let mut inner = || -> anyhow::Result<()> {
                debuggee.resume()?;
                debuggee.update_process_state(true)?;
                info!(process_state = %debuggee.process_state());
                Ok(())
            };

            CommandExecutionResult::Continue(inner())
        })
    }

    fn handle_quit(&self) -> CommandExecutionResult {
        CommandExecutionResult::Quit(Ok(()))
    }

    fn repl_line(&mut self, line: &str) -> CommandExecutionResult {
        let parse_command = move || -> anyhow::Result<Command> {
            let args = shlex::split(&line).ok_or(anyhow!("invalid quoting in command"))?;
            let command_wrapped = CommandWrapper::try_parse_from(args)?;
            Ok(command_wrapped.command)
        };

        match parse_command() {
            Err(err) => CommandExecutionResult::Continue(Err(err)),
            Ok(command) => self.handle_command(command),
        }
    }

    pub fn repl<T>(&mut self, history_file: Option<T>) -> anyhow::Result<()>
    where
        T: AsRef<Path>,
    {
        let mut rl = rustyline::DefaultEditor::new()?;

        if let Some(Err(err)) = history_file
            .as_ref()
            .map(|history_file| rl.load_history(history_file.as_ref()))
        {
            warn!(
                error = Box::<dyn std::error::Error + 'static>::from(err),
                "unable to load history file"
            )
        }

        loop {
            match rl.readline("dbg> ") {
                Ok(line) => {
                    if line.is_empty() {
                        continue;
                    }
                    _ = rl.add_history_entry(&line);
                    let result = self.repl_line(&line);
                    let should_quit = result.should_quit();
                    match result {
                        CommandExecutionResult::Continue(Err(err))
                        | CommandExecutionResult::Quit(Err(err)) => {
                            error!(
                                error = Box::<dyn std::error::Error + 'static>::from(err),
                                "failed to execute command"
                            )
                        }
                        _ => (),
                    }
                    if should_quit {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    info!("ctrl-c");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    info!("ctrl-d");
                    break;
                }
                Err(err) => {
                    error!(
                        error = Box::<dyn std::error::Error + 'static>::from(err),
                        "unknown readline error"
                    );
                    break;
                }
            };
        }

        if let Some(Err(err)) = history_file
            .as_ref()
            .map(|history_file| rl.save_history(history_file.as_ref()))
        {
            warn!(
                error = Box::<dyn std::error::Error + 'static>::from(err),
                "unable to save history file"
            )
        }

        Ok(())
    }
}
