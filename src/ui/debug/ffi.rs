use std::ffi::CStr;

extern "C" {
    fn mi_collect(force: bool);
    fn mi_stats_print_out(
        out: Option<unsafe extern "C" fn(*const libc::c_char, *mut libc::c_void)>,
        arg: *mut libc::c_void,
    );
}

unsafe extern "C" fn mimalloc_output_cb(msg: *const libc::c_char, arg: *mut libc::c_void) {
    if msg.is_null() || arg.is_null() {
        return;
    }
    let target = &mut *(arg as *mut String);
    let cstr = CStr::from_ptr(msg);
    if let Ok(s) = cstr.to_str() {
        target.push_str(s);
    }
}

/// Captures mimalloc's runtime stats as a multi-line string.
pub(super) fn read_mimalloc_stats() -> String {
    let mut out = String::with_capacity(4096);
    unsafe {
        mi_stats_print_out(
            Some(mimalloc_output_cb),
            &mut out as *mut String as *mut libc::c_void,
        );
    }
    out
}

/// Forces mimalloc to return free pages to the OS.
pub(super) fn collect_now() {
    unsafe { mi_collect(true) };
}
