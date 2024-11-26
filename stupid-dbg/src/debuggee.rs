use std::{
    convert::Infallible,
    ffi::CString,
    fs::File,
    io::{read_to_string, Write},
    ops::Not,
    os::fd::OwnedFd,
    process::exit,
};

use anyhow::anyhow;
use libc::EXIT_FAILURE;
use nix::{
    errno::Errno,
    fcntl::OFlag,
    sys::{
        ptrace::{self},
        signal::{kill, Signal},
        wait::{wait, waitpid, WaitPidFlag, WaitStatus},
    },
    unistd::{execvp, fork, pipe2, ForkResult, Pid},
};
use nonempty::NonEmpty;
use tracing::{debug, debug_span, error, info, warn};

use crate::{aux::box_err, register::Registers};

#[derive(Debug, Clone)]
pub enum ProcessState {
    Running,
    Stopped(Option<Signal>),
    Exited(Option<i32>),
    Terminated(Signal),
}

impl ProcessState {
    pub fn is_alive(&self) -> bool {
        match self {
            ProcessState::Running | ProcessState::Stopped(_) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessState::Running => write!(f, "running"),
            ProcessState::Stopped(signal) => {
                if let Some(signal) = signal {
                    write!(f, "stopped with signal: {signal}")
                } else {
                    write!(f, "stopped")
                }
            }
            ProcessState::Exited(status_code) => {
                if let Some(status_code) = status_code {
                    write!(f, "exited with status code: {status_code}")
                } else {
                    write!(f, "exited")
                }
            }
            ProcessState::Terminated(signal) => write!(f, "terminated with signal: {signal}"),
        }
    }
}

#[derive(Debug)]
pub struct Debuggee {
    pid: Pid,
    process_state: ProcessState,
    should_terminate: bool,
    registers: Option<Registers>,
}

#[derive(Debug)]
pub enum Config {
    Existing(Pid),
    SpawnChild(NonEmpty<String>),
}

impl Debuggee {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let span = debug_span!("creating debuggee");
        let _entered = span.enter();

        info!("initializing debuggee");

        let mut debuggee = match config {
            Config::Existing(pid) => {
                Self::attach(pid)?;
                Self {
                    pid,
                    process_state: ProcessState::Stopped(None),
                    should_terminate: false,
                    registers: None,
                }
            }
            Config::SpawnChild(child_args) => {
                let pid = Self::launch(child_args)?;
                Self {
                    pid,
                    process_state: ProcessState::Stopped(None),
                    should_terminate: true,
                    registers: None,
                }
            }
        };

        info!(pid = tracing::field::display(&debuggee.pid));

        debuggee.update_process_state(true)?;

