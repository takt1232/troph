use crate::auth::{self, clear_cookie, session_cookie};
use crate::error::ApiError;
use crate::files;
use crate::ops;
use crate::paths::PathSandbox;
use crate::thumbs;
use crate::zip_route;
use crate::{
    AppState, MediaScanner, NoopScanner, NoopThumbnails, Root, ServerConfig, ServerStatus,
    ThumbnailProvider, APP_NAME, APP_VERSION, DEFAULT_PORT,
};
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use rand::RngCore;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024 * 1024;
const CSP: &str = "default-src 'self'; img-src 'self' data: blob:; media-src 'self' blob:; \
style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; frame-src 'self'; \
object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'self'";

#[derive(RustEmbed)]
#[folder = "../../apps/web/dist"]
struct WebAssets;

pub struct RunningServer {
    runtime: Option<tokio::runtime::Runtime>,
    shutdown: CancellationToken,
    mdns: Option<MdnsRegistration>,
    auth: Arc<auth::Auth>,
    pub config: ServerConfig,
    pub port: u16,
    pub urls: Vec<String>,
    pub hostname: String,
}

impl RunningServer {
    pub fn status(&self) -> ServerStatus {
        let url = self
            .urls
            .first()
            .cloned()
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", self.port));
        ServerStatus {
            ok: true,
            name: APP_NAME.to_string(),
            version: APP_VERSION.to_string(),
            hostname: self.hostname.clone(),
            hostname_url: format!("http://{}:{}", self.hostname, self.port),
            port: self.port,
            url,
            urls: self.urls.clone(),
            device_name: self.config.device_name.clone(),
        }
    }

    pub fn auth_snapshot(&self) -> auth::AuthSnapshot {
        self.auth.snapshot()
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.mdns.take();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(3));
        }
    }
}

struct MdnsRegistration {
    daemon: mdns_sd::ServiceDaemon,
    fullname: String,
}

