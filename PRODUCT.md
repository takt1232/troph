# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

Troph has three surfaces: the browser file manager, the native Android app, and the native
desktop app. The browser file manager is the primary craft surface. The Android app is a Kotlin
interface for controlling the server, sharing storage, and granting permissions. The desktop app
is a Tauri host with a Svelte control panel for sharing local folders/files, starting and
stopping the server, and opening the browser client.

## Users

A single self-hoster moving their own files between devices on the same Wi-Fi network. They want
no cloud service, no account, and no cable, and they run the server themselves on trusted
hardware.

No multi-user, team, or shared-device audience has been confirmed. The PIN protects access, but
the product is designed around one owner.

## Product Purpose

Troph turns a phone or computer into a small, self-hosted file server. Start it on the host,
open the address it shows in a browser on the same network, and browse, stream, upload, and
manage files.

Success means the owner can move or manage files quickly, with nothing leaving the local network
and no setup beyond starting the server.

## Positioning

A self-contained file server for Android and desktop whose web client is embedded in the native
binary. There is no cloud, no account, no cable, and no separate web deployment; files stay on
the device and the LAN, and the host is both the storage source and the server.

A neighboring cloud-transfer product could not truthfully copy this combination of local-only
data, embedded UI, and host-as-server operation.

## Operating Context

- The host runs the server while a browser on the same Wi-Fi or LAN browses the shared files.
- On Android, the owner grants **All files access**, and on Android 13+ notification permission,
  then chooses shared folders/files or enables **Share everything**. The server runs as a
  foreground service.
- On desktop, the owner adds folders/files in the Tauri control panel, starts the server, and
  keeps it running from the tray while the window is hidden.
- The server address and PIN are displayed by the host with a QR code; the browser user opens
  the address and enters the PIN.
- Sharing changes restart the server automatically while keeping the same PIN and browser
  session.
- The browser client is used for browsing, streaming, previewing, uploading, creating folders,
  renaming, moving, copying, deleting, and ZIP downloads.
- A desktop CLI binary runs the same core for development with image thumbnails.
- The product targets Android 11 (API 30) or newer on `arm64-v8a`; the server listens on port
  `55432` by default and advertises over mDNS as `Troph-<hostname>`.

## Capabilities and Constraints

Confirmed functionality:

- Browse folders and files from any modern browser.
- Stream video and audio, and preview images and PDFs with byte-range support.
- Generate and cache thumbnails for images and videos on Android, and image thumbnails on desktop.
- Upload individual files or entire folders in parallel, with progress and estimated time remaining.
- Create folders; rename, move, copy, and delete items.
- Download one file or a selection as a streamed ZIP archive.
- Share selected folders/files, or optionally all storage on Android.
- PIN login with rate limiting, session cookies, and CSRF protection.
- mDNS advertising at `http://<hostname>.local:<port>`.
- Path sandboxing, symlink refusal, and automatic blocking of `Android/data` and `Android/obb`.
- No runtime web assets to deploy: the Svelte front end is embedded in the native binary.
- Native desktop host with tray controls, folder/file pickers, QR code, and launch-at-login.

Durable constraints:

- Designed for trusted local networks. It does not terminate TLS; remote access requires a VPN,
  SSH tunnel, or reverse proxy with TLS. It must not be exposed directly to the internet.
- Anyone with the PIN and network access can read and modify shared storage.
- Uploads are capped at 100 GiB.
- All accessible paths must resolve inside a configured root; symbolic links are refused.
- Server state is tied to the host process: the Android foreground service, or the desktop app
  and its tray lifecycle.
- Desktop PINs are generated per server start and are not persisted. Restarting for a sharing
  change keeps the same PIN and session; stopping and starting creates a new PIN.

Undecided or unconfirmed:

- No iOS support has been confirmed.
- Windows and macOS desktop packages are planned but not yet built.
- No cloud sync, account system, multi-user roles, or hosted service is part of the product.
- Desktop video thumbnails are not implemented; image thumbnails are.

## Brand Commitments

- Product and UI name: **Troph**. Repository and crate names remain `pocketfiles`.
- Umbrella tagline: “Your files, in your browser.” Android-specific copy may keep the
  phone-focused phrasing.
- Voice: plain, direct, calm, and practical. Copy states what happens and what the user must do,
  without hype.
- Existing positioning language centers on privacy, local-only operation, and the absence of
  cloud services, accounts, and cables.

## Evidence on Hand

- `README.md` documents the product story, features, architecture, build steps, API, and
  security model.
- Runnable source demonstrates the browser client, Android app, desktop Tauri host, Rust server,
  desktop CLI binary, and JNI bridge.
- Real UI copy and error states exist in the Svelte components, Kotlin activities, and desktop
  control panel.

Absences future work must not fabricate: there are no testimonials, customer logos, case
studies, press mentions, benchmarks, pricing, licensing claims beyond MIT, or deployment claims
on hand.

## Product Principles

1. **Local by default.** Files and transfers stay on the device and the LAN. Never imply cloud
   storage, accounts, or remote services that do not exist.
2. **The host is the source; the browser is the workplace.** Keep host setup and control
   precise, and make the computer-side file manager the primary craft surface.
3. **Remove transfer friction.** Every flow should reduce the awkwardness of moving files
   between devices: no cables, no extra apps, no ad-filled tools.
4. **Security is structural.** PIN auth, path sandboxing, restricted Android directories, and
   LAN-only operation are product features, not implementation details to paper over.
5. **Self-contained delivery.** The web UI ships inside the native binary; avoid product
   decisions that introduce a separate deployment or runtime asset step.
