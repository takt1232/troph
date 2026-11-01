use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Arc, Mutex, Once};

use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jstring, JNI_FALSE, JNI_TRUE};
use jni::{JNIEnv, JavaVM};
use pocketfiles_core::{
    MediaScanner, RunningServer, ServerConfig, ServerStatus, ThumbnailData, ThumbnailProvider,
};
use serde_json::json;

static SERVER: Mutex<Option<RunningServer>> = Mutex::new(None);
static LOGGER: Once = Once::new();

fn init_logging() {
    LOGGER.call_once(|| {
        #[cfg(target_os = "android")]
        {
            android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(log::LevelFilter::Info)
                    .with_tag("Troph"),
            );
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = env_logger::builder().try_init();
        }
    });
}

fn make_jstring(env: &mut JNIEnv, value: &str) -> jstring {
    match env.new_string(value) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn status_json(server: &RunningServer) -> String {
    let status: ServerStatus = server.status();
    serde_json::to_string(&status).unwrap_or_else(|error| {
        json!({"ok": false, "error": format!("failed to serialize status: {error}")}).to_string()
    })
}

fn global_class(env: &mut JNIEnv, name: &str) -> Result<GlobalRef, String> {
    let class = env
        .find_class(name)
        .map_err(|error| format!("cannot find class {name}: {error}"))?;
    env.new_global_ref(&class)
        .map_err(|error| format!("cannot create global class reference: {error}"))
}

struct AndroidThumbnails {
    vm: Arc<JavaVM>,
    context: GlobalRef,
    class: GlobalRef,
}

impl ThumbnailProvider for AndroidThumbnails {
    fn thumbnail(&self, path: &Path, size: u32) -> Result<Option<ThumbnailData>, String> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|error| format!("cannot attach JNI thread: {error}"))?;
        let path_string = env
            .new_string(path.to_string_lossy().to_string())
            .map_err(|error| format!("cannot create path string: {error}"))?;
        let context: &JObject = &self.context;
        let is_video = is_video_path(path);
        let result = env
            .call_static_method(
                &self.class,
                "thumbnail",
                "(Landroid/content/Context;Ljava/lang/String;IZ)[B",
                &[
                    JValue::Object(context),
                    JValue::Object(&path_string),
                    JValue::Int(size as i32),
                    JValue::Bool(if is_video { 1 } else { 0 }),
                ],
            )
            .map_err(|error| format!("thumbnail call failed: {error}"))?;
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
            return Err("thumbnail bridge raised an exception".to_string());
        }
        let object = result
            .l()
            .map_err(|error| format!("thumbnail returned an invalid value: {error}"))?;
        if object.is_null() {
            return Ok(None);
        }
        let array = JByteArray::from(object);
        let bytes = env
            .convert_byte_array(&array)
            .map_err(|error| format!("cannot read thumbnail bytes: {error}"))?;
        Ok(Some(ThumbnailData {
            bytes,
            content_type: "image/webp".to_string(),
        }))
    }
}

struct AndroidScanner {
    vm: Arc<JavaVM>,
    context: GlobalRef,
    class: GlobalRef,
}

impl MediaScanner for AndroidScanner {
    fn scan(&self, path: &Path) {
        let result = (|| -> Result<(), String> {
            let mut env = self
                .vm
                .attach_current_thread()
                .map_err(|error| format!("cannot attach JNI thread: {error}"))?;
            let path_string = env
                .new_string(path.to_string_lossy().to_string())
                .map_err(|error| format!("cannot create path string: {error}"))?;
            let context: &JObject = &self.context;
            env.call_static_method(
                &self.class,
                "scan",
                "(Landroid/content/Context;Ljava/lang/String;)V",
                &[JValue::Object(context), JValue::Object(&path_string)],
            )
            .map_err(|error| format!("media scan call failed: {error}"))?;
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
                return Err("media scanner bridge raised an exception".to_string());
            }
            Ok(())
        })();
        if let Err(error) = result {
            log::warn!("media scan failed for {}: {error}", path.display());
        }
    }
}

fn is_video_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .map(|extension| extension.to_string_lossy().to_lowercase())
            .as_deref(),
        Some("mp4")
            | Some("m4v")
            | Some("mkv")
            | Some("webm")
            | Some("mov")
            | Some("avi")
            | Some("3gp")
            | Some("ts")
            | Some("m2ts")
    )
}

