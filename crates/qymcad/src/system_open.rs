//! WHAT THE SYSTEM ASKS OF THE PROGRAM: a file handed to the program on start-up (command line argument)
//! or by the desktop (macOS open-document AppleEvent).
//!
//! Two paths:
//!
//! 1. When the program starts: an argument on the command line (`qymcad part.qcad`) or passed by Finder.
//! 2. While the program is running: double-clicking a .qcad file on macOS sends an AppleEvent
//!    (kCoreEventClass / kAEOpenDocuments) to the running instance, which eframe/winit does not pass on.
//!    The event handler puts the path into a queue, and the frame loop picks it up.
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static PENDING_OPEN: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Extract a file path to open from command line arguments.
///
/// Skips the executable name (arg 0) and any flags starting with `-`. On macOS also skips the process
/// serial number (`-psn_...`) Finder passed on older releases. Accepts a path if it ends with `.qcad`
/// or names an existing file.
pub(crate) fn parse_cli_argument<I, S>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    for arg in args.into_iter().skip(1) {
        let s = arg.as_ref();
        if s.starts_with('-') {
            continue;
        }
        let p = Path::new(s);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or_default();
        if ext.eq_ignore_ascii_case("qcad") || p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// Push a file path to be opened by the application.
pub(crate) fn queue_path(path: PathBuf) {
    if let Ok(mut q) = PENDING_OPEN.lock() {
        q.push(path);
    }
}

/// Take the next path waiting to be opened, if any.
pub(crate) fn poll_pending_path() -> Option<String> {
    let mut q = PENDING_OPEN.lock().ok()?;
    if q.is_empty() {
        None
    } else {
        Some(q.remove(0).to_string_lossy().into_owned())
    }
}

/// The file to open at startup: either a queued event that arrived before the first frame, or a command line argument.
pub(crate) fn startup_path() -> Option<String> {
    poll_pending_path().or_else(|| parse_cli_argument(std::env::args()).map(|p| p.to_string_lossy().into_owned()))
}

#[cfg(target_os = "macos")]
mod macos_events {
    use std::path::PathBuf;

    #[repr(C)]
    #[derive(Default)]
    struct AEDesc {
        descriptor_type: u32,
        data_handle: *mut std::ffi::c_void,
    }

    type OSErr = i16;
    type SRefCon = isize;
    type Boolean = u8;

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn AEInstallEventHandler(
            the_ae_event_class: u32,
            the_ae_event_id: u32,
            handler: extern "C" fn(*const AEDesc, *mut AEDesc, SRefCon) -> OSErr,
            handler_refcon: SRefCon,
            is_sys_handler: Boolean,
        ) -> OSErr;

        fn AEGetParamDesc(the_apple_event: *const AEDesc, the_ae_keyword: u32, desired_type: u32, result: *mut AEDesc) -> OSErr;

        fn AECountItems(the_ae_list: *const AEDesc, the_count: *mut isize) -> OSErr;

        fn AEGetNthPtr(
            the_ae_list: *const AEDesc,
            index: isize,
            desired_type: u32,
            the_ae_keyword: *mut u32,
            type_code: *mut u32,
            data_ptr: *mut std::ffi::c_void,
            maximum_size: isize,
            actual_size: *mut isize,
        ) -> OSErr;

        fn AEDisposeDesc(the_ae_desc: *mut AEDesc) -> OSErr;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFURLCreateWithBytes(allocator: *const std::ffi::c_void, bytes: *const u8, length: isize, encoding: u32, base_url: *const std::ffi::c_void) -> *mut std::ffi::c_void;

        fn CFURLGetFileSystemRepresentation(url: *const std::ffi::c_void, resolve_against_base: Boolean, buffer: *mut u8, max_buf_len: isize) -> Boolean;

        fn CFRelease(cf: *const std::ffi::c_void);
    }

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn class_addMethod(cls: *mut std::ffi::c_void, name: *mut std::ffi::c_void, imp: unsafe extern "C" fn(), types: *const std::ffi::c_char) -> bool;
        fn objc_msgSend();
    }

    type Id = *mut std::ffi::c_void;
    type Sel = *const std::ffi::c_void;
    type AppPtr = *mut std::ffi::c_void;
    type FilenamePtr = *mut std::ffi::c_void;

    unsafe extern "C" fn app_open_file(_this: Id, _cmd: Sel, _app: AppPtr, filename: FilenamePtr) -> u8 {
        if !filename.is_null() {
            let utf8_sel = sel_registerName(c"UTF8String".as_ptr());
            let utf8_fn: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> *const std::ffi::c_char = std::mem::transmute(objc_msgSend as *const ());
            let c_str = utf8_fn(filename, utf8_sel);
            if !c_str.is_null() {
                if let Ok(s) = std::ffi::CStr::from_ptr(c_str).to_str() {
                    super::queue_path(PathBuf::from(s));
                }
            }
        }
        1
    }

    unsafe extern "C" fn app_open_files(_this: Id, _cmd: Sel, sender: AppPtr, filenames: FilenamePtr) {
        if !filenames.is_null() {
            let count_sel = sel_registerName(c"count".as_ptr());
            let count_fn: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> usize = std::mem::transmute(objc_msgSend as *const ());
            let count = count_fn(filenames, count_sel);

            let obj_at_idx_sel = sel_registerName(c"objectAtIndex:".as_ptr());
            let obj_fn: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, usize) -> *mut std::ffi::c_void = std::mem::transmute(objc_msgSend as *const ());
            let utf8_sel = sel_registerName(c"UTF8String".as_ptr());
            let utf8_fn: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> *const std::ffi::c_char = std::mem::transmute(objc_msgSend as *const ());

            for i in 0..count {
                let item = obj_fn(filenames, obj_at_idx_sel, i);
                if !item.is_null() {
                    let c_str = utf8_fn(item, utf8_sel);
                    if !c_str.is_null() {
                        if let Ok(s) = std::ffi::CStr::from_ptr(c_str).to_str() {
                            super::queue_path(PathBuf::from(s));
                        }
                    }
                }
            }
        }
        if !sender.is_null() {
            let reply_sel = sel_registerName(c"replyToOpenOrPrint:".as_ptr());
            let reply_fn: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, usize) = std::mem::transmute(objc_msgSend as *const ());
            reply_fn(sender, reply_sel, 0);
        }
    }

    extern "C" fn handle_open_documents(event: *const AEDesc, _reply: *mut AEDesc, _refcon: SRefCon) -> OSErr {
        let key_direct_object = u32::from_be_bytes(*b"----");
        let type_ae_list = u32::from_be_bytes(*b"list");
        let type_file_url = u32::from_be_bytes(*b"furl");
        let type_utf8 = u32::from_be_bytes(*b"utf8");

        unsafe {
            let mut list = AEDesc::default();
            if AEGetParamDesc(event, key_direct_object, type_ae_list, &mut list) == 0 {
                let mut count = 0;
                if AECountItems(&list, &mut count) == 0 {
                    for i in 1..=count {
                        let mut buf = [0u8; 1024];
                        let mut actual_size = 0;
                        let mut key = 0;
                        let mut type_code = 0;
                        if AEGetNthPtr(&list, i, type_file_url, &mut key, &mut type_code, buf.as_mut_ptr() as _, buf.len() as isize, &mut actual_size) == 0 && actual_size > 0 {
                            let url = CFURLCreateWithBytes(std::ptr::null(), buf.as_ptr(), actual_size, 0x08000100, std::ptr::null());
                            if !url.is_null() {
                                let mut path_buf = [0u8; 1024];
                                if CFURLGetFileSystemRepresentation(url, 1, path_buf.as_mut_ptr(), path_buf.len() as isize) != 0 {
                                    if let Ok(s) = std::ffi::CStr::from_ptr(path_buf.as_ptr() as _).to_str() {
                                        super::queue_path(PathBuf::from(s));
                                    }
                                }
                                CFRelease(url);
                            }
                        } else if AEGetNthPtr(&list, i, type_utf8, &mut key, &mut type_code, buf.as_mut_ptr() as _, buf.len() as isize, &mut actual_size) == 0 && actual_size > 0 {
                            if let Ok(s) = std::str::from_utf8(&buf[..actual_size as usize]) {
                                super::queue_path(PathBuf::from(s));
                            }
                        }
                    }
                }
                AEDisposeDesc(&mut list);
            }
        }
        0
    }

    pub(super) fn install() {
        let aevt = u32::from_be_bytes(*b"aevt");
        let odoc = u32::from_be_bytes(*b"odoc");
        let _ = unsafe { AEInstallEventHandler(aevt, odoc, handle_open_documents, 0, 0) };

        // Cocoa AppKit checks whether NSApplicationDelegate responds to application:openFile:.
        // If not, it falls back to NSDocumentController which raises an error dialog.
        // Injecting the methods into NSObject and WinitApplicationDelegate lets AppKit dispatch open requests cleanly.
        unsafe {
            for class_name in [c"NSObject".as_ptr(), c"WinitApplicationDelegate".as_ptr()] {
                let cls = objc_getClass(class_name);
                if !cls.is_null() {
                    let sel_single = sel_registerName(c"application:openFile:".as_ptr());
                    let imp_single: unsafe extern "C" fn() = std::mem::transmute(app_open_file as *const ());
                    let _ = class_addMethod(cls, sel_single, imp_single, c"c@:@@".as_ptr());

                    let sel_multi = sel_registerName(c"application:openFiles:".as_ptr());
                    let imp_multi: unsafe extern "C" fn() = std::mem::transmute(app_open_files as *const ());
                    let _ = class_addMethod(cls, sel_multi, imp_multi, c"v@:@@".as_ptr());
                }
            }
        }
    }
}

/// Register desktop event handlers for files opened by the system.
pub(crate) fn install_open_document_handler() {
    #[cfg(target_os = "macos")]
    macos_events::install();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cli_argument_finds_project_path() {
        let args = vec!["qymcad".to_string(), "-v".to_string(), "sample.qcad".to_string()];
        assert_eq!(parse_cli_argument(args), Some(PathBuf::from("sample.qcad")));

        let flags_only = vec!["qymcad".to_string(), "--help".to_string(), "-psn_0_1234".to_string()];
        assert_eq!(parse_cli_argument(flags_only), None);

        let empty: Vec<String> = vec!["qymcad".to_string()];
        assert_eq!(parse_cli_argument(empty), None);
    }

    #[test]
    fn queue_and_poll_pending_paths() {
        queue_path(PathBuf::from("test_first.qcad"));
        queue_path(PathBuf::from("test_second.qcad"));

        assert_eq!(poll_pending_path(), Some("test_first.qcad".to_string()));
        assert_eq!(poll_pending_path(), Some("test_second.qcad".to_string()));
        assert_eq!(poll_pending_path(), None);
    }

    #[test]
    fn install_handler_runs_without_panic() {
        install_open_document_handler();
    }
}
