use flux::infra::archive::{
    create_archive_directory, list_archive_entries, remove_archive_entry, write_archive_entry,
    zip_is_encrypted, ArchiveError,
};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::unstable::write::FileOptionsExt;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const PASSWORD: &str = "s3cret-pass";

const ENTRIES: [(&str, &[u8]); 2] = [
    ("one.txt", b"first file body"),
    (
        "two.txt",
        b"second file body, long enough to be worth deflating",
    ),
];

const THIRD: (&str, &[u8]) = ("third.txt", b"third file body added later");

#[derive(Clone, Copy, Debug)]
enum Cipher {
    ZipCrypto,
    Aes256,
}

/// Creates `enc.zip` in `dir` with the two [`ENTRIES`], encrypted with `cipher`.
fn build_encrypted_zip(dir: &Path, cipher: Cipher) -> PathBuf {
    let path = dir.join("enc.zip");
    let mut zip = ZipWriter::new(File::create(&path).unwrap());
    for (name, body) in ENTRIES {
        let base = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let options = match cipher {
            Cipher::ZipCrypto => base
                .with_deprecated_encryption(PASSWORD.as_bytes())
                .unwrap(),
            Cipher::Aes256 => base.with_aes_encryption(zip::AesMode::Aes256, PASSWORD),
        };
        zip.start_file(name, options).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
    path
}

/// Writes the file that gets added to the archive and returns its path.
fn source_file(dir: &Path) -> PathBuf {
    let path = dir.join("source.bin");
    std::fs::write(&path, THIRD.1).unwrap();
    path
}

/// Asserts every entry in `expected` is flagged encrypted and decrypts to its body.
fn assert_encrypted_and_readable(path: &Path, expected: &[(&str, &[u8])], cipher: Cipher) {
    let mut zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
    for (name, body) in expected {
        let idx = zip
            .index_for_name(name)
            .unwrap_or_else(|| panic!("{cipher:?}: {name} is missing"));
        assert!(
            zip.by_index_raw(idx).unwrap().encrypted(),
            "{cipher:?}: {name} lost its encryption flag"
        );
        let mut got = Vec::new();
        zip.by_index_decrypt(idx, PASSWORD.as_bytes())
            .unwrap_or_else(|e| panic!("{cipher:?}: cannot decrypt {name}: {e}"))
            .read_to_end(&mut got)
            .unwrap_or_else(|e| panic!("{cipher:?}: cannot read {name}: {e}"));
        assert_eq!(
            got.as_slice(),
            *body,
            "{cipher:?}: {name} has the wrong content"
        );
    }
}

fn sorted_names(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = list_archive_entries(path, "", Some(PASSWORD))
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    names.sort();
    names
}

fn adding_an_entry_with_the_password_keeps_the_archive_encrypted(cipher: Cipher) {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = build_encrypted_zip(dir.path(), cipher);
    let src = source_file(dir.path());
    assert!(
        zip_is_encrypted(&zip_path),
        "{cipher:?}: fixture is not encrypted"
    );

    write_archive_entry(&zip_path, &src, THIRD.0, Some(PASSWORD), None)
        .unwrap_or_else(|e| panic!("{cipher:?}: write failed: {e}"));

    assert!(
        zip_is_encrypted(&zip_path),
        "{cipher:?}: archive lost its encryption"
    );
    assert_eq!(
        sorted_names(&zip_path),
        ["one.txt", "third.txt", "two.txt"],
        "{cipher:?}: wrong entry list"
    );
    assert_encrypted_and_readable(&zip_path, &[ENTRIES[0], ENTRIES[1], THIRD], cipher);
}

fn adding_an_entry_without_a_password_is_refused_and_changes_nothing(cipher: Cipher) {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = build_encrypted_zip(dir.path(), cipher);
    let src = source_file(dir.path());
    let before = std::fs::read(&zip_path).unwrap();

    let result = write_archive_entry(&zip_path, &src, THIRD.0, None, None);

    assert!(
        matches!(result, Err(ArchiveError::PasswordRequired)),
        "{cipher:?}: expected PasswordRequired, got {result:?}"
    );
    assert_eq!(
        std::fs::read(&zip_path).unwrap(),
        before,
        "{cipher:?}: the archive changed on disk"
    );
}

fn the_other_writers_refuse_without_a_password_and_change_nothing(cipher: Cipher) {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = build_encrypted_zip(dir.path(), cipher);
    let before = std::fs::read(&zip_path).unwrap();

    let removed = remove_archive_entry(&zip_path, "one.txt", None, None);
    let created = create_archive_directory(&zip_path, "newdir", None);

    assert!(
        matches!(removed, Err(ArchiveError::PasswordRequired)),
        "{cipher:?}: remove returned {removed:?}"
    );
    assert!(
        matches!(created, Err(ArchiveError::PasswordRequired)),
        "{cipher:?}: create_directory returned {created:?}"
    );
    assert_eq!(std::fs::read(&zip_path).unwrap(), before);
}

fn a_wrong_password_is_refused_and_changes_nothing(cipher: Cipher) {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = build_encrypted_zip(dir.path(), cipher);
    let src = source_file(dir.path());
    let before = std::fs::read(&zip_path).unwrap();

    let result = write_archive_entry(&zip_path, &src, THIRD.0, Some("not-the-password"), None);

    assert!(result.is_err(), "{cipher:?}: a wrong password was accepted");
    // ZipCrypto only has a 1-byte password check, so about 1 wrong password in
    // 256 passes it and fails later on the checksum as `Other`. Only AES
    // (2-byte check) is held to the exact error.
    if let Cipher::Aes256 = cipher {
        assert!(
            matches!(result, Err(ArchiveError::WrongPassword)),
            "{cipher:?}: expected WrongPassword, got {result:?}"
        );
    }
    assert_eq!(
        std::fs::read(&zip_path).unwrap(),
        before,
        "{cipher:?}: the archive changed on disk"
    );
}

fn removing_an_entry_keeps_the_rest_encrypted(cipher: Cipher) {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = build_encrypted_zip(dir.path(), cipher);

    remove_archive_entry(&zip_path, "one.txt", Some(PASSWORD), None)
        .unwrap_or_else(|e| panic!("{cipher:?}: remove failed: {e}"));

    assert!(
        zip_is_encrypted(&zip_path),
        "{cipher:?}: lost its encryption"
    );
    assert_eq!(sorted_names(&zip_path), ["two.txt"]);
    assert_encrypted_and_readable(&zip_path, &[ENTRIES[1]], cipher);
}

fn creating_a_directory_keeps_the_files_encrypted(cipher: Cipher) {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = build_encrypted_zip(dir.path(), cipher);

    create_archive_directory(&zip_path, "newdir", Some(PASSWORD))
        .unwrap_or_else(|e| panic!("{cipher:?}: create_directory failed: {e}"));

    assert!(
        zip_is_encrypted(&zip_path),
        "{cipher:?}: lost its encryption"
    );
    assert_eq!(sorted_names(&zip_path), ["newdir", "one.txt", "two.txt"]);
    assert_encrypted_and_readable(&zip_path, &ENTRIES, cipher);
}

#[test]
fn zipcrypto_write_with_password_keeps_archive_encrypted() {
    adding_an_entry_with_the_password_keeps_the_archive_encrypted(Cipher::ZipCrypto);
}

#[test]
fn aes_write_with_password_keeps_archive_encrypted() {
    adding_an_entry_with_the_password_keeps_the_archive_encrypted(Cipher::Aes256);
}

#[test]
fn zipcrypto_write_without_password_is_refused_and_file_unchanged() {
    adding_an_entry_without_a_password_is_refused_and_changes_nothing(Cipher::ZipCrypto);
}

#[test]
fn aes_write_without_password_is_refused_and_file_unchanged() {
    adding_an_entry_without_a_password_is_refused_and_changes_nothing(Cipher::Aes256);
}

#[test]
fn zipcrypto_remove_and_mkdir_without_password_are_refused() {
    the_other_writers_refuse_without_a_password_and_change_nothing(Cipher::ZipCrypto);
}

#[test]
fn aes_remove_and_mkdir_without_password_are_refused() {
    the_other_writers_refuse_without_a_password_and_change_nothing(Cipher::Aes256);
}

#[test]
fn zipcrypto_write_with_wrong_password_is_refused_and_file_unchanged() {
    a_wrong_password_is_refused_and_changes_nothing(Cipher::ZipCrypto);
}

#[test]
fn aes_write_with_wrong_password_is_refused_and_file_unchanged() {
    a_wrong_password_is_refused_and_changes_nothing(Cipher::Aes256);
}

#[test]
fn zipcrypto_remove_keeps_remaining_entries_encrypted() {
    removing_an_entry_keeps_the_rest_encrypted(Cipher::ZipCrypto);
}

#[test]
fn aes_remove_keeps_remaining_entries_encrypted() {
    removing_an_entry_keeps_the_rest_encrypted(Cipher::Aes256);
}

#[test]
fn zipcrypto_create_directory_keeps_files_encrypted() {
    creating_a_directory_keeps_the_files_encrypted(Cipher::ZipCrypto);
}

#[test]
fn aes_create_directory_keeps_files_encrypted() {
    creating_a_directory_keeps_the_files_encrypted(Cipher::Aes256);
}

/// An unencrypted archive with no password still accepts new entries.
#[test]
fn unencrypted_archive_still_accepts_entries_without_a_password() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("plain.zip");
    let mut zip = ZipWriter::new(File::create(&zip_path).unwrap());
    zip.start_file("one.txt", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"plain").unwrap();
    zip.finish().unwrap();
    let src = source_file(dir.path());

    write_archive_entry(&zip_path, &src, THIRD.0, None, None).unwrap();

    assert!(!zip_is_encrypted(&zip_path));
    let mut names: Vec<String> = list_archive_entries(&zip_path, "", None)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    names.sort();
    assert_eq!(names, ["one.txt", "third.txt"]);
}
