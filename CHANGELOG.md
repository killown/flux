# Changelog

All notable changes to Flux are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-10-20

Flux 0.3.0 adds a `--preview` CLI flag and fixes two issues in the wide-preview mode introduced in 0.2.6.

### Features

- add `--preview` flag to open a folder with wide preview

### Bug Fixes

- open the `--preview` panel immediately and select the first item
- stop the wide-preview toggle from growing the window past the screen

### Chores

- reset the changelog, keep only 0.2.0+
- reorganize `pt_BR.po` by source file


## [0.2.5] - 2026-10-19

### Fixed

- Preview panel: window could grow beyond the screen width when opened
  in list mode. The grid's minimum width now no longer pushes the window,
  and the panel keeps a fixed minimum in wide mode instead of adding its
  natural width on top of the list.

### Added

- `Space` now toggles the preview panel _and_ the MIME filter together.
  `Ctrl+Space` and `Esc` close both. Explicit `Ctrl+M` filters survive
  closing the preview panel.
- Text and table previews are now clamped to a max content width so the
  floating prev/next arrows no longer overlap the content in wide mode.
- PDF previews open at 50% of the panel width by default (zoom still works).

### Translations

- Updated `pt_BR` for the new help strings.
