//! Parsers for the C-locale output of `udisksctl`.
//!
//! These only understand the English wording, so every `udisksctl` call that
//! feeds them must run with `LC_ALL=C` (see `mount::udisksctl_command`).

use std::path::PathBuf;

/// Last whitespace-separated token of `stdout` without its trailing dot, when
/// it starts with one of `prefixes`.
fn last_device_token<'a>(stdout: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    stdout
        .split_whitespace()
        .last()
        .map(|s| s.trim_end_matches('.'))
        .filter(|s| prefixes.iter().any(|p| s.starts_with(p)))
}

/// Extracts the loop device from `udisksctl loop-setup` output.
///
/// Expects `Mapped file /path/vault.img as /dev/loopN.` and returns
/// `/dev/loopN`, or `None` when the output has any other shape.
pub fn parse_loop_device(stdout: &str) -> Option<String> {
    last_device_token(stdout, &["/dev/loop"]).map(str::to_string)
}

/// Extracts the decrypted device node from `udisksctl unlock` output.
///
/// Expects `Unlocked /dev/loopN as /dev/dm-N.` (or a `/dev/mapper/...` node)
/// and returns the device node, or `None` when the output has any other shape.
pub fn parse_dm_device(stdout: &str) -> Option<String> {
    last_device_token(stdout, &["/dev/dm-", "/dev/mapper/"]).map(str::to_string)
}

/// Extracts the mount point from `udisksctl mount` output.
///
/// Expects `Mounted /dev/dm-N at /run/media/user/name.` and returns the path
/// after the first ` at `, without the trailing dot. Returns `None` when the
/// separator is missing or nothing follows it.
pub fn parse_mount_point(stdout: &str) -> Option<PathBuf> {
    let (_, rest) = stdout.split_once(" at ")?;
    let mount_point = rest.trim().trim_end_matches('.');
    (!mount_point.is_empty()).then(|| PathBuf::from(mount_point))
}

/// Whether the `udisksctl unlock` stderr reports a rejected passphrase.
///
/// Matching is case-insensitive on the C-locale wording.
pub fn is_wrong_passphrase(stderr: &str) -> bool {
    let stderr = stderr.to_lowercase();
    stderr.contains("bad passphrase") || stderr.contains("no key available")
}