impl Drop for MdnsRegistration {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

pub fn start_server(config: ServerConfig) -> Result<RunningServer, String> {
    start_server_with(config, Arc::new(NoopThumbnails), Arc::new(NoopScanner))
}

pub fn start_server_with(
    config: ServerConfig,
    thumbnails: Arc<dyn ThumbnailProvider>,
    scanner: Arc<dyn MediaScanner>,
) -> Result<RunningServer, String> {
    start_server_with_snapshot(config, thumbnails, scanner, None)
}

pub fn start_server_with_snapshot(
    mut config: ServerConfig,
    thumbnails: Arc<dyn ThumbnailProvider>,
    scanner: Arc<dyn MediaScanner>,
    auth_snapshot: Option<auth::AuthSnapshot>,
) -> Result<RunningServer, String> {
    validate_config(&config)?;
    let hostname = normalize_hostname(&config.hostname);
    let device_name = if config.device_name.trim().is_empty() {
        APP_NAME.to_string()
    } else {
        config.device_name.trim().to_string()
    };
    config.hostname = hostname.clone();
    config.device_name = device_name;

    let sandbox = PathSandbox::new(&config.roots)?;
    config.roots = sandbox.roots();
    let cache_dir = match config.cache_dir.as_deref() {
        Some(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => std::env::temp_dir().join("troph-cache"),
    };
    std::fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("cannot create cache directory: {error}"))?;
    let cache_dir = std::fs::canonicalize(&cache_dir)
        .map_err(|error| format!("cannot resolve cache directory: {error}"))?;
    std::fs::create_dir_all(cache_dir.join("thumbnails"))
        .map_err(|error| format!("cannot create thumbnail cache: {error}"))?;
    let cache_for_cleanup = cache_dir.join("thumbnails");
    std::thread::spawn(move || thumbs::cleanup_cache(&cache_for_cleanup, 256 * 1024 * 1024));
    config.cache_dir = Some(cache_dir.display().to_string());

    let auth = Arc::new(match auth_snapshot {
        Some(snapshot) => auth::Auth::with_snapshot(config.pin.clone(), snapshot),
        None => auth::Auth::new(config.pin.clone()),
    });
    let paths = Arc::new(sandbox);
    let state = AppState {
        config: Arc::new(config.clone()),
        paths: paths.clone(),
        auth: auth.clone(),
        thumbs: thumbnails,
        scanner,
        cache_dir,
        thumb_sem: thumbs::new_semaphore(),
        thumb_writes: Arc::new(AtomicU64::new(0)),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("troph")
        .build()
        .map_err(|error| format!("cannot create runtime: {error}"))?;

    let requested_port = if config.port == 0 {
        DEFAULT_PORT
    } else {
        config.port
    };
    let listener = runtime
        .block_on(async { TcpListener::bind((Ipv4Addr::UNSPECIFIED, requested_port)).await })
        .map_err(|error| format!("cannot bind port {requested_port}: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("cannot read local address: {error}"))?
        .port();
    config.port = port;

    let ips = local_ipv4s();
    let urls: Vec<String> = ips.iter().map(|ip| format!("http://{ip}:{port}")).collect();
    let primary_ip = ips.first().copied().unwrap_or(Ipv4Addr::LOCALHOST);

    let mdns = match register_mdns(&hostname, &config.device_name, primary_ip, port) {
        Ok(registration) => Some(registration),
        Err(error) => {
            log::warn!("mDNS registration failed: {error}");
            None
        }
    };

    let app = build_router(state);
    let shutdown = CancellationToken::new();
    let shutdown_token = shutdown.clone();
    runtime.spawn(async move {
        let result = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move { shutdown_token.cancelled().await })
        .await;
        if let Err(error) = result {
            log::error!("server stopped: {error}");
        }
    });

    Ok(RunningServer {
        runtime: Some(runtime),
        shutdown,
        mdns,
        auth,
        config,
        port,
        urls,
        hostname,
    })
}

fn validate_config(config: &ServerConfig) -> Result<(), String> {
    if config.pin.len() < 4
        || config.pin.len() > 12
        || !config.pin.chars().all(|c| c.is_ascii_digit())
    {
        return Err("PIN must be 4 to 12 digits".to_string());
    }
    Ok(())
}

fn normalize_hostname(input: &str) -> String {
    let mut label = input.trim().trim_end_matches('.').to_string();
    if let Some(stripped) = label.strip_suffix(".local") {
        label = stripped.to_string();
    }
    label = label.to_lowercase();
    let mut sanitized = String::new();
    for character in label.chars() {
        if character.is_ascii_alphanumeric() || character == '-' {
            sanitized.push(character);
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    let sanitized = sanitized.trim_matches('-').to_string();
    let sanitized = if sanitized.is_empty() {
        let mut bytes = [0u8; 3];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        format!("troph-{}", hex::encode(bytes))
    } else {
        sanitized
    };
    format!("{}.local", sanitized)
}

fn register_mdns(
    hostname: &str,
    device_name: &str,
    ip: Ipv4Addr,
    port: u16,
) -> Result<MdnsRegistration, String> {
    let daemon = mdns_sd::ServiceDaemon::new().map_err(|error| error.to_string())?;
    let instance = format!("{}-{}", APP_NAME, hostname.trim_end_matches(".local"));
    let host_dns = format!("{hostname}.");
    let mut properties = HashMap::new();
    properties.insert("path".to_string(), "/".to_string());
    properties.insert("version".to_string(), APP_VERSION.to_string());
    properties.insert("device".to_string(), device_name.to_string());
    let info = mdns_sd::ServiceInfo::new(
        "_http._tcp.local.",
        &instance,
        &host_dns,
        ip.to_string(),
        port,
        properties,
    )
    .map_err(|error| error.to_string())?;
    let fullname = info.get_fullname().to_string();
    daemon
        .register(info)
        .map_err(|error| format!("cannot register mDNS service: {error}"))?;
    Ok(MdnsRegistration { daemon, fullname })
}

fn local_ipv4s() -> Vec<Ipv4Addr> {
    let mut addresses = Vec::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for interface in interfaces {
            let IpAddr::V4(ip) = interface.ip() else {
                continue;
            };
            if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() {
                continue;
            }
            addresses.push((interface_priority(&interface.name), ip));
        }
    }
    addresses.sort();
    addresses.dedup();
    let mut result: Vec<Ipv4Addr> = Vec::new();
    for (_, ip) in addresses {
        if !result.contains(&ip) {
            result.push(ip);
        }
    }
    if result.is_empty() {
        result.push(Ipv4Addr::LOCALHOST);
    }
    result
}

fn interface_priority(name: &str) -> u8 {
    let name = name.to_lowercase();
    if name.starts_with("wlan") || name.starts_with("wifi") || name.starts_with("ap") {
        0
    } else if name.starts_with("eth") || name.starts_with("en") {
        1
    } else if name.starts_with("docker") || name.starts_with("veth") || name.starts_with("br-") {
        4
    } else if name.starts_with("rmnet") || name.starts_with("ccmni") || name.starts_with("tun") {
        5
    } else {
        2
    }
}

fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/me", get(me))
        .route("/api/roots", get(roots))
        .route("/api/list", get(files::list))
        .route("/api/list/roots", get(files::list_roots))
        .route("/api/thumb", get(files::thumb))
        .route("/api/raw", get(files::raw))
        .route("/api/zip", post(zip_route::download))
        .route(
            "/api/upload",
            post(upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/api/mkdir", post(ops::mkdir))
        .route("/api/rename", post(ops::rename))
        .route("/api/delete", post(ops::delete))
        .route("/api/move", post(ops::move_items))
        .route("/api/copy", post(ops::copy_items))
        .route("/api/logout", post(logout))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/login", post(login))
        .route("/", get(root_handler))
        .merge(protected)
        .fallback(static_handler)
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    response
}

async fn require_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if state.auth.is_authenticated(request.headers()) {
        next.run(request).await
    } else {
        ApiError::unauthorized("login required").into_response()
    }
}

#[derive(Serialize)]
struct Health {
    ok: bool,
    name: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        name: APP_NAME,
        version: APP_VERSION,
    })
}

#[derive(Serialize)]
struct MeResponse {
    csrf: String,
    version: &'static str,
    hostname: String,
    device_name: String,
    roots: Vec<Root>,
}

async fn me(State(state): State<AppState>, _headers: HeaderMap) -> Json<MeResponse> {
    Json(MeResponse {
        csrf: state.auth.csrf_token().to_string(),
        version: APP_VERSION,
        hostname: state.config.hostname.clone(),
        device_name: state.config.device_name.clone(),
        roots: state.config.roots.clone(),
    })
}

#[derive(Serialize)]
struct RootsResponse {
    roots: Vec<Root>,
}

async fn roots(State(state): State<AppState>) -> Json<RootsResponse> {
    Json(RootsResponse {
        roots: state.config.roots.clone(),
    })
}

#[derive(Deserialize)]
struct LoginRequest {
    pin: String,
}

async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let ip = peer.ip();
    if state.auth.rate_limited(ip) {
        return Err(ApiError::too_many_requests(
            "too many attempts, wait a minute",
        ));
    }
    if !state.auth.pin_matches(&request.pin) {
        state.auth.record_attempt(ip);
        return Err(ApiError::unauthorized("incorrect PIN"));
    }
    state.auth.clear_attempts(ip);
    let token = state.auth.create_session();
    let mut response = Json(json!({
        "ok": true,
        "csrf": state.auth.csrf_token(),
        "version": APP_VERSION,
    }))
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(&token))
            .map_err(|_| ApiError::internal("cannot build session cookie"))?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    state.auth.remove_session(&headers);
    let mut response = Json(json!({ "ok": true })).into_response();
    if let Ok(value) = HeaderValue::from_str(&clear_cookie()) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

#[derive(Default, Deserialize)]
struct RootQuery {
    pin: Option<String>,
    #[allow(dead_code)]
    error: Option<String>,
}

async fn root_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(query): Query<RootQuery>,
) -> Response {
    if let Some(pin) = query.pin {
        let ip = peer.ip();
        if state.auth.rate_limited(ip) {
            return Redirect::to("/?error=rate").into_response();
        }
        if state.auth.pin_matches(&pin) {
            state.auth.clear_attempts(ip);
            let token = state.auth.create_session();
            let mut response = Redirect::to("/").into_response();
            if let Ok(value) = HeaderValue::from_str(&session_cookie(&token)) {
                response.headers_mut().insert(header::SET_COOKIE, value);
            }
            return response;
        }
        state.auth.record_attempt(ip);
        return Redirect::to("/?error=pin").into_response();
    }
    serve_index()
}

