#[cfg(test)]
mod debuggee_tests {
    use std::env;

    use nonempty::nonempty;
    use stupid_dbg::debuggee::{self, Debuggee};
    use tracing::Level;
    use tracing_subscriber::fmt::format::FmtSpan;

    mod aux {
        use nix::{errno::Errno, sys::signal::kill, unistd::Pid};

        pub fn is_process_existing(pid: Pid) -> bool {
            match kill(pid, None) {
                Ok(()) => true,
                Err(err) => err != Errno::ESRCH,
            }
        }
    }

    #[ctor::ctor]
    fn init() {
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

    #[test]
    fn launch_yes() {
        let debuggee =
            Debuggee::new(debuggee::Config::SpawnChild(nonempty!["yes".to_string()])).unwrap();
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
}
