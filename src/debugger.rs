use std::path::Path;

use anyhow::anyhow;
use clap::Parser as _;
use rustyline::error::ReadlineError;
use tracing::{error, info, warn};

use crate::debuggee::Debuggee;

#[derive(Debug, clap::Parser)]
#[command(multicall = true)]
struct CommandWrapper {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
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
    debuggee: Debuggee,
}

impl Debugger {
    pub fn new(debuggee: Debuggee) -> Self {
        Self { debuggee }
    }

    pub fn handle_command(&mut self, command: Command) -> CommandExecutionResult {
        match command {
            Command::Continue => self.handle_continue(),
            Command::Quit => self.handle_quit(),
        }
    }

    fn handle_continue(&mut self) -> CommandExecutionResult {
        let mut inner = || -> anyhow::Result<()> {
            self.debuggee.resume()?;
            self.debuggee.update_process_state(true)?;
            info!(process_state = %self.debuggee.process_state());
            Ok(())
        };

        CommandExecutionResult::Continue(inner())
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
