use flux::ui::file_ops::build_execution_command;
use std::path::Path;
use std::path::PathBuf;

#[test]
fn test_build_execution_command_single_target_substitutions() {
    let template = "echo %p %d %f";
    let targets = vec![PathBuf::from("/home/user/docs/file.txt")];
    let cwd = Path::new("/home/user/docs");

    let (cmd, label) = build_execution_command(template, &targets, cwd);
    assert_eq!(label, "file.txt");
    assert_eq!(
        cmd,
        "echo '/home/user/docs/file.txt' '/home/user/docs' 'file.txt'"
    );
}

#[test]
fn test_build_execution_command_handles_single_quotes() {
    let template = "cat %f";
    let targets = vec![PathBuf::from("/tmp/O'Reilly's Book.pdf")];
    let cwd = Path::new("/tmp");

    let (cmd, label) = build_execution_command(template, &targets, cwd);
    assert_eq!(label, "O'Reilly's Book.pdf");
    assert_eq!(cmd, "cat 'O'\\''Reilly'\\''s Book.pdf'");
}

#[test]
fn test_build_execution_command_blocks_newlines_and_null_bytes() {
    let template = "echo %p";
    let targets_newline = vec![PathBuf::from("/tmp/bad\nname.txt")];
    let cwd = Path::new("/tmp");

    let (cmd, label) = build_execution_command(template, &targets_newline, cwd);
    assert!(
        cmd.is_empty(),
        "Command with newline in target must be rejected"
    );
    assert!(label.is_empty());

    let targets_cr = vec![PathBuf::from("/tmp/bad\rname.txt")];
    let (cmd_cr, _) = build_execution_command(template, &targets_cr, cwd);
    assert!(cmd_cr.is_empty());
}

#[test]
fn test_build_execution_command_cwd_placeholder() {
    let template = "tar -czf %d/archive.tar.gz %f";
    let targets = vec![PathBuf::from("/workspace/src/lib.rs")];
    let cwd = Path::new("/workspace/src");

    let (cmd, _) = build_execution_command(template, &targets, cwd);
    assert_eq!(cmd, "tar -czf '/workspace/src'/archive.tar.gz 'lib.rs'");
}

#[test]
fn test_build_execution_command_multi_target_placeholder() {
    let template = "rm -f %p";
    let targets = vec![
        PathBuf::from("/tmp/file1.txt"),
        PathBuf::from("/tmp/file 2.txt"),
        PathBuf::from("/tmp/file'3.txt"),
    ];
    let cwd = Path::new("/tmp");

    let (cmd, label) = build_execution_command(template, &targets, cwd);
    assert_eq!(label, "3 items");
    assert_eq!(
        cmd,
        "rm -f '/tmp/file1.txt' '/tmp/file 2.txt' '/tmp/file'\\''3.txt'"
    );
}
