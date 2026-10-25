use flux::services::luks::{
    is_wrong_passphrase, parse_dm_device, parse_loop_device, parse_mount_point,
};
use std::path::PathBuf;

#[test]
fn loop_device_is_read_from_loop_setup_output() {
    assert_eq!(
        parse_loop_device("Mapped file /home/u/vault.img as /dev/loop3.\n").as_deref(),
        Some("/dev/loop3")
    );
    assert_eq!(parse_loop_device("Mapped file x as /dev/sda1."), None);
    assert_eq!(parse_loop_device(""), None);
}

#[test]
fn dm_device_is_read_from_unlock_output() {
    assert_eq!(
        parse_dm_device("Unlocked /dev/loop3 as /dev/dm-1.\n").as_deref(),
        Some("/dev/dm-1")
    );
    assert_eq!(
        parse_dm_device("Unlocked /dev/loop3 as /dev/mapper/luks-abc.").as_deref(),
        Some("/dev/mapper/luks-abc")
    );
    assert_eq!(parse_dm_device("Unlocked /dev/loop3 as /dev/sda1."), None);
}

#[test]
fn mount_point_is_read_from_c_locale_output() {
    assert_eq!(
        parse_mount_point("Mounted /dev/dm-1 at /run/media/u/vault.\n"),
        Some(PathBuf::from("/run/media/u/vault"))
    );
    assert_eq!(
        parse_mount_point("Mounted /dev/dm-1 at /run/media/u/my disk."),
        Some(PathBuf::from("/run/media/u/my disk"))
    );
}

#[test]
fn mount_point_is_none_for_translated_or_empty_output() {
    assert_eq!(
        parse_mount_point("Montado /dev/dm-1 em /run/media/u/vault.\n"),
        None
    );
    assert_eq!(parse_mount_point("Mounted /dev/dm-1 at \n"), None);
    assert_eq!(parse_mount_point(""), None);
}

#[test]
fn wrong_passphrase_matches_both_udisks_messages() {
    assert!(is_wrong_passphrase(
        "Error unlocking /dev/loop0: GDBus.Error:org.freedesktop.UDisks2.Error.Failed: Bad passphrase"
    ));
    assert!(is_wrong_passphrase("No key available with this passphrase"));
    assert!(!is_wrong_passphrase("Device busy"));
}
