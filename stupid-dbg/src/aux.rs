use std::{mem::MaybeUninit, path::Path, ptr};

use nix::{errno::Errno, sys::ptrace, unistd::Pid};
use tracing::warn;

pub fn box_err<E>(err: E) -> Box<dyn std::error::Error + 'static>
where
    E: Into<Box<dyn std::error::Error + 'static>>,
{
    err.into()
}

pub unsafe fn read_any_from_u8_pointer<T>(from_ptr: *const u8, size: usize) -> T {
    assert!(size_of::<T>() >= size);
    let mut ret = MaybeUninit::<T>::zeroed();
    let ptr = ret.as_mut_ptr().cast::<u8>();
    from_ptr.copy_to(ptr, size);
    let ret = ret.assume_init();
    return ret;
}

pub fn ptrace_get_data<T>(request: ptrace::Request, pid: Pid) -> nix::Result<T> {
    let mut data = MaybeUninit::<T>::uninit();
    let res = unsafe {
        libc::ptrace(
            request as libc::c_uint,
            libc::pid_t::from(pid),
            ptr::null_mut::<T>(),
            data.as_mut_ptr(),
        )
    };
    Errno::result(res)?;
    Ok(unsafe { data.assume_init() })
}

pub fn ptrace_getfpregs(pid: Pid) -> nix::Result<libc::user_fpregs_struct> {
    ptrace_get_data(ptrace::Request::PTRACE_GETFPREGS, pid)
}

pub struct RlWithOpitonalHistoryFile<P: AsRef<Path>> {
    history_file: Option<P>,
    rl: rustyline::Editor<(), rustyline::history::FileHistory>,
}

impl<P: AsRef<Path>> RlWithOpitonalHistoryFile<P> {
    pub fn new(history_file: Option<P>) -> anyhow::Result<Self> {
        let mut rl = rustyline::DefaultEditor::new()?;

        if let Some(history_file) = &history_file {
            if let Err(err) = rl.load_history(history_file) {
                warn!(error = box_err(err), "unable to load history file");
            }
        }

        Ok(Self { history_file, rl })
    }

    pub fn readline(&mut self, prompt: &str) -> rustyline::Result<String> {
        self.rl.readline(prompt)
    }

    pub fn add_history_entry<S: AsRef<str> + Into<String>>(
        &mut self,
        s: S,
    ) -> rustyline::Result<bool> {
        self.rl.add_history_entry(s)
    }
}

impl<P: AsRef<Path>> Drop for RlWithOpitonalHistoryFile<P> {
    fn drop(&mut self) {
        if let Some(history_file) = &self.history_file {
            if let Err(err) = self.rl.save_history(history_file) {
                warn!(error = box_err(err), "unable to save history file");
            }
        }
    }
}
