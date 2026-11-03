# Changelog

All notable changes to Troph are recorded here. The canonical version is the four-part
`major.minor.patch.build` value in [`VERSION`](VERSION).

When preparing a release:

1. Run `scripts/bump-version.sh` with `--major`, `--minor`, `--patch`, or `--build` to increment
   the chosen component and sync the Cargo workspace and npm package versions.
2. Add a new section at the top of this file.

If you edit `VERSION` manually, run `node scripts/sync-version.mjs` afterward to sync the Cargo
workspace and npm package versions.

## [1.2.0.2] - 2026-11-03

### Added

- Live transfer speed in the upload panel and collapsed pill, sampled and smoothed over the
  upload so the rate stays readable instead of jumping between chunks.
- `formatSpeed()` helper for human-readable bytes-per-second values.
- `scripts/build-android.sh` to build and install the Android app in one step: builds the web
  UI, cross-compiles the native library with `cargo-ndk`, builds the APK with Gradle, and
  installs it with `adb`. Supports `--release`, `--no-install`, `--no-web`, `--launch`, and
  `--serial`.

### Changed

- `scripts/bump-version.sh` now takes `--major`, `--minor`, `--patch`, or `--build`, validates
  that `VERSION` has four numeric components, resets the lower components when a higher one is
  incremented, and syncs the Cargo workspace and npm package versions.
- README and changelog versioning instructions now describe the per-component bump workflow.

## [1.1.3.1] - 2026-11-02

### Fixed

- Dragging a folder from the OS file manager onto the web page now uploads its contents
  recursively, preserving the folder name and nested structure, instead of treating the folder
  as a single failed file.
- Parallel uploads into the same new folder no longer fail when they race to create it.

## [1.1.3.0] - 2026-11-02

### Added

- Parallel uploads: up to four files upload at once instead of one at a time.
- Aggregate upload progress and estimated time remaining in the upload panel and collapsed pill.
- `formatDuration()` helper for human-readable ETA values.

### Changed

- Queued uploads remember their destination folder, so navigating during an upload does not
  send later files to the wrong place.
- Uploads refresh the current listing once when the batch finishes instead of after each file.
- Upload completion reports the number of failed uploads.

### Documentation

- Documented parallel upload behavior in `README.md` and `PRODUCT.md`.
- Added this changelog and synced the root npm package version.

## [1.1.2.3] - 2026-11-02

### Added

- Transfer tracking in the Rust core, including upload name, loaded bytes, and total size.
- Upload progress indicators on the web, desktop, and Android host surfaces.
- Desktop tray tooltip and "Receiving" card for active transfers.
- Android notification progress and in-app transfer list.
- Collapsible upload panel with persisted state and a scrollable task list.

### Changed

- Upload panel now orders active, queued, failed, and completed tasks.
- Desktop and Android transfer displays update while uploads are running.

## [1.1.0.0] - 2026-11-02

### Added

- Tauri desktop host with server lifecycle controls, tray support, QR code, folder/file pickers,
  and launch-at-login.
- npm workspaces for the web and desktop front ends.
- Desktop control panel for managing shared items and server settings.

### Changed

- Reorganized the repository into `apps/` and `crates/`.
- Expanded README and product documentation for desktop and Android.

## [1.0.1.1] - 2026-11-01

### Added

- `VERSION` as the canonical app version and version sync scripts.
- Version display in the web UI footer.
- Impeccable UI overhaul: new app shell, menus, dialogs, theming, responsive layout, and shared
  Android UI components.

### Changed

- Android screens updated to use the shared theme and UI components.
- Web client updated with the new visual system and selection actions.

## Initial development - 2026-11-01

### Added

- Initial Troph Android file server with Rust core.
- PIN authentication, session cookies, CSRF protection, and login rate limiting.
- Path sandboxing, symlink refusal, and blocked Android data directories.
- File listing, streaming with byte ranges, uploads, ZIP downloads, thumbnails, and mDNS
  advertising.
- Android foreground service, share management, and embedded web client.
