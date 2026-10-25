//! LUKS encrypted image detection and mount/unmount via udisksctl + key-file.
//!
//! Layout:
//!   * [`image`]    - `LuksImage` newtype
//!   * [`probe`]    - read-only sysfs/procfs probes
//!   * [`mount`]    - `unlock_and_mount`
//!   * [`parse`]    - parsers for C-locale `udisksctl` output

mod image;
mod mount;
mod parse;
mod probe;

pub use image::LuksImage;
pub use mount::unlock_and_mount;
#[allow(unused_imports)]
pub use mount::unlock_and_mount_with;
#[allow(unused_imports)]
pub use parse::{is_wrong_passphrase, parse_dm_device, parse_loop_device, parse_mount_point};
#[allow(unused_imports)]
pub use probe::{find_mount_point, is_luks_image};
