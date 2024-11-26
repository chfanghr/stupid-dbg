use std::{mem::MaybeUninit, ptr};

use nix::{errno::Errno, sys::ptrace, unistd::Pid};

pub fn box_err<E>(err: E) -> Box<dyn std::error::Error + 'static>
where
    E: Into<Box<dyn std::error::Error + 'static>>,
{
    err.into()
}

pub unsafe fn read_any_from_void_pointer<T>(from_ptr: *const u8, size: usize) -> T {
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
