// Temporary compatibility shim: every module now lives under `crate::infra`.
// Removed once call sites use the new paths.
pub use crate::infra::archive;
pub use crate::infra::constants;
pub use crate::infra::db;
pub use crate::infra::fonts;
pub use crate::infra::git;
pub use crate::infra::inspector;
pub use crate::infra::loader;
pub use crate::infra::luks;
pub use crate::infra::mounts;
pub use crate::infra::network;
pub use crate::infra::search;
pub use crate::infra::tasks;
pub use crate::infra::terminal;
pub use crate::infra::thumbnails;
pub use crate::infra::trash;
pub use crate::infra::window_control;
