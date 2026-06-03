use gtk::glib;

// 1. Declare the gettext functions that are missing from the `libc` crate
extern "C" {
    fn bindtextdomain(
        domainname: *const libc::c_char,
        dirname: *const libc::c_char,
    ) -> *mut libc::c_char;
    fn bind_textdomain_codeset(
        domainname: *const libc::c_char,
        codeset: *const libc::c_char,
    ) -> *mut libc::c_char;
    fn textdomain(domainname: *const libc::c_char) -> *mut libc::c_char;
    fn ngettext(
        msgid1: *const libc::c_char,
        msgid2: *const libc::c_char,
        n: libc::c_ulong,
    ) -> *mut libc::c_char;
}

/// Initializes gettext locale bindings for the process.
pub fn init() {
    unsafe {
        // Set locale based on environment variables (LANG, LC_ALL, etc.)
        libc::setlocale(libc::LC_ALL, c"".as_ptr());

        // Dynamically resolve ~/.local/share/locale
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let locale_path = format!("{}/.local/share/locale", home);
        let c_locale_path = std::ffi::CString::new(locale_path).unwrap();

        // Bind the "flux" domain to the resolved locale directory
        // Note: We call the functions directly, NOT libc::bindtextdomain
        bindtextdomain(c"flux".as_ptr(), c_locale_path.as_ptr());
        bind_textdomain_codeset(c"flux".as_ptr(), c"UTF-8".as_ptr());

        // Set "flux" as the default text domain
        textdomain(c"flux".as_ptr());
    }
}

/// Translates a string using the active locale.
#[inline]
pub fn tr(s: &str) -> String {
    // Explicitly specify the "flux" domain to guarantee GLib looks in our .mo file
    glib::dgettext(Some("flux"), s).to_string()
}

/// Translates a string with singular/plural forms based on `n`.
#[allow(dead_code)]
#[inline]
pub fn ntr(singular: &str, plural: &str, n: u64) -> String {
    let c_str = unsafe {
        let s = std::ffi::CString::new(singular).unwrap_or_default();
        let p = std::ffi::CString::new(plural).unwrap_or_default();
        ngettext(s.as_ptr(), p.as_ptr(), n as libc::c_ulong)
    };
    unsafe { std::ffi::CStr::from_ptr(c_str) }
        .to_string_lossy()
        .into_owned()
}

/// Translates a string with a disambiguation context prefix.
#[allow(dead_code)]
#[inline]
pub fn ptr(ctx: &str, s: &str) -> String {
    // Explicitly specify the "flux" domain here as well
    glib::dpgettext2(Some("flux"), ctx, s).to_string()
}
