//! Runtime availability checks for optional external binaries.

/// Logs a warning for each optional external binary not found in `$PATH`.
pub fn check_optional_deps() {
    for bin in ["ffmpeg", "ffprobe", "magick"] {
        if std::process::Command::new("which")
            .arg(bin)
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            eprintln!("[flux] optional dependency '{bin}' not found in PATH");
        }
    }

    check_gvfs_deps();
}

/// Checks GVFS daemon and backend availability and logs a warning for each
/// missing component.
///
/// Missing `gvfsd` prevents all network browsing. Missing backend daemons
/// (e.g. `gvfsd-smb`) disable that specific protocol while leaving others
/// functional. The check is advisory: Flux reports the deficiency as a toast
/// at the point of use rather than blocking startup.
fn check_gvfs_deps() {
    let results = crate::services::network::check_gvfs_deps();

    if !results.get("gvfsd").copied().unwrap_or(false) {
        eprintln!(
            "[flux] GVFS daemon ('gvfsd') not found - network browsing will be unavailable. \
             Install the 'gvfs' package via your package manager."
        );
        return;
    }

    let backends = [
        ("gvfsd-smb", "SMB/Samba"),
        ("gvfsd-sftp", "SFTP"),
        ("gvfsd-ftp", "FTP"),
        ("gvfsd-nfs", "NFS"),
    ];

    for (binary, label) in backends {
        if !results.get(binary).copied().unwrap_or(false) {
            eprintln!(
                "[flux] optional GVFS backend '{binary}' not found - {label} browsing may be \
                 limited. Install the corresponding gvfs backend package."
            );
        }
    }
}
