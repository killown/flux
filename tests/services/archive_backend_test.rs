use flux::services::archive::{
    extract_dir_to_tempdir_with_backend, extract_entry_to_tempfile_with_backend,
    list_archive_entries_with_backend, ArchiveBackend, ArchiveEntry, ArchiveError,
};
use std::path::Path;
use std::path::PathBuf;

// ─── Mock backend (defined only in tests) ────────────────────────────────────

#[derive(Clone)]
struct MockBackend {
    list_result: Option<Result<Vec<ArchiveEntry>, ArchiveError>>,
    extract_result: Option<Result<Vec<u8>, ArchiveError>>,
    extract_dir_result: Option<Result<PathBuf, ArchiveError>>,
}

impl MockBackend {
    fn new() -> Self {
        Self {
            list_result: None,
            extract_result: None,
            extract_dir_result: None,
        }
    }

    fn with_list(mut self, result: Result<Vec<ArchiveEntry>, ArchiveError>) -> Self {
        self.list_result = Some(result);
        self
    }

    fn with_extract(mut self, result: Result<Vec<u8>, ArchiveError>) -> Self {
        self.extract_result = Some(result);
        self
    }

    fn with_extract_dir(mut self, result: Result<PathBuf, ArchiveError>) -> Self {
        self.extract_dir_result = Some(result);
        self
    }
}

impl ArchiveBackend for MockBackend {
    fn list_entries(
        &self,
        _archive_path: &Path,
        _prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        self.list_result
            .clone()
            .unwrap_or(Err(ArchiveError::Other("mock not set".into())))
    }

    fn extract_entry_bytes(
        &self,
        _archive_path: &Path,
        _inner_path: &str,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        self.extract_result
            .clone()
            .unwrap_or(Err(ArchiveError::Other("mock not set".into())))
    }

    fn extract_dir(
        &self,
        _archive_path: &Path,
        _inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        self.extract_dir_result
            .clone()
            .unwrap_or(Err(ArchiveError::Other("mock not set".into())))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_list_requires_password() {
    let mock = MockBackend::new().with_list(Err(ArchiveError::PasswordRequired));
    let result =
        list_archive_entries_with_backend(Path::new("test.zip"), "", None, Some(Box::new(mock)));
    assert!(matches!(result, Err(ArchiveError::PasswordRequired)));
}

#[test]
fn test_list_wrong_password() {
    let mock = MockBackend::new().with_list(Err(ArchiveError::WrongPassword));
    let result = list_archive_entries_with_backend(
        Path::new("test.zip"),
        "",
        Some("wrong"),
        Some(Box::new(mock)),
    );
    assert!(matches!(result, Err(ArchiveError::WrongPassword)));
}

#[test]
fn test_list_success() {
    let entries = vec![ArchiveEntry {
        name: "file1.txt".into(),
        is_dir: false,
        size: 123,
        mtime: 0,
        inner_path: "file1.txt".into(),
        child_count: 0,
        is_encrypted: false,
    }];
    let mock = MockBackend::new().with_list(Ok(entries));
    let result =
        list_archive_entries_with_backend(Path::new("test.zip"), "", None, Some(Box::new(mock)))
            .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "file1.txt");
}

#[test]
fn test_extract_entry_success() {
    let data = b"hello world".to_vec();
    let mock = MockBackend::new().with_extract(Ok(data.clone()));
    let tmp = extract_entry_to_tempfile_with_backend(
        Path::new("test.zip"),
        "file.txt",
        None,
        Some(Box::new(mock)),
    )
    .unwrap();
    let content = std::fs::read(tmp.path()).unwrap();
    assert_eq!(content, data);
}

#[test]
fn test_extract_wrong_password() {
    let mock = MockBackend::new().with_extract(Err(ArchiveError::WrongPassword));
    let result = extract_entry_to_tempfile_with_backend(
        Path::new("test.7z"),
        "secret.txt",
        Some("wrong"),
        Some(Box::new(mock)),
    );
    assert!(matches!(result, Err(ArchiveError::WrongPassword)));
}

#[test]
fn test_extract_dir_unsupported() {
    let mock = MockBackend::new().with_extract_dir(Err(ArchiveError::Other("unsupported".into())));
    let result = extract_dir_to_tempdir_with_backend(
        Path::new("test.rar"),
        "folder/",
        None,
        Some(Box::new(mock)),
    );
    assert!(matches!(result, Err(ArchiveError::Other(_))));
}