#[derive(Deserialize)]
struct UploadQuery {
    dir: Option<String>,
    name: Option<String>,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Serialize)]
struct UploadResponse {
    ok: bool,
    path: String,
    name: String,
    size: u64,
}

async fn upload(
    State(state): State<AppState>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<UploadResponse>, ApiError> {
    if !state.auth.check_csrf_header(&headers) {
        return Err(ApiError::forbidden("invalid CSRF token"));
    }
    let dir = query
        .dir
        .ok_or_else(|| ApiError::bad_request("missing destination folder"))?;
    let name = query
        .name
        .ok_or_else(|| ApiError::bad_request("missing file name"))?;
    let overwrite = query.overwrite;
    let paths = state.paths.clone();
    let target = tokio::task::spawn_blocking(move || {
        let target = paths.resolve_new(&dir, &name)?;
        if !overwrite {
            Ok::<PathBuf, ApiError>(crate::paths::unique_target(&target))
        } else {
            Ok(target)
        }
    })
    .await
    .map_err(|error| ApiError::internal(format!("upload preparation failed: {error}")))??;

    let parent = target
        .parent()
        .ok_or_else(|| ApiError::bad_request("target has no parent"))?
        .to_path_buf();
    let temp = parent.join(format!(".troph-upload-{}", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .await
        .map_err(|error| ApiError::io("cannot create temporary upload", error))?;

    let mut stream = request.into_body().into_data_stream();
    let mut total = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(ApiError::bad_request(format!(
                    "upload interrupted: {error}"
                )));
            }
        };
        total += chunk.len() as u64;
        if total > MAX_UPLOAD_BYTES {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(ApiError::payload_too_large("upload is too large"));
        }
        if let Err(error) = file.write_all(&chunk).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(ApiError::io("cannot write upload", error));
        }
    }
    if let Err(error) = file.flush().await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(ApiError::io("cannot flush upload", error));
    }
    drop(file);

    if let Err(error) = tokio::fs::rename(&temp, &target).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(ApiError::io("cannot finalize upload", error));
    }
    state.scanner.scan(&target);
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "upload".to_string());
    Ok(Json(UploadResponse {
        ok: true,
        path: target.to_string_lossy().to_string(),
        name,
        size: total,
    }))
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.contains("..") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.is_empty() {
        return serve_index();
    }
    if let Some(content) = WebAssets::get(path) {
        return asset_response(path, content.data.to_vec());
    }
    let has_extension = Path::new(path)
        .extension()
        .map(|extension| !extension.is_empty())
        .unwrap_or(false);
    if has_extension || path.starts_with("assets/") {
        StatusCode::NOT_FOUND.into_response()
    } else {
        serve_index()
    }
}

fn serve_index() -> Response {
    match WebAssets::get("index.html") {
        Some(content) => asset_response("index.html", content.data.to_vec()),
        None => (StatusCode::SERVICE_UNAVAILABLE, "Troph web UI is not built").into_response(),
    }
}

fn asset_response(path: &str, data: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    let mut response = Response::new(Body::from(data));
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&mime) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    if path.starts_with("assets/") {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    } else {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_roots_are_allowed() {
        let config = ServerConfig::new("123456", Vec::new());
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn pin_must_be_numeric() {
        let config = ServerConfig::new("12ab56", Vec::new());
        assert!(validate_config(&config).is_err());
    }
}
