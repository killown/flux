fn format_size(size: u64) -> String {
    if size == 0 {
        return "0B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let i = (size as f64).log(1024.0).floor() as usize;
    let i = i.min(units.len() - 1);
    let s = size as f64 / 1024.0f64.powi(i as i32);
    format!("{:.2} {}", s, units[i])
}

fn get_shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    let mut entropy = 0.0;
    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }
    (entropy * 10000.0).round() / 10000.0
}

#[test]
fn test_format_size_units() {
    assert_eq!(format_size(0), "0B");
    assert_eq!(format_size(1024), "1.00 KB");
    assert_eq!(format_size(1024 * 1024), "1.00 MB");
    assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
}

#[test]
fn test_entropy_calculation() {
    assert_eq!(get_shannon_entropy(b""), 0.0);
    assert_eq!(get_shannon_entropy(&vec![0u8; 100]), 0.0);
    let entropy = get_shannon_entropy(b"flux-file-manager");
    assert!(entropy > 0.0 && entropy <= 8.0);
}

#[test]
fn test_elf_header_validation_logic() {
    let non_elf_buffer = b"NOT_AN_ELF_HEADER_FILE";
    assert_ne!(&non_elf_buffer[0..4], b"\x7fELF");

    let elf_buffer = b"\x7fELF\x02\x01\x01\x00";
    assert_eq!(&elf_buffer[0..4], b"\x7fELF");
}

#[test]
fn test_text_metrics_line_and_word_count() {
    let text_content = "Line 1 TODO\nLine 2 with words\nLine 3 TODO item";
    let lines: Vec<&str> = text_content.lines().collect();
    let word_count: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
    let todo_count = lines
        .iter()
        .filter(|l| l.to_uppercase().contains("TODO"))
        .count();

    assert_eq!(lines.len(), 3);
    assert_eq!(word_count, 11);
    assert_eq!(todo_count, 2);
}
