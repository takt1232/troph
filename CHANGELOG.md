# Changelog

All notable changes to Troph are recorded here. The canonical version is the four-part
`major.minor.patch.build` value in [`VERSION`](VERSION).

When preparing a release:

1. Update `VERSION`, or run `scripts/bump-version.sh` to increment the build component.
2. Run `node scripts/sync-version.mjs` to sync the Cargo workspace and npm package versions.
3. Add a new section at the top of this file.

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
