//! Text decoding for the peek preview.

/// A decoded text head plus the encoding label shown in the Details tab.
pub(crate) struct Decoded {
    pub text: String,
    pub label: &'static str,
}

/// How many leading bytes are inspected when deciding whether data is text.
const PROBE_BYTES: usize = 8192;

/// Decodes the head of a file, or returns `None` when it looks binary.
pub(crate) fn decode_text(bytes: &[u8], hit_cap: bool, texty_mime: bool) -> Option<Decoded> {
    crate::hit!("decode_text");
    if let Some(decoded) = decode_utf16_bom(bytes, hit_cap) {
        return Some(decoded);
    }

    let probe = &bytes[..bytes.len().min(PROBE_BYTES)];
    if probe.contains(&0) {
        return None;
    }

    let (body, bom) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (rest, true),
        None => (bytes, false),
    };
    let utf8_label = if bom { "UTF-8 (BOM)" } else { "UTF-8" };
    match std::str::from_utf8(body) {
        Ok(s) => {
            return Some(Decoded {
                text: s.to_owned(),
                label: utf8_label,
            });
        }
        Err(e) if hit_cap && e.error_len().is_none() => {
            let ok = std::str::from_utf8(&body[..e.valid_up_to()]).ok()?;
            return Some(Decoded {
                text: ok.to_owned(),
                label: utf8_label,
            });
        }
        Err(_) => {}
    }

    if !texty_mime && !looks_like_text(probe) {
        return None;
    }
    let lossy = String::from_utf8_lossy(body);
    let bad = lossy.chars().filter(|&c| c == '\u{FFFD}').count();
    let non_ascii = lossy.chars().filter(|c| !c.is_ascii()).count();
    if bad > 0 && bad == non_ascii {
        return Some(Decoded {
            text: body.iter().map(|&b| cp1252_char(b)).collect(),
            label: "Windows-1252 (guessed)",
        });
    }
    Some(Decoded {
        text: lossy.into_owned(),
        label: "UTF-8 (lossy)",
    })
}

/// UTF-16 with a byte order mark. Without a BOM the encoding cannot be told
/// apart from binary data reliably, so those files stay in the hex view.
fn decode_utf16_bom(bytes: &[u8], hit_cap: bool) -> Option<Decoded> {
    let (big_endian, rest) = bytes
        .strip_prefix(&[0xFF, 0xFE])
        .map(|r| (false, r))
        .or_else(|| Some((true, bytes.strip_prefix(&[0xFE, 0xFF])?)))?;

    if !big_endian && rest.starts_with(&[0, 0]) {
        None?;
    }

    let even = &rest[..rest.len() & !1];
    let mut units: Vec<u16> = even
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[a, b]| {
            if big_endian {
                u16::from_be_bytes([a, b])
            } else {
                u16::from_le_bytes([a, b])
            }
        })
        .collect();

    if hit_cap && matches!(units.last(), Some(u) if (0xD800..0xDC00).contains(u)) {
        units.pop();
    }

    let mut lossy = false;
    let text: String = char::decode_utf16(units)
        .map(|r| {
            r.unwrap_or_else(|_| {
                lossy = true;
                '\u{FFFD}'
            })
        })
        .collect();
    let label = match (big_endian, lossy) {
        (false, false) => "UTF-16 LE",
        (false, true) => "UTF-16 LE (lossy)",
        (true, false) => "UTF-16 BE",
        (true, true) => "UTF-16 BE (lossy)",
    };
    Some(Decoded { text, label })
}

/// No control characters other than tab, line feeds, form feed and escape.
fn looks_like_text(probe: &[u8]) -> bool {
    probe
        .iter()
        .all(|&b| (b >= 0x20 && b != 0x7F) || matches!(b, b'\t' | b'\n' | b'\r' | 0x0C | 0x1B))
}

/// Windows-1252 differs from Latin-1 only in 0x80..=0x9F.
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

fn cp1252_char(b: u8) -> char {
    match b {
        0x80..=0x9F => {
            let cp = u32::from(CP1252_HIGH[usize::from(b - 0x80)]);
            char::from_u32(cp).unwrap_or('\u{FFFD}')
        }
        _ => b as char,
    }
}
