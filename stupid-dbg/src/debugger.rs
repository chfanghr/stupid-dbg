use std::path::Path;

use anyhow::anyhow;
use clap::Parser as _;
use libc::pid_t;
use nix::unistd::Pid;
use nonempty::NonEmpty;
use rustyline::error::ReadlineError;
use tracing::{error, info, warn};

use crate::{
    aux::{box_err, RlWithOpitonalHistoryFile},
    debuggee::{self, Debuggee, ProcessState},
    register::{Register, Registers},
};

#[derive(Debug, clap::Parser)]
#[command(multicall = true)]
struct CommandWrapper {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Attach {
        pid: pid_t,
    },
    Run {
        args: Vec<String>,
    },
    Detach,
    Continue,
    Register {
        #[command(subcommand)]
        command: RegisterCommand,
    },
    Quit,
}

#[derive(Debug, clap::Subcommand)]
pub enum RegisterCommand {
    Read { name: Option<String> },
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
            Command::Register { command } => self.handle_register_command(command),
            Command::Quit => self.handle_quit(),
        }
    }

    pub fn handle_register_command(&mut self, command: RegisterCommand) -> CommandExecutionResult {
        match command {
            RegisterCommand::Read { name } => self.handle_register_read(name.as_deref()),
        }
    }

    fn handle_with_debuggee_mut<F>(&mut self, action: &mut F) -> CommandExecutionResult
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

    fn handle_with_debuggee<F>(&self, action: F) -> CommandExecutionResult
    where
        F: FnOnce(&Debuggee) -> CommandExecutionResult,
    {
        match &self.debuggee {
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
        // TODO: move this to debuggee module
        fn pp_process_state(state: &ProcessState) {
            match state {
                ProcessState::Running => info!(process_state = %"running"),
                ProcessState::Stopped(signal) => {
                    if let Some(signal) = signal {
                        info!(process_state = %"stopped", signal = %signal)
                    } else {
                        info!("stopped")
                    }
                }
                ProcessState::Exited(status_code) => {
                    if let Some(status_code) = status_code {
                        info!(process_state = %"exited", status_code = status_code)
                    } else {
                        info!("exited")
                    }
                }
                ProcessState::Terminated(signal) => {
                    info!(process_state = %"terminated", signal = %signal)
                }
            }
        }

        self.handle_with_debuggee_mut(&mut move |debuggee| {
            let mut inner = || -> anyhow::Result<()> {
                debuggee.resume()?;
                debuggee.update_process_state(true)?;
                pp_process_state(&debuggee.process_state());
                Ok(())
            };

            CommandExecutionResult::Continue(inner())
        })
    }

    fn handle_register_read(&self, name: Option<&str>) -> CommandExecutionResult {
        // TODO: move all these to register module
        fn pp_register(registers: &Registers, register: Register) -> anyhow::Result<()> {
            let register_value = registers.read_register(register)?;

            info!(register = %register.name(), register_value = %register_value);

            Ok(())
        }

        fn pp_register_with_name(registers: &Registers, name: &str) -> anyhow::Result<()> {
            let register = Register::lookup_by_name(name)
                .ok_or(anyhow!("unable to find register with name: {}", name))?;
            pp_register(registers, register)
        }

        fn pp_all_registers(registers: &Registers) -> anyhow::Result<()> {
            Register::all_registers()
                .into_iter()
                .try_for_each(|reg| pp_register(registers, reg))?;

            Ok(())
        }

        self.handle_with_debuggee(|debuggee| {
            CommandExecutionResult::Continue(match debuggee.registers() {
                Some(registers) => match name {
                    Some(name) => pp_register_with_name(registers, name),
                    None => pp_all_registers(registers),
                },
                None => {
                    warn!("no register info available");
                    Ok(())
                }
            })
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
        let mut rl = RlWithOpitonalHistoryFile::new(history_file)?;

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
                            error!(error = box_err(err), "failed to execute command")
                        }
                        _ => (),
                    }
                    if should_quit {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    info!("use `quit` or ctrl+d to exit");
                }
                Err(ReadlineError::Eof) => {
                    info!("ctrl-d");
                    break;
                }
                Err(err) => {
                    error!(error = box_err(err), "unknown readline error");
                    break;
                }
            };
        }

        Ok(())
    }
}