        Ok(debuggee)
    }

    fn attach(pid: Pid) -> anyhow::Result<()> {
        let span = debug_span!(
            "attaching to child with pid",
            pid = tracing::field::display(&pid)
        );
        let _entered = span.enter();

        info!("attaching to debuggee");

        debug!("calling ptrace::attach");
        ptrace::attach(pid)
            .map_err(|err| anyhow!("unable to attach to debuggee process: {}", err))?;
        Ok(())
    }

    fn launch(child_args: NonEmpty<String>) -> anyhow::Result<Pid> {
        let span = debug_span!("launching child");
        let _entered = span.enter();

        info!("launching child process as debuggee");

        let (error_reporting_pipe_read_end, error_reporting_pipe_write_end) =
            pipe2(OFlag::O_CLOEXEC)?;

        match unsafe { fork() }? {
            ForkResult::Parent { child: pid } => {
                drop(error_reporting_pipe_write_end);

                let maybe_error_message = read_to_string(File::from(error_reporting_pipe_read_end))
                    .map_err(|err| anyhow!("unable to read from error reporting pipe: {}", err))?;
                if !maybe_error_message.is_empty() {
                    return Err(anyhow!(
                        "failed to launch debuggee: {}",
                        maybe_error_message
                    ));
                }

                Ok(pid)
            }
            ForkResult::Child => {
                drop(error_reporting_pipe_read_end);
                Self::exec_traceme(child_args, error_reporting_pipe_write_end)
            }
        }
    }

    fn exec_traceme(child_args: NonEmpty<String>, error_reporting_pipe_write_end: OwnedFd) -> ! {
        let span = debug_span!("child exec_traceme");
        let _entered = span.entered();

        let internal = || -> anyhow::Result<Infallible> {
            let child_args = child_args
                .iter()
                .map(|arg| CString::new(arg.clone()).unwrap())
                .collect::<Vec<_>>();
            debug!(?child_args);

            debug!("calling ptrace::traceme");
            ptrace::traceme().map_err(|err| anyhow!("unable to set traceme: {}", err))?;

            debug!("launching");
            execvp(
                CString::new(child_args[0].clone()).unwrap().as_ref(),
                &child_args,
            )?;

            unreachable!("POST EXEC")
        };

        let Err(err) = internal();
        _ = File::from(error_reporting_pipe_write_end).write_all(err.to_string().as_bytes());

        error!(
            error = box_err(err),
            child_args = ?child_args,
            "unable to spawn child",
        );

        exit(EXIT_FAILURE)
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn process_state(&self) -> ProcessState {
        self.process_state.clone()
    }

    pub fn registers(&self) -> Option<&Registers> {
        self.registers.as_ref()
    }

    pub fn registers_mut(&mut self) -> Option<&mut Registers> {
        self.registers.as_mut()
    }

    pub fn update_process_state(&mut self, blocking: bool) -> anyhow::Result<()> {
        let span = debug_span!(
            "waiting for debuggee state change",
            pid = tracing::field::display(&self.pid),
        );
        let _entered = span.entered();

        let wait_status = waitpid(self.pid, blocking.not().then_some(WaitPidFlag::WNOWAIT));

        self.process_state = match wait_status {
            Ok(WaitStatus::Exited(_, status_code)) => ProcessState::Exited(Some(status_code)),
            Ok(WaitStatus::Signaled(_, signal, _)) => ProcessState::Terminated(signal),
            Ok(WaitStatus::Stopped(_, signal)) => ProcessState::Stopped(Some(signal)),
            Ok(WaitStatus::Continued(_)) | Ok(WaitStatus::StillAlive) => ProcessState::Running,
            Ok(_) => unreachable!("Unhandled wait status"),
            Err(Errno::ECHILD) => ProcessState::Exited(None),
            Err(err) => Err(err)?,
        };

        if let ProcessState::Stopped(_) = self.process_state {
            self.read_registers()?;
        }

        Ok(())
    }

    pub fn resume(&mut self) -> anyhow::Result<()> {
        let span = debug_span!(
            "resuming debuggee",
            pid = tracing::field::display(&self.pid),
        );
        let _entered = span.entered();

        match self.process_state {
            ProcessState::Stopped(_) | ProcessState::Running => {
                ptrace::cont(self.pid, None)?;
                self.process_state = ProcessState::Running;
            }
            ProcessState::Exited(_) | ProcessState::Terminated(_) => {
                Err(anyhow!("unable to resume an exited or terminated process"))?;
            }
        }

        info!("debuggee process resumed");

        return Ok(());
    }

    fn read_registers(&mut self) -> anyhow::Result<()> {
        let span = debug_span!(
            "read registers of debuggee",
            pid = tracing::field::display(&self.pid),
        );
        let _entered = span.entered();

        debug!("reading registers");
        let regs = Registers::read_with_ptrace(self.pid)?;

        self.registers = Some(regs);

        Ok(())
    }
}

impl Drop for Debuggee {
    fn drop(&mut self) {
        let span = debug_span!(
            "dropping debuggee",
            pid = (tracing::field::display(&self.pid))
        );
        let _entered = span.enter();

        info!("detaching from debuggee");

        let _ = self.update_process_state(false);

        if self.process_state.is_alive() {
            if let Err(err) = kill(self.pid, Signal::SIGSTOP) {
                warn!(error = box_err(err), "unable to stop the debuggee process");

                return;
            };

            if let Err(err) = ptrace::detach(self.pid, Some(Signal::SIGCONT)) {
                warn!(
                    error = box_err(err),
                    "unable to detach from the debuggee process",
                )
            }

            if let Err(err) = kill(self.pid, Signal::SIGCONT) {
                warn!(
                    error = box_err(err),
                    "unable to resume the debuggee process",
                )
            }

            if self.should_terminate {
                info!("terminating debuggee");

                if let Err(err) = kill(self.pid, Signal::SIGKILL) {
                    warn!(error = box_err(err), "unable to kill the debuggee");

                    return;
                }

                if let Err(err) = waitpid(self.pid, None) {
                    warn!(error = box_err(err), "unable to wait for debuggee to exit")
                }
            }
        }

        if self.should_terminate {
            _ = wait();
        }
    }
}
