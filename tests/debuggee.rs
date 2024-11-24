#[cfg(test)]
mod debuggee_tests {
    use nix::unistd::Pid;
    use nonempty::nonempty;
    use stupid_dbg::debuggee::{self, Debuggee};

    mod aux {
        use std::{
            env,
            ffi::CString,
            fs::File,
            io::{read_to_string, stderr, stdout, Write},
            os::fd::AsRawFd,
        };

        use nix::{
            errno::Errno,
            fcntl::OFlag,
            sys::signal::kill,
            unistd::{dup2, execvp, fork, pipe2, ForkResult, Pid},
        };
        use nonempty::NonEmpty;
        use tracing::Level;
        use tracing_subscriber::fmt::format::FmtSpan;

        pub fn setup_logging() {
            if let Ok(val) = env::var("STUPID_DBG_TEST_VERBOSE_LOGGING") {
                if &val == "1" {
                    let collector = tracing_subscriber::fmt()
                        .with_max_level(Level::DEBUG)
                        .with_span_events(FmtSpan::FULL)
                        .finish();
                    tracing::subscriber::set_global_default(collector).unwrap();
                }
            }
        }
        pub fn is_process_existing(pid: Pid) -> bool {
            match kill(pid, None) {
                Ok(()) => true,
                Err(err) => err != Errno::ESRCH,
            }
        }

        pub fn read_process_stat_from_procfs(pid: Pid) -> procfs::process::Stat {
            procfs::process::Process::new(pid.as_raw())
                .unwrap()
                .stat()
                .unwrap()
        }

        pub fn spawn(args: NonEmpty<String>, no_std_out_or_std_err: bool) -> Pid {
            let (error_reporting_pipe_read_end, error_reporting_pipe_write_end) =
                pipe2(OFlag::O_CLOEXEC).unwrap();
            match unsafe { fork() }.unwrap() {
                ForkResult::Parent { child: pid } => {
                    drop(error_reporting_pipe_write_end);
                    let err_msg =
                        read_to_string(File::from(error_reporting_pipe_read_end)).unwrap();
                    if !err_msg.is_empty() {
                        panic!("child failed to launch: {}", err_msg)
                    }
                    pid
                }
                ForkResult::Child => {
                    drop(error_reporting_pipe_read_end);

                    if no_std_out_or_std_err {
                        let dev_null = File::open("/dev/null").unwrap();
                        dup2(dev_null.as_raw_fd(), stdout().as_raw_fd()).unwrap();
                        dup2(dev_null.as_raw_fd(), stderr().as_raw_fd()).unwrap();
                    }

                    let args = args
                        .iter()
                        .map(|arg| CString::new(arg.clone()).unwrap())
                        .collect::<Vec<_>>();
                    let Err(err) = execvp(CString::new(args[0].clone()).unwrap().as_ref(), &args);
                    _ = File::from(error_reporting_pipe_write_end)
                        .write_all(err.to_string().as_bytes());
                    panic!("execvp failed: {}", err.to_string())
                }
            }
        }

        pub fn get_program_running_endlessly() -> String {
            if let Ok(program) = env::var("STUPID_DBG_TEST_PROGRAM_RUNNING_ENDLESSLY") {
                program
            } else {
                "yes".to_string()
            }
        }

        pub fn get_program_exiting_immediately() -> String {
            if let Ok(program) = env::var("STUPID_DBG_TEST_PROGRAM_EXITING_IMMEDIATELY") {
                program
            } else {
                "true".to_string()
            }
        }
    }

    #[ctor::ctor]
    fn init() {
        aux::setup_logging();
    }

    #[test]
    fn launch_program() {
        let debuggee = Debuggee::new(debuggee::Config::SpawnChild(nonempty![
            aux::get_program_running_endlessly()
        ]))
        .unwrap();
        let pid = debuggee.pid();
        assert!(aux::is_process_existing(pid))
    }

    #[test]
    fn launch_nonexistent_program() {
        assert!(
            Debuggee::new(debuggee::Config::SpawnChild(nonempty![format!(
                "this_program_doesnt_exist",
            )]))
            .is_err()
        )
    }

    #[test]
    fn attach_to_process() {
        let pid = aux::spawn(nonempty![aux::get_program_running_endlessly()], true);
        let _debuggee = Debuggee::new(debuggee::Config::Existing(pid)).unwrap();
        let debuggee_procfs_stat = aux::read_process_stat_from_procfs(pid);
        assert_eq!(
            debuggee_procfs_stat.state().unwrap(),
            procfs::process::ProcState::Tracing
        )
    }

    #[test]
    fn attach_to_invalid_pid() {
        assert!(Debuggee::new(debuggee::Config::Existing(Pid::from_raw(-1))).is_err())
    }

    #[test]
    fn launch_and_resume_program_running_endlessly() {
        let mut debuggee = Debuggee::new(debuggee::Config::SpawnChild(nonempty![
            aux::get_program_running_endlessly()
        ]))
        .unwrap();
        debuggee.resume().unwrap();
        let debuggee_procfs_stat = aux::read_process_stat_from_procfs(debuggee.pid());
        let debuggee_procfs_state = debuggee_procfs_stat.state().unwrap();
        let expected_states = [
            procfs::process::ProcState::Running,
            procfs::process::ProcState::Sleeping,
        ];
        assert!(expected_states
            .into_iter()
            .any(|expected| debuggee_procfs_state == expected))
    }

    #[test]
    fn attach_and_resume_program_running_endlessly() {
        let pid = aux::spawn(nonempty![aux::get_program_running_endlessly()], true);
        let mut debuggee = Debuggee::new(debuggee::Config::Existing(pid)).unwrap();
        debuggee.resume().unwrap();
        let debuggee_procfs_stat = aux::read_process_stat_from_procfs(pid);
        let debuggee_procfs_state = debuggee_procfs_stat.state().unwrap();
        let expected_states = [
            procfs::process::ProcState::Running,
            procfs::process::ProcState::Sleeping,
        ];
        assert!(expected_states
            .into_iter()
            .any(|expected| debuggee_procfs_state == expected))
    }

    #[test]
    fn launch_and_resume_program_exiting_immediately() {
        let mut debuggee = Debuggee::new(debuggee::Config::SpawnChild(nonempty![
            aux::get_program_exiting_immediately()
        ]))
        .unwrap();
        debuggee.resume().unwrap();
        debuggee.update_process_state(true).unwrap();
        assert!(debuggee.resume().is_err())
    }
}
