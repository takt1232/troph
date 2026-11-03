#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod server;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};

use server::{DesktopSnapshot, DesktopState};

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn emit_error(app: &tauri::AppHandle, error: impl Into<String>) {
    let _ = app.emit("server-error", error.into());
}

fn toggle_from_tray(app: &tauri::AppHandle) {
    let state = app.state::<DesktopState>();
    let snapshot = state.snapshot();
    let result = if snapshot.running {
        server::stop(app, &state)
    } else {
        server::start(app, &state)
    };
    if let Err(error) = result {
        log::warn!("tray toggle failed: {error}");
        emit_error(app, error);
    }
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Troph", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Start / stop server", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open file manager", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &toggle, &open, &separator, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(tauri::include_image!("./icons/32x32.png"))
        .tooltip("Troph")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "toggle" => toggle_from_tray(app),
            "open" => {
                let state = app.state::<DesktopState>();
                if let Err(error) = server::open_file_manager(app, &state) {
                    emit_error(app, error);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

#[tauri::command]
fn get_state(state: tauri::State<'_, DesktopState>) -> DesktopSnapshot {
    state.snapshot()
}

#[tauri::command]
fn start_server(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopSnapshot, String> {
    server::start(&app, &state)
}

#[tauri::command]
fn stop_server(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopSnapshot, String> {
    server::stop(&app, &state)
}

#[tauri::command]
fn add_root(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    path: String,
    label: Option<String>,
) -> Result<DesktopSnapshot, String> {
    server::add_root(&app, &state, path, label)
}

#[tauri::command]
fn remove_root(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    path: String,
) -> Result<DesktopSnapshot, String> {
    server::remove_root(&app, &state, path)
}

#[tauri::command]
fn update_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    settings: server::SettingsInput,
) -> Result<DesktopSnapshot, String> {
    server::update_settings(&app, &state, settings)
}

#[tauri::command]
fn open_file_manager(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    server::open_file_manager(&app, &state)
}

#[tauri::command]
fn cancel_transfer(state: tauri::State<'_, DesktopState>, id: String) -> Result<(), String> {
    server::cancel_transfer(&state, id)
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let builder = tauri::Builder::default();

    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        show_main_window(app);
    }));

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let config = config::load(app.handle());
            app.manage(DesktopState::new(config));
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            start_server,
            stop_server,
            add_root,
            remove_root,
            update_settings,
            open_file_manager,
            cancel_transfer
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<DesktopState>();
                if state.close_to_tray() {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    app.exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Troph");
}
