# Changelog

All notable changes to Flux are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.5.0] - 2026-10-20

Flux 0.5.0 turns the preview panel into a real viewer: folders open as an
interactive grid, the panel can go full-screen, and video/audio can play in
sequence. It also removes two sources of main-thread stalls when folders
contain large custom icons.

### Added

- **Preview:** directories now open as an interactive grid instead of a flat
  listing. Navigate into folders and open files directly from the panel.
- **Preview:** full-screen mode, toggled by double-clicking the preview
  content. Double-click again or press `Esc` to leave.
- **Preview:** playlist mode for video and audio. When a track or clip ends,
  the panel advances to the next item automatically. Non-playable items are
  skipped, and playback stops at the end of the list.
- **Preview:** prev/next arrows are now shown only while the pointer is over
  the panel, so they no longer sit on top of content that isn't being
  interacted with.

### Changed

- Custom raster icons are decoded with a progressive downscale
  (`image::thumbnail`) instead of decoding at source resolution and scaling
  afterwards. Roughly 3–5x faster on multi-megapixel sources.

### Fixed

- Custom file icons are no longer decoded on the GTK main thread. Opening a
  folder with large custom icons no longer stalls the UI while each icon is
  decoded.
- Custom file icon decodes are throttled through the thumbnail semaphore, so
  a folder with hundreds of large icons can no longer starve thumbnail
  generation and file operations on the shared blocking pool.


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
