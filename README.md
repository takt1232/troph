# Troph

**Your phone's files, in your computer's browser.**

Troph is a small, self-hosted file server for Android. Start it on your phone, open the
address it shows in a browser on the same Wi-Fi network, and browse, stream, upload, and
manage files without cloud services, accounts, or cables.

The project is called `pocketfiles` in the repository and crate names; the product and UI
are named **Troph**.

## Why

Moving files between a phone and a computer is usually awkward: cloud uploads, USB drivers,
messaging apps, or ad-filled transfer tools. Troph runs entirely on the device and serves a
mobile-friendly web app over the local network. Nothing leaves your LAN.

## Features

- Browse folders and files from any modern browser
- Stream video and audio, and preview images and PDFs with byte-range support
- Generate and cache thumbnails for images and videos
- Upload individual files or entire folders with progress
- Create folders, rename, move, copy, and delete items
- Download one file or a selection as a streamed ZIP archive
- Share selected folders/files, or optionally all storage
- PIN login with rate limiting, session cookies, and CSRF protection
- mDNS advertising so the server can be reached at `http://<hostname>.local:<port>`
- Path sandboxing, symlink refusal, and automatic blocking of `Android/data` and
  `Android/obb`
- No runtime web assets to deploy: the Svelte front end is embedded in the native binary

## Architecture

```
pocketfiles/
├── crates/
│   ├── core/       Rust library: HTTP server, auth, path sandbox, file ops, ZIP, thumbnails
│   ├── dev/        Native desktop/dev binary that runs the core with image thumbnails
│   └── android/    JNI cdylib exposing the core to the Android app
├── android/        Kotlin Android app: UI, foreground service, sharing, JNI bridges
├── web/            Svelte 5 + TypeScript + Vite front end
└── Cargo.toml      Rust workspace
```

- **`pocketfiles-core`** is a library built on [Axum](https://github.com/tokio-rs/axum) and
  Tokio. It embeds the built web UI with `rust-embed` and exposes a JSON API plus static
  assets.
- **`pocketfiles-dev`** runs the same core on desktop for development. It enables the
  `dev-thumbs` feature, which uses the `image` crate to generate image thumbnails.
- **`pocketfiles-android`** is a `cdylib` named `pocketfiles` (`libpocketfiles.so`) loaded by
  the Android app. It implements thumbnails and media scanning by calling back into Kotlin
  via JNI.
- **`web`** is the browser client. Production builds are written to `web/dist` and compiled
  into the Rust binary; during development Vite proxies `/api` to the local server.

## Requirements

- A recent stable Rust toolchain and Cargo
- Node.js 20+ and npm
- Android SDK with NDK, plus JDK 17 for the Android build
- `cargo-ndk` for cross-compiling to Android (`cargo install cargo-ndk`)
- Android device or emulator running Android 11 (API 30) or newer; the build targets
  `arm64-v8a`

## Build and run the web UI

The Rust server embeds the contents of `web/dist` at compile time, so build the front end
before compiling Rust:

```sh
cd web
npm install
npm run build
```

For front-end development with live reload, keep the Rust dev server running (see below)
and start Vite in another terminal:

```sh
cd web
npm run dev
```

Vite serves the app at `http://127.0.0.1:5173` and proxies API requests to port 55432.

## Run the desktop dev server

The dev binary serves the built web UI and is the fastest way to try Troph without a phone:

```sh
cargo run -p pocketfiles-dev -- --pin 123456 --root ~/Pictures --root ~/Documents
```

Options:

| Flag | Description |
| --- | --- |
| `--port <PORT>` | Listening port (default `55432`) |
| `--pin <PIN>` | 4–12 digit login PIN (default `123456`) |
| `--root <PATH>` | Folder or file to share; repeatable. Defaults to the current directory |
| `--cache <PATH>` | Thumbnail cache directory (default: system temp dir `/troph-cache`) |

Open the URL printed on startup, for example `http://192.168.1.20:55432`, and enter the PIN.
Set `RUST_LOG=info` for logs.

Useful checks:

```sh
cargo test
```

## Build the Android app

1. Build the web UI as described above.
2. Cross-compile the native library into the app's JNI folder. Set `ANDROID_NDK_HOME` (or
   `NDK_HOME`) to your installed NDK first:

   ```sh
   cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build -p pocketfiles-android --release
   ```

   This produces `android/app/src/main/jniLibs/arm64-v8a/libpocketfiles.so`.

