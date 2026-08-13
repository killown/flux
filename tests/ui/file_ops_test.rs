use flux::ui::file_ops::{build_execution_command, should_track_in_transfer_dialog};
use std::path::PathBuf;

#[test]
fn test_paste_text_parsing_cut_flag() {
    let text = "cut\nfile:///tmp/a.txt\nfile:///tmp/b.txt";
    let mut lines = text.lines();
    let first_line = lines.next().unwrap_or("");

    let is_cut = first_line == "cut";
    let uris: Vec<&str> = lines.filter(|u| !u.is_empty()).collect();

    assert!(is_cut);
    assert_eq!(uris.len(), 2);
    assert_eq!(uris[0], "file:///tmp/a.txt");
    assert_eq!(uris[1], "file:///tmp/b.txt");
}

#[test]
fn test_paste_text_parsing_copy_flag() {
    let text = "file:///tmp/a.txt\nfile:///tmp/b.txt";
    let mut lines = text.lines();
    let first_line = lines.next().unwrap_or("");

    let is_cut = first_line == "cut";

    assert!(!is_cut);
}

#[test]
fn test_confirm_replace_paste_message_formatting() {
    let single_conflict = vec!["Documents".to_string()];
    let body_single = if single_conflict.len() == 1 {
        format!(
            "\"{}\" already exists in this location. Replace it and merge its contents?",
            single_conflict[0]
        )
    } else {
        format!(
            "{} folders already exist in this location. Replace them and merge their contents?",
            single_conflict.len()
        )
    };

    assert_eq!(
        body_single,
        "\"Documents\" already exists in this location. Replace it and merge its contents?"
    );

    let multi_conflicts = vec!["Folder1".to_string(), "Folder2".to_string()];
    let body_multi = if multi_conflicts.len() == 1 {
        format!(
            "\"{}\" already exists in this location. Replace it and merge its contents?",
            multi_conflicts[0]
        )
    } else {
        format!(
            "{} folders already exist in this location. Replace them and merge their contents?",
            multi_conflicts.len()
        )
    };

    assert_eq!(
        body_multi,
        "2 folders already exist in this location. Replace them and merge their contents?"
    );
}

#[test]
fn test_drop_destination_path_resolution() {
    let source_path = PathBuf::from("/tmp/source_folder/file.txt");
    let dest_dir = PathBuf::from("/tmp/destination_folder");

    let file_name = source_path.file_name().unwrap();
    let final_dest = dest_dir.join(file_name);

    assert_eq!(
        final_dest,
        PathBuf::from("/tmp/destination_folder/file.txt")
    );
}

#[test]
fn test_command_template_paths_arg_formatting() {
    let targets = vec![
        PathBuf::from("/tmp/file1.txt"),
        PathBuf::from("/tmp/file with spaces.txt"),
    ];

    let paths_arg = targets
        .iter()
        .map(|p| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ");

    assert_eq!(paths_arg, "'/tmp/file1.txt' '/tmp/file with spaces.txt'");
}

#[test]
fn test_is_recursive_paste_detection() {
    use std::path::Path;

    let is_recursive_paste = |src: &Path, dest_dir: &Path| -> bool { dest_dir.starts_with(src) };

    let parent = Path::new("/home/user/Documents");
    let child = Path::new("/home/user/Documents/Projects/Flux");
    let unrelated = Path::new("/home/user/Downloads");

    assert!(is_recursive_paste(parent, child));
    assert!(!is_recursive_paste(child, parent));
    assert!(!is_recursive_paste(parent, unrelated));
}

#[test]
fn test_command_template_file_properties_bypasses_transfer_dialog() {
    let cmd_template = "flux-fm --file-properties %p";
    assert!(!should_track_in_transfer_dialog(cmd_template));
}

#[test]
fn test_command_template_standard_enables_transfer_dialog() {
    let cmd_template = "ffmpeg -i %p %p.mp4";
    assert!(should_track_in_transfer_dialog(cmd_template));
}

#[test]
fn test_build_execution_command_single_file_escaping() {
    let template = "echo %p %d %f";
    let targets = vec![PathBuf::from("/tmp/my folder/test 'file'.txt")];
    let cwd = PathBuf::from("/tmp/my folder");

    let (cmd, label) = build_execution_command(template, &targets, &cwd);

    assert_eq!(label, "test 'file'.txt");
    assert!(cmd.contains("'/tmp/my folder/test '\\''file'\\''.txt'"));
}

#[test]
fn test_build_execution_command_multi_file() {
    let template = "zip archive.zip %p";
    let targets = vec![
        PathBuf::from("/tmp/file1.txt"),
        PathBuf::from("/tmp/file2.txt"),
    ];
    let cwd = PathBuf::from("/tmp");

    let (cmd, label) = build_execution_command(template, &targets, &cwd);

    assert_eq!(label, "2 items");
    assert_eq!(cmd, "zip archive.zip '/tmp/file1.txt' '/tmp/file2.txt'");
}
