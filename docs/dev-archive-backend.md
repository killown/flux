# Adding an Archive Backend

Flux dispatches archive reads through the `ArchiveBackend` trait
(`src/infra/archive/backend.rs`). Every supported format is one
implementation of that trait plus a small set of registrations scattered
across the archive service. Adding a format means touching these files in
order, missing any one is either a compile error or a format that lists
but never extracts (or vice versa).

Read `src/infra/archive/backends/zip.rs` first if the new format is
seekable and single-file. Read `backends/tar.rs` first if the new format
is stream-oriented or needs an external decompressor. Read
`backends/rar.rs` first if the new format has to shell out to a CLI
tool.

## Checklist

1. **`src/infra/archive/backends/<fmt>.rs`** - new file implementing
    `ArchiveBackend`. The trait requires:
    - `list_entries` - returns immediate children of `prefix`, with
      `name`, `is_dir`, `size`, `mtime`, `inner_path`. Route through
      `entry::collect_entry` so synthesised directory nodes and
      `child_count` are populated correctly.
    - `extract_entry_bytes` - returns one entry's raw bytes.
    - `extract_dir` - extracts a subtree to a `tempfile::TempDir` and
      returns its root.

    Override the provided `remove_entry`, `write_entry`,
    `create_directory` only if the format supports in-place editing. The
    defaults return `ArchiveError::Other(...)`, which is the correct
    behaviour for read-only formats.

2. **`src/infra/archive/backends/mod.rs`** - declare the module and
    re-export the struct:

        mod <fmt>;
        pub(in crate::infra::archive) use self::<fmt>::<Fmt>Backend;

    Exports that other modules need (e.g. `extract_dir_<fmt>_at` for the
    fast path in `extract_archive_to_dir`) go here too.

3. **`src/infra/archive/backend.rs`** - two edits in `get_backend`:
    - Add an `else if name_lc.ends_with(".<ext>") { return Box::new(<Fmt>Backend); }`
      arm. Order matters: the `.tar.gz` family must stay grouped because
      every one of them begins with `.tar`.
    - Add a MIME fallback in the `match mime.as_str() { ... }` block for
      cases where the extension is absent or non-standard (e.g. `.cbz`
      which is a ZIP in disguise, or MIME types that arrive from a network
      share).

4. **`src/infra/archive/format.rs`** - two functions:
    - `is_browsable_archive` - add the extension to the `matches_extension`
      list so the grid treats the file as enterable.
    - `is_supported_archive` - add the extension to the `ends_with` chain
      so the CLI `--preview` and `EnterArchive` accept it.

    If only the MIME type is supported (no extension), extend the
    `matches!` on `mime.as_str()` in `is_browsable_archive` instead.

5. **`src/utils/media/mime.rs`** - add the MIME type to
    `guess_mime_from_extension`'s table _if_ the extension is novel enough
    that GIO's content-type database doesn't already know it. Most common
    formats are already covered, skip this step for those.

6. **`README.md`** - add a row to the supported-formats table (or the
    equivalent block in `docs/development.md` if it lives there).

7. **`po/pt_BR.po`** - any user-facing string the backend emits
    (`ArchiveError::Other(...)` messages that reach a toast). Group under
    `# src/infra/archive/backends/<fmt>.rs`.

## Where the reference code lives

- `backends/zip.rs` - the fullest example: streaming read,
  `by_index_raw` for raw copying, password handling, in-archive editing
  and the `.flux-cache/` thumbnail writer.
- `backends/tar.rs` - stream-oriented, one wrapper per decompressor
  (`flate2`, `bzip2`, `xz2`, `zstd`, `lz4_flex`). Use this when the
  format has no random access.
- `backends/rar.rs` - external CLI wrapper (`unar` or `unrar`). Use this
  when the format has no pure-Rust library.
- `backends/sevenz.rs` - pure-Rust random-access archive with AES256.
- `backends/iso.rs` - hybrid: native reader for ISO 9660, `7z` fallback
  for UDF. Use this if the format has more than one inner filesystem.
- `backends/single.rs` - the "one virtual entry" shape (`.gz`, `.bz2`,
  `.xz`, `.zst`, `.lz4` standalone). There is nothing to list
  `list_entries` returns a single synthesised entry named after the
  stripped extension.
- `backends/unsupported.rs` - the fallback. Every `get_backend` chain
  ends here, do not delete it.

## Behaviour rules

- **Never block the main thread.** Backends run inside
  `relm4::spawn_blocking` or a Rayon pool, but a backend that itself
  blocks on a subprocess is fine because it is already off the GTK
  thread. The rule is about _the caller_, not the backend.
- **Never panic on malformed input.** Every backend is fuzzed
  (`fuzz/fuzz_targets/fuzz_archive_*.rs`). Return `ArchiveError::Other`
  for truncated headers, corrupt central directories, out-of-range
  offsets, and any I/O error.
- **Reject path traversal.** A `ZipFile` entry named `../../etc/passwd`
  must be rejected in `extract_entry_bytes` (return `Other("invalid path")`)
  and silently skipped in `extract_dir` (as `tar.rs` does with
  `has_traversal`). The archive is untrusted input.
- **Return inner paths with forward slashes.** Windows-built archives
  use `\`, normalise with `.replace('\\', "/")` before routing through
  `collect_entry`.
- **Preserve `is_dir` for synthesised nodes.** An entry named `foo/` with
  size 0 is a directory. An entry named `foo/` with size 0 that has
  children is still a directory. Do not infer from size alone.
- **Forward the password argument, never cache it.** The password lives
  on `FluxApp::cached_archive_password` and is forwarded by the caller.
  A backend that stores it in a field survives past a session and
  leaks it into the next archive that happens to use the same struct.
- **Respect `ArchiveError::PasswordRequired` vs `WrongPassword`.** The
  UI uses these to decide whether to prompt or to show "incorrect
  password, try again". A generic `Other` message makes the encrypted
  archive prompt permanently stuck.

## Verifying the result

Once wired up, the new format should:

- appear in the `--help` supported list (`is_supported_archive`)
- open with `EnterArchive` and `--preview`
- list, navigate into a subfolder, and back out
- extract a single file (`Enter` on a file, or `Space` for a preview)
- extract the whole archive via the header button
- return `PasswordRequired` on an encrypted archive with no password,
  and `WrongPassword` on a wrong password, if the format supports
  encryption
- pass the corresponding fuzz target (`cargo fuzz run fuzz_archive_<fmt>`
  if you add one, or reuse `fuzz_archive_tree` by feeding it a sample).
