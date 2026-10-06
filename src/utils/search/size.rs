#[derive(Debug, Clone, PartialEq)]
pub enum SizeOp {
    Gt(u64),
    Lt(u64),
    Range(u64, u64),
}

/// Parse size filter from query.
/// Returns (SizeOp, remaining_query) or None.
pub fn parse_size_filter(query: &str) -> Option<(SizeOp, String)> {
    let query_lc = query.to_lowercase();
    let parts: Vec<&str> = query_lc.split_whitespace().collect();

    for (i, part) in parts.iter().enumerate() {
        if let Some(op) = parse_size_op(part) {
            let rest = parts
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, s)| *s)
                .collect::<Vec<_>>()
                .join(" ");
            return Some((op, rest));
        }
    }
    None
}

fn parse_size_op(s: &str) -> Option<SizeOp> {
    if let Some((left, right)) = s.split_once("..") {
        let l = parse_size(left)?;
        let r = parse_size(right)?;
        return Some(SizeOp::Range(l, r));
    }

    if let Some(rest) = s.strip_prefix('>') {
        return Some(SizeOp::Gt(parse_size(rest)?));
    }
    if let Some(rest) = s.strip_prefix('<') {
        return Some(SizeOp::Lt(parse_size(rest)?));
    }

    None
}

fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let num_end = s
        .find(|c: char| !c.is_numeric() && c != '.')
        .unwrap_or(s.len());

    if num_end == 0 || num_end == s.len() {
        return None;
    }

    let (num_part, unit_part) = s.split_at(num_end);
    let n: u64 = num_part.parse().ok()?;

    let unit_clean = unit_part.trim().to_lowercase();
    let unit = unit_clean
        .strip_suffix("ib")
        .or_else(|| unit_clean.strip_suffix('b'))
        .unwrap_or(&unit_clean);

    let bytes = match unit {
        "k" | "kb" => n.checked_mul(1024)?,
        "m" | "mb" => n.checked_mul(1024 * 1024)?,
        "g" | "gb" => n.checked_mul(1024 * 1024 * 1024)?,
        "t" | "tb" => n.checked_mul(1024 * 1024 * 1024 * 1024)?,
        _ => return None,
    };
    Some(bytes)
}
