use std::sync::Arc;
use std::time::Duration;

use pocketfiles_core::{ImageThumbnails, NoopScanner, Root, RootKind, ServerConfig, DEFAULT_PORT};

fn main() {
    env_logger::init();

    let mut port = DEFAULT_PORT;
    let mut pin = "123456".to_string();
    let mut roots: Vec<Root> = Vec::new();
    let mut cache_dir = std::env::temp_dir().join("troph-cache");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" if i + 1 < args.len() => {
                port = args[i + 1].parse().expect("invalid port");
                i += 2;
            }
            "--pin" if i + 1 < args.len() => {
                pin = args[i + 1].clone();
                i += 2;
            }
            "--root" if i + 1 < args.len() => {
                let path = std::fs::canonicalize(&args[i + 1]).expect("invalid root");
                let kind = if path.is_file() {
                    RootKind::File
                } else {
                    RootKind::Folder
                };
                roots.push(Root {
                    label: path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Root".to_string()),
                    path: path.display().to_string(),
                    kind,
                });
                i += 2;
            }
            "--cache" if i + 1 < args.len() => {
                cache_dir = std::path::PathBuf::from(&args[i + 1]);
                i += 2;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    if roots.is_empty() {
        roots.push(Root {
            label: "Home".to_string(),
            path: std::env::current_dir().unwrap().display().to_string(),
            kind: RootKind::Folder,
        });
    }

    let config = ServerConfig {
        port,
        pin: pin.clone(),
        roots,
        cache_dir: Some(cache_dir.display().to_string()),
        device_name: "Troph Dev".to_string(),
        hostname: "troph-dev.local".to_string(),
    };
    match pocketfiles_core::start_server_with(
        config,
        Arc::new(ImageThumbnails),
        Arc::new(NoopScanner),
    ) {
        Ok(server) => {
            println!("Troph dev server running:");
            println!(
                "{}",
                serde_json::to_string_pretty(&server.status()).unwrap()
            );
            println!("PIN: {pin}");
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        }
        Err(error) => {
            eprintln!("failed to start: {error}");
            std::process::exit(1);
        }
    }
}
