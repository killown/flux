use std::sync::OnceLock;

static HAS_NERD_FONT: OnceLock<bool> = OnceLock::new();

/// Returns whether a Nerd Font is available on the system using Fontconfig.
pub fn is_nerd_font_available() -> bool {
    *HAS_NERD_FONT.get_or_init(|| {
        #[link(name = "fontconfig")]
        extern "C" {
            fn FcInitLoadConfigAndFonts() -> *mut std::ffi::c_void;
            fn FcPatternCreate() -> *mut std::ffi::c_void;
            fn FcPatternDestroy(p: *mut std::ffi::c_void);
            fn FcConfigDestroy(c: *mut std::ffi::c_void);
            fn FcFontList(
                config: *mut std::ffi::c_void,
                p: *mut std::ffi::c_void,
                os: *mut std::ffi::c_void,
            ) -> *mut std::ffi::c_void;
            fn FcFontSetDestroy(fs: *mut std::ffi::c_void);
            fn FcPatternGetString(
                p: *mut std::ffi::c_void,
                object: *const std::os::raw::c_char,
                n: std::os::raw::c_int,
                s: *mut *mut std::os::raw::c_char,
            ) -> std::os::raw::c_int;
        }

        #[repr(C)]
        struct FcFontSet {
            nfont: std::os::raw::c_int,
            sfont: std::os::raw::c_int,
            fonts: *mut *mut std::ffi::c_void,
        }

        unsafe {
            let config = FcInitLoadConfigAndFonts();
            if config.is_null() {
                return false;
            }

            let pat = FcPatternCreate();
            let fs_ptr = FcFontList(config, pat, std::ptr::null_mut());
            FcPatternDestroy(pat);

            let mut found = false;
            if !fs_ptr.is_null() {
                let fs = &*(fs_ptr as *const FcFontSet);
                let family_key = c"family";
                for i in 0..fs.nfont {
                    let font_pat = *fs.fonts.add(i as usize);
                    let mut family_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
                    if FcPatternGetString(font_pat, family_key.as_ptr(), 0, &mut family_ptr) == 0
                        && !family_ptr.is_null()
                    {
                        let family = std::ffi::CStr::from_ptr(family_ptr).to_string_lossy();
                        if family.contains("Nerd Font") || family.contains("Symbols Nerd") {
                            found = true;
                            break;
                        }
                    }
                }
                FcFontSetDestroy(fs_ptr);
            }
            FcConfigDestroy(config);
            found
        }
    })
}