3. Build the APK:

   ```sh
   cd android
   ./gradlew assembleDebug
   ```

   The debug APK is written to `android/app/build/outputs/apk/debug/app-debug.apk`.

On first launch, grant **All files access** and, on Android 13+, notification permission.
Then:

1. Open **Manage shared items** and choose folders/files, or enable **Share everything**.
2. Tap **Start server**.
3. Scan the QR code, or type the displayed address and PIN into a browser on the same
   network.

Sharing changes automatically restart the server while keeping the same PIN and browser
session.

## How it works

### Authentication

The server requires a numeric PIN. A successful login creates a random session token stored
in the `pf_session` cookie (`HttpOnly`, `SameSite=Strict`, 12-hour lifetime). State-changing
requests must also include the per-process CSRF token in the `x-csrf-token` header. Login
attempts are limited to 8 per IP per minute. PIN comparisons use constant-time equality.

### File sandbox

All accessible paths must resolve inside a configured root. Canonicalization is applied
before the sandbox check, symbolic links are refused for reads/writes/copies, and operations
cannot escape a root. `Android/data` and `Android/obb` are always blocked, matching modern
Android storage restrictions.

### Transfer behavior

- `/api/raw` supports `Range` and `If-None-Match` for streaming and resumable media playback.
- Uploads stream to a temporary file then rename into place, with a 100 GiB cap.
- ZIP downloads are generated and streamed on the fly; already-compressed media is stored
  without recompression.
- Thumbnails are keyed by path, size, file length, and modification time, cached on disk,
  and evicted with a 256 MiB LRU-ish cleanup.

### Networking

The server binds `0.0.0.0` on the selected port (default `55432`) and advertises an HTTP
service over mDNS as `Troph-<hostname>`. Non-loopback IPv4 interfaces are preferred, with
Wi-Fi ranked ahead of Ethernet and tunnels. The Android service holds wake, Wi-Fi, and
multicast locks while running.

## HTTP API

All endpoints except `/api/health`, `/api/login`, and static assets require an authenticated
session. Mutating endpoints also require the CSRF header (the ZIP form uses a `csrf` field).

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/health` | Server name and version |
| `POST` | `/api/login` | Exchange a PIN for a session cookie |
| `POST` | `/api/logout` | Destroy the current session |
| `GET` | `/api/me` | CSRF token, version, hostname, device name, roots |
| `GET` | `/api/roots` | Configured shared roots |
| `GET` | `/api/list?path=` | Directory listing |
| `GET` | `/api/list/roots` | Virtual "Shared items" listing |
| `GET` | `/api/thumb?path=&size=` | Cached image/video thumbnail |
| `GET` | `/api/raw?path=&dl=1` | Stream or download a file |
| `POST` | `/api/zip` | Streamed ZIP of selected paths (form encoded) |
| `POST` | `/api/upload?dir=&name=&overwrite=` | Stream an upload |
| `POST` | `/api/mkdir` | Create a folder |
| `POST` | `/api/rename` | Rename an item |
| `POST` | `/api/delete` | Delete one or more paths |
| `POST` | `/api/move` | Move paths into a destination folder |
| `POST` | `/api/copy` | Copy paths into a destination folder |

JSON requests use paths returned by the listing API. Errors are returned as
`{ "error": "message" }` with an appropriate HTTP status.

## Security notes

Troph is designed for trusted local networks and does **not** terminate TLS. Do not expose
it directly to the internet; use a VPN, SSH tunnel, or a reverse proxy with TLS if you need
remote access. Anyone with the PIN and network access can read and modify shared storage.

## License

MIT. See `Cargo.toml` for the workspace license declaration.
