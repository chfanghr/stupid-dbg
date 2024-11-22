use std::{ffi::CString, process::exit};

use anyhow::anyhow;
use libc::EXIT_FAILURE;
use nix::{
    sys::{
        ptrace::{self},
        signal::{kill, Signal},
        wait::waitpid,
    },
    unistd::{execvp, fork, ForkResult, Pid},
};
use nonempty::NonEmpty;
use tracing::{debug, debug_span, error, info, warn};

#[derive(Debug, Copy, Clone)]
pub enum ProcessState {
    Running,
    Paused,
    Exited,
}

#[derive(Debug)]
pub struct Debuggee {
    pid: Pid,
    process_state: ProcessState,
    should_terminate: bool,
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

        let debuggee = match config {
            Config::Existing(pid) => {
                Self::attach(pid)?;
                Self {
                    pid,
                    process_state: ProcessState::Paused,
                    should_terminate: false,
                }
            }
            Config::SpawnChild(child_args) => {
                let pid = Self::launch(child_args)?;
                Self {
                    pid,
                    process_state: ProcessState::Paused,
                    should_terminate: true,
                }
            }
        };

        info!(pid = tracing::field::display(&debuggee.pid));

        debuggee.wait_for_state_change()?;

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

        match unsafe { fork() }? {
            ForkResult::Parent { child: pid } => Ok(pid),
            ForkResult::Child => Self::exec_traceme(child_args),
        }
    }

    fn exec_traceme(child_args: NonEmpty<String>) -> ! {
        let span = debug_span!("child exec_traceme");
        let _entered = span.entered();

        let internal = || -> anyhow::Result<()> {
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

            Ok(())
        };

        match internal() {
            Err(err) => {
                error!(
                    error = Box::<dyn std::error::Error + 'static>::from(err),
                    child_args = ?child_args,
                    "unable to spawn child",
                );

                exit(EXIT_FAILURE)
            }
            Ok(_) => unreachable!("POST EXEC"),
        }
    }

    pub fn process_state(&self) -> ProcessState {
        self.process_state
    }

    pub fn wait_for_state_change(&self) -> anyhow::Result<()> {
        let span = debug_span!(
            "waiting for debuggee state change",
            pid = tracing::field::display(&self.pid),
        );
        let _entered = span.entered();

        waitpid(self.pid, None)?;

        Ok(())
    }

    pub fn resume(&mut self) -> anyhow::Result<()> {
        let span = debug_span!(
            "resuming debuggee",
            pid = tracing::field::display(&self.pid),
        );
        let _entered = span.entered();

        match self.process_state {
            ProcessState::Paused | ProcessState::Running => {
                ptrace::cont(self.pid, None)?;
                self.process_state = ProcessState::Running;
            }
            ProcessState::Exited => {
                Err(anyhow!("unable to resume an exited process"))?;
            }
        }

        info!("debuggee process resumed");

        return Ok(());
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

        if let Err(err) = kill(self.pid, Signal::SIGSTOP) {
            warn!(
                error = Box::<dyn std::error::Error + 'static>::from(err),
                "unable to stop the debuggee process"
            );

            return;
        };

        if let Err(err) = ptrace::detach(self.pid, Some(Signal::SIGCONT)) {
            warn!(
                error = Box::<dyn std::error::Error + 'static>::from(err),
                "unable to detach from the debuggee process",
            )
        }

        if let Err(err) = kill(self.pid, Signal::SIGCONT) {
            warn!(
                error = Box::<dyn std::error::Error + 'static>::from(err),
                "unable to resume the debuggee process",
            )
        }

        if self.should_terminate {
            info!("terminating debuggee");

            if let Err(err) = kill(self.pid, Signal::SIGKILL) {
                warn!(
                    error = Box::<dyn std::error::Error + 'static>::from(err),
                    "unable to kill the debuggee"
                );

                return;
            }

            if let Err(err) = waitpid(self.pid, None) {
                warn!(
                    error = Box::<dyn std::error::Error + 'static>::from(err),
                    "unable to wait for debuggee to exit"
                )
            }
        }
    }
}
