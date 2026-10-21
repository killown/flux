//! Fish function definitions that bridge a shell and the running Flux window.
//!
//! One source of truth, two consumers:
//! * `services::terminal::fish_theme` embeds them in the `conf.d` snippet for
//!   Flux's embedded terminal.
//! * `gen-docs` writes them to `data/fish/flux-integration.fish`, which can be
//!   installed into fish's `vendor_conf.d` so any terminal gets them.
//!
//! Flux publishes two small files under `$XDG_CACHE_HOME/flux/`: `cwd` (the
//! directory shown in the grid) and `pid` (the window that last navigated).
//! `goto` flows the other way: a shell writes a directory to it and that
//! window navigates. The selection is not a file: `flux-fm --selection` asks
//! the window for it at the moment it is needed.

/// Returns the fish source, unindented, without a guard or marker line.
pub fn render_functions() -> String {
    FUNCTIONS.to_string()
}

const FUNCTIONS: &str = r#"function __flux_cache_dir
    if test -n "$XDG_CACHE_HOME"
        echo $XDG_CACHE_HOME/flux
    else
        echo $HOME/.cache/flux
    end
end

function flux_selection --description 'Print the paths selected in Flux, one per line'
    argparse --max-args 0 n/names d/dir -- $argv; or return
    set -l paths (command flux-fm --selection 2>/dev/null); or return 0
    test (count $paths) -gt 0; or return 0
    if set -q _flag_names
        path basename $paths
    else if set -q _flag_dir
        path dirname $paths | sort -u
    else
        printf '%s\n' $paths
    end
end

function flux_cwd --description "Print the directory shown in Flux's grid"
    set -l file (__flux_cache_dir)/cwd
    test -f $file; and cat $file
end

function flux_goto --description 'Navigate the open Flux window to PATH (default: .), or start Flux'
    set -q argv[1]; or set argv .
    set -l target (path resolve $argv[1])
    if not test -e $target
        echo "flux_goto: no such path: $argv[1]" >&2
        return 1
    end
    test -d $target; or set target (path dirname $target)
    set -l dir (__flux_cache_dir)
    set -l pid (cat $dir/pid 2>/dev/null)
    if test -n "$pid"; and test -r /proc/$pid/comm; and string match -q flux-fm (cat /proc/$pid/comm)
        printf '%s\n' $target >$dir/goto
    else
        command flux-fm $target &
        disown
    end
end

function __flux_sync_env --on-event fish_preexec
    set -gx FCWD (flux_cwd)
    if string match -q '*FLUX_SELECTION*' -- $argv[1]
        set -gx FLUX_SELECTION (flux_selection | string escape | string join ' ')
    end
end

if status is-interactive
    abbr --add --position anywhere '$F' '(flux_selection)'
    abbr --add --position anywhere '$FN' '(flux_selection --names)'
    abbr --add --position anywhere '$FD' '(flux_selection --dir)'
end
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defines_every_helper() {
        let body = render_functions();
        for needle in [
            "function flux_selection",
            "flux-fm --selection",
            "function flux_cwd",
            "function flux_goto",
            "--on-event fish_preexec",
            "abbr --add --position anywhere '$F'",
            "abbr --add --position anywhere '$FN'",
            "abbr --add --position anywhere '$FD'",
        ] {
            assert!(body.contains(needle), "missing: {needle}");
        }
    }
}
