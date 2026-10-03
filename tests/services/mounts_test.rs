#[test]
fn unescape_mount_field_decodes_octal() {
    use flux::services::mounts::unescape_mount_field;
    assert_eq!(unescape_mount_field(r"/media/My\040Disk"), "/media/My Disk");
    assert_eq!(unescape_mount_field(r"\040"), " ");
    assert_eq!(unescape_mount_field(r"foo\bar"), r"foo\bar");
    assert_eq!(unescape_mount_field(r"\777"), r"\777");
}
