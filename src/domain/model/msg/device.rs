//! Mounted devices and LUKS images.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum DeviceMsg {
    /// Delivers background-enumerated system and network mounts to update the sidebar.
    SystemMountsReady(Vec<(String, std::path::PathBuf)>),
    /// Dispatched when a LUKS image file is double-clicked and confirmed as LUKS.
    UnlockLuksImage { path: PathBuf },
    /// Asks the user for the passphrase of a LUKS image no saved passphrase unlocked.
    PromptLuksPassphrase {
        path: PathBuf,
        /// `true` when a saved passphrase was rejected - shows an error hint in the dialog.
        wrong_passphrase: bool,
    },
    /// Dispatched by the background thread after successful unlock + mount.
    LuksMounted {
        image_path: PathBuf,
        mount_point: PathBuf,
    },
    /// Triggers the asynchronous unmounting of a system drive or mounted volume.
    UnmountDevice(std::path::PathBuf),
}
