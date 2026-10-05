//! Format-specific archive backends.

// ─── Real backends ────────────────────────────────────────────────────────────

mod deb;
mod iso;
mod rar;
mod sevenz;
mod single;
mod tar;
mod unsupported;
mod zip;

pub(in crate::services::archive) use self::deb::DebBackend;
pub(in crate::services::archive) use self::iso::IsoBackend;
pub(in crate::services::archive) use self::rar::RarBackend;
pub(in crate::services::archive) use self::sevenz::{extract_dir_7z_at, SevenZBackend};
pub(in crate::services::archive) use self::single::SingleFileBackend;
pub(in crate::services::archive) use self::tar::{extract_dir_tar_at, TarBackend};
pub(in crate::services::archive) use self::unsupported::UnsupportedBackend;
pub(in crate::services::archive) use self::zip::{extract_dir_zip_at, ZipBackend};
