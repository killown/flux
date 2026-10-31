//! Archive browsing, extraction and passwords.

use crate::domain::model::{ArchiveEntry, ArchiveError};
use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ArchiveMsg {
    /// Prompts the user for confirmation before permanently deleting an entry from an archive.
    PromptArchiveDeletion {
        archive_path: PathBuf,
        inner_path: String,
    },
    /// Asks whether to extract an archive that lives inside another archive
    /// (`a.zip` containing `b.zip`) so it can be browsed.
    PromptNestedArchiveExtract {
        archive_path: PathBuf,
        inner_path: String,
    },
    /// Extracts the nested archive entry next to its outer archive, then
    /// enters the extracted copy with [`ArchiveMsg::EnterArchive`].
    ExtractNestedArchive {
        archive_path: PathBuf,
        inner_path: String,
    },
    /// Extracts the currently browsed archive to a sibling folder named.
    ExtractArchive,
    /// Delivers the result of an async archive directory listing.
    ArchiveLoaded {
        archive_path: PathBuf,
        prefix: String,
        password: Option<String>,
        /// Session identifier captured at spawn time, used to discard results
        /// from background tasks that were superseded by a subsequent navigation.
        load_id: u64,
        result: Result<Vec<ArchiveEntry>, ArchiveError>,
    },
    /// Background ZIP header probe result: archives found to be encrypted.
    /// Sent after `FolderLoaded` so the padlock never delays the first paint.
    ArchiveLocksProbed {
        load_id: u64,
        encrypted: Vec<PathBuf>,
    },
    /// Draws browsable archives as folders in the file grid. Persisted in the
    /// config file, archives stay files (`is_dir` is unchanged).
    SetArchivesAsFolders(bool),
    /// Enter a compressed archive and browse its contents as a virtual directory.
    ///
    /// The `PathBuf` is the real on-disk path of the archive file. The app
    /// synthesises an `archive://` URI internally and delegates rendering to
    /// `load_archive` in `services/loader.rs`.
    EnterArchive(PathBuf),
    /// Show a password dialog then retry loading the archive.
    ///
    /// Triggered by [`ArchiveError::PasswordRequired`] or [`ArchiveError::WrongPassword`]
    /// from `load_archive`. The dialog dispatches [`ArchiveMsg::LoadArchiveWithPassword`] on
    /// confirmation.
    PromptArchivePassword {
        archive_path: PathBuf,
        prefix: String,
        /// `true` when a previous attempt failed - shows an error hint in the dialog.
        wrong_password: bool,
    },
    /// Retry loading an archive after the user supplied a password.
    LoadArchiveWithPassword {
        archive_path: PathBuf,
        prefix: String,
        password: String,
    },
    /// Request that an archive password be saved to the system keyring.
    SaveArchivePasswordToKeyring {
        archive_path: PathBuf,
        password: String,
    },
    /// Delete the keyring entry for an archive and drop the session copy.
    ForgetArchivePassword(PathBuf),
}
