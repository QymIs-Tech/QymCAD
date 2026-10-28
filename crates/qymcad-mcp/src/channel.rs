//! THE PROTOCOL'S OWN LINE: stdout kept for the answers alone, whatever else in the process writes to it.
//!
//! OCCT prints to the standard output of the process - the STEP writer prints eight lines of transfer statistics on
//! every export - and a client reading one JSON message per line takes them for an answer and drops the session. The
//! kernel is not told to keep quiet: its messages are the window's too, and what it prints next is not ours to list.
//! Instead the server takes a copy of stdout for itself and points the descriptor everybody else writes to at stderr,
//! where a client shows text as a log and reads none of it as protocol.
use std::fs::File;

/// Take stdout for the protocol: the copy to write answers to, with descriptor 1 now leading to stderr. `None` when
/// the descriptors cannot be moved; the answers then go to stdout as it is, and the kernel's lines go with them.
pub fn take_stdout() -> Option<File> {
    sys::take_stdout()
}

#[cfg(windows)]
mod sys {
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;

    extern "C" {
        fn _dup(fd: i32) -> i32;
        fn _dup2(from: i32, to: i32) -> i32;
        fn _get_osfhandle(fd: i32) -> isize;
    }

    pub fn take_stdout() -> Option<File> {
        // the C runtime's descriptors are what the C++ streams of the kernel write through; `_dup2` onto 1 also moves
        // the process's standard output handle, so the Rust side and the C++ side are moved together
        unsafe {
            let copy = _dup(1);
            if copy < 0 || _dup2(2, 1) < 0 {
                return None;
            }
            let handle = _get_osfhandle(copy);
            if handle == -1 {
                return None;
            }
            Some(File::from_raw_handle(handle as *mut std::ffi::c_void))
        }
    }
}

#[cfg(unix)]
mod sys {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    extern "C" {
        fn dup(fd: i32) -> i32;
        fn dup2(from: i32, to: i32) -> i32;
    }

    pub fn take_stdout() -> Option<File> {
        unsafe {
            let copy = dup(1);
            if copy < 0 || dup2(2, 1) < 0 {
                return None;
            }
            Some(File::from_raw_fd(copy))
        }
    }
}
