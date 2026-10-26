#[test]
fn unescape_mount_field_decodes_octal() {
    use flux::infra::mounts::unescape_mount_field;
    assert_eq!(unescape_mount_field(r"/media/My\040Disk"), "/media/My Disk");
    assert_eq!(unescape_mount_field(r"\040"), " ");
    assert_eq!(unescape_mount_field(r"foo\bar"), r"foo\bar");
    assert_eq!(unescape_mount_field(r"\777"), r"\777");
}

#[test]
fn unescape_mount_field_handles_edge_cases() {
    use flux::infra::mounts::unescape_mount_field;
    assert_eq!(unescape_mount_field(""), "");
    assert_eq!(unescape_mount_field(r"a\134b"), r"a\b");
    assert_eq!(unescape_mount_field(r"tab\011end"), "tab\tend");
    // Truncated or non-octal sequences are kept verbatim.
    assert_eq!(unescape_mount_field(r"\04"), r"\04");
    assert_eq!(unescape_mount_field(r"\089"), r"\089");
    assert_eq!(unescape_mount_field(r"trailing\"), r"trailing\");
    // Values above 0xFF are not decoded.
    assert_eq!(unescape_mount_field(r"\400"), r"\400");
    // Bytes that are not valid UTF-8 are replaced, never a panic.
    assert_eq!(unescape_mount_field(r"\377"), "\u{FFFD}");
}

#[test]
fn parse_mountinfo_reads_the_mount_point_column() {
    use flux::infra::mounts::parse_mountinfo;
    let content = "\
36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 - ext3 /dev/root rw
37 35 98:1 / /media/My\\040Disk rw - vfat /dev/sdb1 rw
short line
";
    assert_eq!(parse_mountinfo(content), vec!["/mnt2", "/media/My Disk"]);
    assert!(parse_mountinfo("").is_empty());
}

#[test]
fn parse_fstab_skips_comments_blanks_and_non_paths() {
    use flux::infra::mounts::parse_fstab;
    let content = "\
# static file system information
   # indented comment

UUID=abcd / ext4 defaults 0 1
/dev/sdb1 /mnt/My\\040Disk ext4 defaults 0 2
/dev/sda2 none swap sw 0 0
onlyonefield
";
    assert_eq!(parse_fstab(content), vec!["/", "/mnt/My Disk"]);
    assert!(parse_fstab("").is_empty());
}