fn make_providers(
    env: &mut JNIEnv,
    context: &JObject,
) -> Result<(Arc<AndroidThumbnails>, Arc<AndroidScanner>), String> {
    let vm = Arc::new(
        env.get_java_vm()
            .map_err(|error| format!("cannot get JavaVM: {error}"))?,
    );
    let context_global = env
        .new_global_ref(context)
        .map_err(|error| format!("cannot retain Android context: {error}"))?;
    let thumbnail_class = global_class(env, "com/sixdaggers/pocketfiles/ThumbnailBridge")?;
    let scanner_class = global_class(env, "com/sixdaggers/pocketfiles/MediaScannerBridge")?;

    let thumbnails = Arc::new(AndroidThumbnails {
        vm: vm.clone(),
        context: context_global.clone(),
        class: thumbnail_class,
    });
    let scanner = Arc::new(AndroidScanner {
        vm,
        context: context_global,
        class: scanner_class,
    });
    Ok((thumbnails, scanner))
}

fn parse_config(env: &mut JNIEnv, config_json: &JString) -> Result<ServerConfig, String> {
    let config_text: String = env
        .get_string(config_json)
        .map_err(|error| format!("bad config: {error}"))?
        .into();
    serde_json::from_str(&config_text).map_err(|error| format!("invalid config JSON: {error}"))
}

fn do_start(env: &mut JNIEnv, context: &JObject, config_json: &JString) -> Result<String, String> {
    let config = parse_config(env, config_json)?;
    let (thumbnails, scanner) = make_providers(env, context)?;

    let mut guard = SERVER.lock().unwrap_or_else(|poison| poison.into_inner());
    guard.take();
    let server = pocketfiles_core::start_server_with(config, thumbnails, scanner)?;
    let status = status_json(&server);
    *guard = Some(server);
    Ok(status)
}

fn do_restart(
    env: &mut JNIEnv,
    context: &JObject,
    config_json: &JString,
) -> Result<String, String> {
    let config = parse_config(env, config_json)?;
    let (thumbnails, scanner) = make_providers(env, context)?;

    let mut guard = SERVER.lock().unwrap_or_else(|poison| poison.into_inner());
    let snapshot = guard.as_ref().map(|server| server.auth_snapshot());
    guard.take();
    let server =
        pocketfiles_core::start_server_with_snapshot(config, thumbnails, scanner, snapshot)?;
    let status = status_json(&server);
    *guard = Some(server);
    Ok(status)
}

#[no_mangle]
pub extern "system" fn Java_com_sixdaggers_pocketfiles_ServerBridge_startServer(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
    config_json: JString,
) -> jstring {
    init_logging();
    let result = catch_unwind(AssertUnwindSafe(|| {
        match do_start(&mut env, &context, &config_json) {
            Ok(status) => make_jstring(&mut env, &status),
            Err(error) => {
                log::error!("startServer failed: {error}");
                make_jstring(&mut env, &json!({"ok": false, "error": error}).to_string())
            }
        }
    }));

    match result {
        Ok(pointer) => pointer,
        Err(_) => make_jstring(
            &mut env,
            &json!({"ok": false, "error": "native panic in startServer"}).to_string(),
        ),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sixdaggers_pocketfiles_ServerBridge_restartServer(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
    config_json: JString,
) -> jstring {
    init_logging();
    let result = catch_unwind(AssertUnwindSafe(|| {
        match do_restart(&mut env, &context, &config_json) {
            Ok(status) => make_jstring(&mut env, &status),
            Err(error) => {
                log::error!("restartServer failed: {error}");
                make_jstring(&mut env, &json!({"ok": false, "error": error}).to_string())
            }
        }
    }));

    match result {
        Ok(pointer) => pointer,
        Err(_) => make_jstring(
            &mut env,
            &json!({"ok": false, "error": "native panic in restartServer"}).to_string(),
        ),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sixdaggers_pocketfiles_ServerBridge_stopServer(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    init_logging();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut guard = SERVER.lock().unwrap_or_else(|poison| poison.into_inner());
        guard.take();
        make_jstring(&mut env, &json!({"ok": true}).to_string())
    }));
    match result {
        Ok(pointer) => pointer,
        Err(_) => make_jstring(
            &mut env,
            &json!({"ok": false, "error": "native panic in stopServer"}).to_string(),
        ),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sixdaggers_pocketfiles_ServerBridge_isRunning(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if SERVER.lock().map(|guard| guard.is_some()).unwrap_or(false) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}
