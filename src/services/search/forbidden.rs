use aho_corasick::AhoCorasick;
use std::sync::OnceLock;

/// Static AhoCorasick matcher for forbidden system and virtual paths.
static FORBIDDEN_PATHS: OnceLock<AhoCorasick> = OnceLock::new();

pub(super) fn forbidden_matcher() -> &'static AhoCorasick {
    FORBIDDEN_PATHS.get_or_init(|| {
        AhoCorasick::builder()
            .build([
                b"/proc/".as_slice(),
                b"/sys/".as_slice(),
                b"/dev/".as_slice(),
                b"/run/".as_slice(),
                b"/var/run/".as_slice(),
                b"/dosdevices/".as_slice(),
                b"/Prefixes/".as_slice(),
                b"/compatdata/".as_slice(),
                b"/drive_c/".as_slice(),
            ])
            .expect("valid forbidden path patterns")
    })
}
