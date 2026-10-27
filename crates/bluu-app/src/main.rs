#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod provider_defaults;
mod theme;
mod ui;
mod worker;

#[cfg(test)]
mod provider_tests;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Internal helper mode is only meaningful on Windows. The public app intentionally
    // rejects unknown commands rather than passing arbitrary arguments to a helper.
    if args.iter().any(|a| a == bluu_platform::launch::HELPER_ARG) {
        eprintln!("The ETW helper is a separate executable: bluu-etw-helper.");
        std::process::exit(2);
    }

    let demo = args.iter().any(|a| a == "--demo");
    let snapshot_json = args.iter().any(|a| a == "--snapshot-json");
    let provider_smoke = args.iter().any(|a| a == "--provider-smoke");
    let aac_state = args.iter().any(|a| a == "--aac-state");
    if snapshot_json || provider_smoke || aac_state {
        run_headless(snapshot_json, provider_smoke, aac_state, demo);
        return;
    }
    let settings_path = bluu_core::settings::settings_path();
    let settings =
        bluu_core::settings::Settings::load_or_default(&settings_path).unwrap_or_else(|e| {
            tracing::warn!(%e, "settings could not be loaded; using defaults");
            bluu_core::settings::Settings::default()
        });
    init_logging(settings.verbose_logging);
    tracing::info!(version = bluu_core::VERSION, "starting Bluu");
    let backend = bluu_platform::make_backend(demo);

    let icon = load_icon();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Bluu")
            .with_inner_size([settings.window.width, settings.window.height])
            .with_min_inner_size([820.0, 560.0])
            .with_app_id("io.bluu.Bluu")
            .with_icon(icon.unwrap_or_default()),
        centered: true,
        ..Default::default()
    };
    let settings_path: PathBuf = settings_path;
    let result = eframe::run_native(
        "Bluu",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::BluuApp::new(
                cc,
                backend,
                settings,
                settings_path,
                demo,
            )))
        }),
    );
    if let Err(e) = result {
        tracing::error!(%e, "window failed");
        eprintln!("Bluu could not start its window: {e}");
    }
}

fn run_headless(snapshot_json: bool, provider_smoke: bool, aac_state: bool, demo: bool) {
    let mut backend = bluu_platform::make_backend(demo);
    let snapshot = match backend.scan() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("scan failed: {error}");
            std::process::exit(1);
        }
    };
    if snapshot_json {
        match serde_json::to_string_pretty(&snapshot) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("snapshot JSON failed: {error}");
                std::process::exit(1);
            }
        }
    }
    if aac_state {
        match serde_json::to_string_pretty(&snapshot.inbox_codec) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("inbox codec state JSON failed: {error}");
                std::process::exit(1);
            }
        }
    }
    if provider_smoke && demo {
        let provider = snapshot.providers.iter().find(|p| p.is_usable()).cloned();
        let Some(provider) = provider else {
            eprintln!("no usable demo provider found");
            std::process::exit(2);
        };
        let device = snapshot.devices.iter().find(|d| d.connected).map(|d| {
            bluu_core::provider::ProviderDevice {
                id: d.id.clone(),
                name: d.name.clone(),
                address: d.address.clone(),
                connected: d.connected,
            }
        });
        let request = bluu_core::model::ProviderOperationRequest {
            provider_id: provider.manifest.id.clone(),
            device_id: device.as_ref().map(|d| d.id.clone()),
            operation: bluu_core::ProviderAction::Describe,
            preference: None,
            rollback_token: None,
        };
        match backend.provider_operation(&request) {
            Ok(response) => println!(
                "provider={} ok={} message={}",
                provider.manifest.id, response.accepted, response.message
            ),
            Err(error) => {
                eprintln!("provider smoke failed: {error}");
                std::process::exit(1);
            }
        }
    } else if provider_smoke {
        let providers = bluu_platform::providers::discover();
        let provider = providers.iter().find(|p| p.is_usable()).cloned();
        let Some(provider) = provider else {
            eprintln!("no usable provider found");
            std::process::exit(2);
        };
        match bluu_platform::providers::smoke_provider(&provider) {
            Ok(summary) => println!("{summary}"),
            Err(error) => {
                eprintln!("provider smoke failed: {error}");
                std::process::exit(1);
            }
        }
    }
}

fn init_logging(verbose: bool) {
    let dir = bluu_core::settings::log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("bluu.log"))
        .ok();
    let default_filter = if verbose {
        "bluu=debug,warn"
    } else {
        "bluu=info,warn"
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false);
    if let Some(file) = file {
        let _ = builder.with_writer(file).try_init();
    } else {
        let _ = builder.try_init();
    }
}

/// Decode the repository icon into raw RGBA for eframe. If the PNG is missing or
/// malformed, the app still starts with its default icon rather than failing.
fn load_icon() -> Option<eframe::egui::IconData> {
    let bytes = include_bytes!("../../../assets/bluu.png");
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(eframe::egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
