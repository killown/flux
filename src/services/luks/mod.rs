//! LUKS encrypted image detection and mount/unmount via udisksctl + key-file.
//!
//! Layout:
//!   * [`image`]    - `LuksImage` newtype
//!   * [`probe`]    - read-only sysfs/procfs probes
//!   * [`mount`]    - `unlock_and_mount`
//!   * [`unmount`]  - `unmount_and_lock`

mod image;
mod mount;
mod probe;
mod unmount;

pub use image::LuksImage;
pub use mount::unlock_and_mount;
#[allow(unused_imports)]
pub use probe::{find_mount_point, is_luks_image};
