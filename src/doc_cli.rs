//! Renderer for `docs/cli.md`.

use crate::args::CLI_OPTIONS;
use std::fmt::Write as _;

/// Returns the complete `docs/cli.md`, prose and generated option table.
pub fn render_cli_markdown() -> String {
    let mut out = String::new();

    out.push_str("# Command line\n\n");
    out.push_str(
        "```text\n\
         flux [OPTIONS] [PATH]\n\
         ```\n\n\
         The binary is built as `flux-fm`; `--help` refers to it as `flux`. \
         `PATH` may be a plain path or a `file://` URI; the prefix is stripped and the rest is not percent-decoded.\n\n",
    );

    out.push_str("## Opening a location\n\n");
    out.push_str(
        "| `PATH` | Result |\n\
         |---|---|\n\
         | none | Opens the home directory. |\n\
         | a directory | Opens it. |\n\
         | a supported archive file | Opens the archive explorer on it. |\n\
         | `#name`, `:tag:name`, `:t:name` or `tags://name` | Opens the home directory filtered by the tag `name`. |\n\n",
    );

    out.push_str("## Options\n\n");
    out.push_str("<!-- BEGIN GENERATED: options -->\n");
    out.push_str("| Option | Description |\n");
    out.push_str("|---|---|\n");
    for (flags, description) in CLI_OPTIONS {
        let _ = writeln!(
            out,
            "| `{}` | {} |",
            flags.trim_start(),
            description.replace('<', "&lt;").replace('>', "&gt;")
        );
    }
    out.push_str("<!-- END GENERATED: options -->\n\n");

    out.push_str("## Session flags\n\n");
    out.push_str(
        "`--no-sidebar`, `--no-header`, `--no-statusbar` and `--terminal` may \
         appear anywhere on the command line. They apply to the window opened by \
         `PATH`, a tag, `--preview`, `--quick-list` or `--quick-list-stdin`, \
         and are ignored by the other options.\n\n\
         Every other option must be the first argument once the session flags \
         are removed. Arguments after the first are read only by the options \
         that take values.\n\n\
         `--terminal` opens the embedded terminal in the position configured \
         under Settings → Terminal: the bottom pane, the right sidebar panel or \
         the full window. Combined with `--preview`, both open unless the \
         position is full window, where the terminal takes over the whole \
         window and `--preview` is superseded.\n\n",
    );

    out.push_str("## Custom icons\n\n");
    out.push_str(
        "`--set-icon` stores the icon in `config.toml`. For a directory it also \
         updates any matching pinned sidebar entry. `TARGET` and `IMAGE` are \
         resolved to canonical absolute paths, and the command fails if `IMAGE` does not \
         exist.\n\n\
         `--set-icons-stdin` reads one pair per line. Separate the two paths \
         with a tab, or write two words and quote or escape any that contain \
         spaces. Blank lines and lines starting with `#` are ignored. A bad \
         line is reported on stderr with its line number and skipped, and the \
         command exits with status 1 after applying the rest.\n\n\
         ```text\n\
         /home/me/Music /home/me/icons/music.svg\n\
         \"/home/me/My Videos\" /home/me/icons/video.svg\n\
         ```\n\n\
         `--reset-icons-stdin` reads one target path per line, with the same \
         rules for blank lines and comments.\n\n",
    );

    out.push_str("## Quick list\n\n");
    out.push_str(
        "`--quick-list` opens the first path and fills the quick panel with all \
         the paths given. Arguments that start with `-` are not treated as \
         paths. `--quick-list-stdin` reads one path per line, skipping blank \
         lines and `#` comments, and opens the home directory when the input is \
         empty.\n\n\
         ```text\n\
         find . -name '*.log' | flux --quick-list-stdin\n\
         ```\n\n",
    );

    out.push_str("## Selection\n\n");
    out.push_str(
        "`--selection` asks a running window for the paths selected in its active \
         tab and prints them, one per line. It talks to the window named by \
         `$FLUX_PID`, which Flux sets in its embedded terminal, and otherwise to \
         the window that navigated last. It does not open a window.\n\n\
         A path containing a newline is skipped with a warning; `--selection -0` \
         terminates each path with NUL instead, which keeps every path intact \
         for `xargs -0`.\n\n\
         ```text\n\
         flux-fm --selection | xargs -d '\\n' ls -l\n\
         ```\n\n",
    );

    out.push_str("## Exit status\n\n");
    out.push_str(
        "| Status | Meaning |\n\
         |---|---|\n\
         | `0` | Success. Missing values for `--file-properties`, `--set-icon`, `--reset-icon` and `--quick-list` print the help text and also exit with `0`. |\n\
         | `1` | Unrecognized option, `--preview` without a directory, an icon file that does not exist, a skipped line in `--set-icons-stdin`, or `--selection` finding no running window. |\n",
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{resolve_startup_action_with_reader, StartupAction};
    use std::io::Cursor;
    use std::path::PathBuf;

    #[test]
    fn doc_is_in_sync() {
        let generated = render_cli_markdown();
        let committed = std::fs::read_to_string("docs/cli.md")
            .expect("docs/cli.md is missing; create it from render_cli_markdown()");
        assert_eq!(
            generated, committed,
            "\ndocs/cli.md is stale.\n\
             Run `cargo run --bin gen-docs` to regenerate it."
        );
    }

    #[test]
    fn every_listed_option_is_recognized() {
        for (flags, _) in CLI_OPTIONS {
            let long = flags
                .split_whitespace()
                .find(|word| word.starts_with("--"))
                .expect("every option has a long form");
            let args: Vec<String> = ["flux", long, "/", "/"]
                .iter()
                .map(|s| s.to_string())
                .collect();
            let action =
                resolve_startup_action_with_reader(&args, PathBuf::from("/"), Cursor::new(""));
            assert!(
                !matches!(
                    action,
                    StartupAction::UnknownFlag(_) | StartupAction::InvalidArgument(_)
                ),
                "{long} is listed in CLI_OPTIONS but the parser rejects it"
            );
        }
    }
}
