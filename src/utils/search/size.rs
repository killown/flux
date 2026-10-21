#[derive(Debug, Clone, PartialEq)]
pub enum SizeOp {
    Gt(u64),
    Lt(u64),
    Range(u64, u64),
}

/// Units accepted by [`parse_size`]. The multiplier is the byte count per
/// unit. Kept as a `pub const` so `render_search_markdown` documents exactly
/// the units the parser accepts.
pub const ACCEPTED_SIZE_UNITS: &[(&str, u64)] = &[
    ("k", 1024),
    ("m", 1024 * 1024),
    ("g", 1024 * 1024 * 1024),
    ("t", 1024 * 1024 * 1024 * 1024),
];

/// Unit suffixes stripped from a size token before it is parsed as a number.
/// `>10MB`, `>10MiB`, and `>10M` all resolve to the same threshold.
pub const SIZE_UNIT_SUFFIXES: &[&str] = &["ib", "b"];

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
    if num_end == 0 {
        return None;
    }

    let (num_part, unit_part) = s.split_at(num_end);
    if num_part.contains('.') {
        return None;
    }

    let n: u64 = num_part.parse().ok()?;

    let unit_clean = unit_part.trim().to_lowercase();
    let unit = SIZE_UNIT_SUFFIXES
        .iter()
        .find_map(|suffix| unit_clean.strip_suffix(suffix))
        .unwrap_or(&unit_clean);

    if unit.is_empty() {
        return None;
    }

    let multiplier = ACCEPTED_SIZE_UNITS
        .iter()
        .find_map(|(name, mult)| (*name == unit).then_some(*mult))?;

    n.checked_mul(multiplier)
}
