//! Helpers for building `sh -c` command strings from user templates.

/// Single-quotes `s` for a POSIX shell. Returns `None` for text that cannot
/// be passed safely (newline, carriage return or NUL).
pub fn shell_safe(s: &str) -> Option<String> {
    if s.contains('\n') || s.contains('\r') || s.contains('\0') {
        return None;
    }
    Some(format!("'{}'", s.replace('\'', "'\\''")))
}

/// What a `%x` token in a command template stands for.
pub enum Placeholder {
    /// Not a placeholder: the text is kept exactly as written.
    Unknown,
    /// A value that is already shell-quoted. Quote characters written around
    /// the token in the template (`"%p"` or `'%p'`) are consumed with it.
    Shell(String),
    /// An unquoted value that is inserted as is (the line number).
    Plain(String),
    /// A value cannot be used safely, so the whole command is abandoned.
    Reject,
}

/// Expands the placeholders of `template` in a single left-to-right pass.
///
/// Substituted values are written straight to the output and are never
/// scanned again. Expanding with chained `str::replace` calls re-scans the
/// output of earlier replacements, so a file named `,touch pwn,%f` would have
/// its own `%f` expanded inside the quotes around it and break out of them.
///
/// `lookup` receives each two-character token (for example `"%p"`). Returns
/// `None` when `lookup` answers [`Placeholder::Reject`].
pub fn expand_template(
    template: &str,
    mut lookup: impl FnMut(&str) -> Placeholder,
) -> Option<String> {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::with_capacity(template.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // A token wrapped in matching quotes, e.g. `"%p"` or `'%p'`.
        if (c == '"' || c == '\'')
            && i + 3 < chars.len()
            && chars[i + 1] == '%'
            && chars[i + 3] == c
        {
            let token: String = chars[i + 1..i + 3].iter().collect();
            match lookup(&token) {
                Placeholder::Shell(value) => {
                    out.push_str(&value);
                    i += 4;
                    continue;
                }
                Placeholder::Reject => return None,
                Placeholder::Plain(_) | Placeholder::Unknown => {}
            }
        }

        if c == '%' && i + 1 < chars.len() {
            let token: String = chars[i..i + 2].iter().collect();
            match lookup(&token) {
                Placeholder::Shell(value) | Placeholder::Plain(value) => {
                    out.push_str(&value);
                    i += 2;
                    continue;
                }
                Placeholder::Reject => return None,
                Placeholder::Unknown => {}
            }
        }

        out.push(c);
        i += 1;
    }

    Some(out)
}
