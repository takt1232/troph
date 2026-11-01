# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

Both surfaces are in scope, with the browser file manager as the primary craft surface and the native Android app maintained alongside it. The Android app is a Kotlin interface for controlling the server, sharing storage, and granting permissions; the web client is the Svelte 5 file manager opened on a computer.

## Users

A single self-hoster moving their own files between an Android phone and a computer on the same Wi-Fi network. They want no cloud service, no account, and no cable, and they run the server themselves on trusted hardware.

No multi-user, team, or shared-device audience has been confirmed. The PIN protects access, but the product is designed around one owner.

## Product Purpose

Troph turns an Android phone into a small, self-hosted file server. Start it on the phone, open the address it shows in a browser on the same network, and browse, stream, upload, and manage files.

Success means the owner can move or manage files from their computer quickly, with nothing leaving the local network and no setup beyond starting the server.

## Positioning

A self-contained Android file server whose web client is embedded in the native binary. There is no cloud, no account, no cable, and no separate web deployment; files stay on the device and the LAN, and the phone is both the storage source and the server.

A neighboring cloud-transfer product could not truthfully copy this combination of local-only data, embedded UI, and phone-as-server operation.

## Operating Context

- The phone runs the server as a foreground service while the computer browses over the same Wi-Fi or LAN.
- The owner grants **All files access**, and on Android 13+ notification permission, then chooses shared folders/files or enables **Share everything**.
- The server address and PIN are displayed on the phone with a QR code; the computer user opens the address and enters the PIN.
- Sharing changes restart the server automatically while keeping the same PIN and browser session.
- The browser client is used for browsing, streaming, previewing, uploading, creating folders, renaming, moving, copying, deleting, and ZIP downloads.
- A desktop dev binary runs the same core for development with image thumbnails.
- The product targets Android 11 (API 30) or newer on `arm64-v8a`; the server listens on port `55432` by default and advertises over mDNS as `Troph-<hostname>`.

## Capabilities and Constraints

Confirmed functionality:

- Browse folders and files from any modern browser.
- Stream video and audio, and preview images and PDFs with byte-range support.
- Generate and cache thumbnails for images and videos.
- Upload individual files or entire folders with progress.
- Create folders; rename, move, copy, and delete items.
- Download one file or a selection as a streamed ZIP archive.
- Share selected folders/files, or optionally all storage.
- PIN login with rate limiting, session cookies, and CSRF protection.
- mDNS advertising at `http://<hostname>.local:<port>`.
- Path sandboxing, symlink refusal, and automatic blocking of `Android/data` and `Android/obb`.
- No runtime web assets to deploy: the Svelte front end is embedded in the native binary.

Durable constraints:

- Designed for trusted local networks. It does not terminate TLS; remote access requires a VPN, SSH tunnel, or reverse proxy with TLS. It must not be exposed directly to the internet.
- Anyone with the PIN and network access can read and modify shared storage.
- Uploads are capped at 100 GiB.
- All accessible paths must resolve inside a configured root; symbolic links are refused.
- Server state is tied to Android storage permissions and the foreground service.

Undecided or unconfirmed:

- No cross-platform phone support has been confirmed; the app is Android-only today.
- No cloud sync, account system, multi-user roles, or hosted service is part of the product.

## Brand Commitments

- Product and UI name: **Troph**. Repository and crate names remain `pocketfiles`.
- Tagline: “Your phone's files, in your computer's browser.”
- Voice: plain, direct, calm, and practical. Copy states what happens and what the user must do, without hype.
- Existing positioning language centers on privacy, local-only operation, and the absence of cloud services, accounts, and cables.

## Evidence on Hand

- `README.md` documents the product story, features, architecture, build steps, API, and security model.
- Runnable source demonstrates the browser client, Android app, Rust server, desktop dev binary, and JNI bridge.
- Real UI copy and error states exist in the Svelte components and Kotlin activities.

Absences future work must not fabricate: there are no testimonials, customer logos, case studies, press mentions, benchmarks, pricing, licensing claims beyond MIT, or deployment claims on hand.

## Product Principles

1. **Local by default.** Files and transfers stay on the device and the LAN. Never imply cloud storage, accounts, or remote services that do not exist.
2. **The phone is the source; the browser is the workplace.** Keep Android setup and control precise, and make the computer-side file manager the primary craft surface.
3. **Remove transfer friction.** Every flow should reduce the awkwardness of moving files between phone and computer: no cables, no extra apps, no ad-filled tools.
4. **Security is structural.** PIN auth, path sandboxing, restricted Android directories, and LAN-only operation are product features, not implementation details to paper over.
5. **Self-contained delivery.** The web UI ships inside the native binary; avoid product decisions that introduce a separate deployment or runtime asset step.
