//! `unlock_and_mount_with` against a scripted stand-in for `udisksctl`: every
//! call is logged, and each failure after the loop attachment must roll back
//! what the call created.

use flux::services::luks::{unlock_and_mount_with, LuksImage};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const LOOP_OK: &str = "Mapped file x as /dev/loop99.";
const UNLOCK_OK: &str = r#"echo "Unlocked /dev/loop99 as /dev/mapper/fluxtest-vault.""#;
const MOUNT_OK: &str = r#"echo "Mounted /dev/mapper/fluxtest-vault at /run/media/u/vault.""#;

/// Writes the fake `udisksctl` into `dir`, `unlock` and `mount` are the shell
/// snippets run for those subcommands. Returns the script and its call log.
fn fake_udisksctl(dir: &Path, unlock: &str, mount: &str) -> (PathBuf, PathBuf) {
    let log = dir.join("calls.log");
    let script = dir.join("udisksctl");
    let body = format!(
        "#!/bin/sh\n\
         printf '%s|%s\\n' \"$LC_ALL\" \"$*\" >> '{log}'\n\
         case \"$1\" in\n\
         loop-setup) echo \"{LOOP_OK}\";;\n\
         unlock) {unlock};;\n\
         mount) {mount};;\n\
         esac\n\
         exit 0\n",
        log = log.display(),
    );
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (script, log)
}

/// Runs the unlock sequence and returns its result with the logged
/// subcommands (and the locale each call ran under).
fn run(unlock: &str, mount: &str) -> (Result<PathBuf, String>, Vec<String>, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let image_path = dir.path().join("vault.img");
    std::fs::write(&image_path, b"x").unwrap();
    let (script, log) = fake_udisksctl(dir.path(), unlock, mount);

    let result = unlock_and_mount_with(&script, &LuksImage { path: image_path }, "pw");

    let logged = std::fs::read_to_string(log).unwrap_or_default();
    let mut locales = Vec::new();
    let mut calls = Vec::new();
    for line in logged.lines() {
        let (locale, args) = line.split_once('|').unwrap();
        locales.push(locale.to_string());
        calls.push(args.split_whitespace().next().unwrap().to_string());
    }
    (result, calls, locales)
}

#[test]
fn success_returns_mount_point_and_keeps_everything_open() {
    let (result, calls, _) = run(UNLOCK_OK, MOUNT_OK);
    assert_eq!(result.unwrap(), PathBuf::from("/run/media/u/vault"));
    assert_eq!(calls, ["loop-setup", "unlock", "mount"]);
}

#[test]
fn every_call_runs_in_the_c_locale() {
    let (_, calls, locales) = run(UNLOCK_OK, MOUNT_OK);
    assert_eq!(locales.len(), calls.len());
    assert!(locales.iter().all(|l| l == "C"), "{locales:?}");
}

#[test]
fn mount_failure_locks_and_deletes_the_loop() {
    let (result, calls, _) = run(UNLOCK_OK, "echo 'mount broke' >&2; exit 1");
    assert!(result.unwrap_err().contains("mount broke"));
    assert_eq!(
        calls,
        ["loop-setup", "unlock", "mount", "lock", "loop-delete"]
    );
}

#[test]
fn unparsable_mount_output_unmounts_locks_and_deletes_the_loop() {
    let (result, calls, _) = run(
        UNLOCK_OK,
        r#"echo "Montado /dev/mapper/fluxtest-vault em /srv/x.""#,
    );
    assert!(result.unwrap_err().contains("mount point"));
    assert_eq!(
        calls,
        [
            "loop-setup",
            "unlock",
            "mount",
            "unmount",
            "lock",
            "loop-delete"
        ]
    );
}

#[test]
fn unparsable_unlock_output_locks_and_deletes_the_loop() {
    let (result, calls, _) = run(r#"echo "Desbloqueado algo""#, MOUNT_OK);
    assert!(result.unwrap_err().contains("dm device"));
    assert_eq!(calls, ["loop-setup", "unlock", "lock", "loop-delete"]);
}

#[test]
fn rejected_passphrase_deletes_the_loop_without_locking() {
    let (result, calls, _) = run("echo 'Bad passphrase' >&2; exit 1", MOUNT_OK);
    assert!(result.is_err());
    assert_eq!(calls, ["loop-setup", "unlock", "loop-delete"]);
}

#[test]
fn other_unlock_failure_returns_stderr_and_deletes_the_loop() {
    let (result, calls, _) = run("echo 'Device busy' >&2; exit 1", MOUNT_OK);
    assert!(result.unwrap_err().contains("Device busy"));
    assert_eq!(calls, ["loop-setup", "unlock", "loop-delete"]);
}
