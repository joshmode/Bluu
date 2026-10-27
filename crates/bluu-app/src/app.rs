use std::path::PathBuf;

use bluu_core::{
    codec::{CodecId, CodecReport},
    diagnostics::{self, DiagnosticFinding},
    model::{CaptureRequest, Snapshot},
    privacy::DiagnosticExport,
    settings::{Settings, ThemePreference},
    AudioBackend,
};
use eframe::egui::{self, Color32, RichText};

use crate::{
    theme,
    ui::{self, NavSection},
    worker::{Worker, WorkerEvent},
};

pub struct BluuApp {
    pub worker: Worker,
    pub snapshot: Option<Snapshot>,
    pub scan_error: Option<String>,
    pub capture_note: Option<(bool, String)>,
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub nav: NavSection,
    pub selected_device: Option<String>,
    pub selected_codec: Option<CodecId>,
    pub demo: bool,
    pub scanning: bool,
    pub capturing: bool,
    pub capture_seconds: u32,
    pub show_about: bool,
    pub show_privacy: bool,
    pub last_saved: Option<String>,
    pub first_scan_sent: bool,
}

impl BluuApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        backend: Box<dyn AudioBackend>,
        settings: Settings,
        settings_path: PathBuf,
        demo: bool,
    ) -> Self {
        theme::configure(&cc.egui_ctx, settings.theme);
        let last_selected_device = settings.last_selected_device.clone();
        Self {
            worker: Worker::spawn(backend),
            snapshot: None,
            scan_error: None,
            capture_note: None,
            capture_seconds: settings.capture_seconds,
            settings,
            settings_path,
            nav: NavSection::Home,
            selected_device: last_selected_device,
            selected_codec: None,
            demo,
            scanning: true,
            capturing: false,
            show_about: false,
            show_privacy: false,
            last_saved: None,
            first_scan_sent: false,
        }
    }

    pub fn current_device(&self) -> Option<&bluu_core::BluetoothAudioDevice> {
        self.snapshot
            .as_ref()?
            .selected_device(self.selected_device.as_deref())
    }

    pub fn reports(&self) -> Vec<CodecReport> {
        let Some(snapshot) = &self.snapshot else {
            return vec![];
        };
        diagnostics::codec_reports(snapshot, self.current_device())
    }

    pub fn findings(&self, reports: &[CodecReport]) -> Vec<DiagnosticFinding> {
        let Some(snapshot) = &self.snapshot else {
            return vec![];
        };
        diagnostics::findings(snapshot, self.current_device(), reports)
    }

    pub fn active_report<'a>(&self, reports: &'a [CodecReport]) -> Option<&'a CodecReport> {
        reports.iter().find(|r| r.state.is_active())
    }

    pub fn scan(&mut self) {
        self.scanning = true;
        self.scan_error = None;
        self.worker.scan();
    }

    pub fn verify_codec(&mut self) {
        if self.capturing {
            return;
        }
        self.capturing = true;
        self.capture_note = Some((
            false,
            format!(
                "Watching A2DP streaming events for {} seconds. Play music now.",
                self.capture_seconds
            ),
        ));
        self.worker.capture(CaptureRequest {
            device_id: self.current_device().map(|d| d.id.clone()),
            duration_seconds: self.capture_seconds,
        });
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, theme: ThemePreference) {
        self.settings.theme = theme;
        theme::configure(ctx, theme);
        self.save_settings();
    }

    pub fn save_settings(&mut self) {
        self.settings.capture_seconds = self.capture_seconds;
        self.settings.last_selected_device = self.selected_device.clone();
        match self.settings.save(&self.settings_path) {
            Ok(()) => self.last_saved = Some("Saved".into()),
            Err(e) => self.last_saved = Some(format!("Could not save settings: {e}")),
        }
    }

    pub fn restore_defaults(&mut self, ctx: &egui::Context) {
        let path = self.settings_path.clone();
        self.settings = Settings::default();
        self.capture_seconds = self.settings.capture_seconds;
        let _ = self.settings.save(&path);
        theme::configure(ctx, ThemePreference::Dark);
        self.last_saved = Some("Defaults restored".into());
    }

    pub fn export_diagnostics(&mut self) -> Option<PathBuf> {
        let snapshot = self.snapshot.clone()?;
        let export = if self.settings.include_device_names_in_export
            || self.settings.include_addresses_in_export
        {
            let options = bluu_core::privacy::PrivacyOptions {
                redact_device_names: !self.settings.include_device_names_in_export,
                redact_addresses: !self.settings.include_addresses_in_export,
                ..Default::default()
            };
            DiagnosticExport::with_options(snapshot, options)
        } else {
            DiagnosticExport::redacted(snapshot)
        };
        let dir = bluu_core::settings::export_dir();
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(format!(
            "bluu-diagnostics-{}.json",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        match export
            .to_pretty_json()
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                self.last_saved = Some(format!("Exported {}", path.display()));
                Some(path)
            }
            Err(e) => {
                self.last_saved = Some(format!("Export failed: {e}"));
                None
            }
        }
    }

    fn poll_worker(&mut self) {
        for event in self.worker.poll() {
            match event {
                WorkerEvent::Scanned(boxed) => match *boxed {
                    Ok(snapshot) => {
                        self.worker.scan_finished();
                        self.scanning = false;
                        let keep = self
                            .selected_device
                            .as_ref()
                            .and_then(|id| snapshot.device(id))
                            .map(|d| d.id.clone());
                        self.selected_device = keep.or_else(|| {
                            snapshot
                                .connected_devices()
                                .next()
                                .or_else(|| snapshot.devices.first())
                                .map(|d| d.id.clone())
                        });
                        if let Some(active) = &snapshot.active_codec {
                            if self.selected_device.is_some()
                                && active.device_id.as_deref().is_none()
                            {
                                {
                                    // The trace event only carries codec IDs. Keep it as a
                                    // low-confidence system observation rather than pretend
                                    // it belongs to a specific device.
                                    self.capture_note = Some((
                                    true,
                                    "Codec observed, but Windows did not expose a device identifier in the trace payload. The result is system-level, not device-verified.".into(),
                                ));
                                }
                            }
                        }
                        self.snapshot = Some(snapshot);
                    }
                    Err(e) => {
                        self.worker.scan_finished();
                        self.scanning = false;
                        self.scan_error = Some(format!("{e}\n{}", e.actionable()));
                    }
                },
                WorkerEvent::Captured(Ok(outcome)) => {
                    self.capturing = false;
                    self.capture_note = Some((
                        outcome.observation.is_some(),
                        if outcome.observation.is_some() {
                            format!(
                                "{} {} A2DP event(s) observed. Codec association remains system-level unless independently verified.",
                                outcome.captured_events, outcome.note
                            )
                        } else {
                            outcome.note
                        },
                    ));
                    if let Some(observation) = outcome.observation {
                        if let Some(snapshot) = &mut self.snapshot {
                            let codec = observation.codec;
                            snapshot.active_codec = Some(observation);
                            self.selected_codec = Some(codec);
                        }
                    }
                }
                WorkerEvent::Captured(Err(e)) => {
                    self.capturing = false;
                    self.capture_note = Some((false, format!("{e}\n{}", e.actionable())));
                }
            }
        }
    }
}

impl eframe::App for BluuApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::sync(ctx);
        if !self.first_scan_sent {
            self.first_scan_sent = true;
            self.scan();
        }
        self.poll_worker();
        if self.scanning || self.capturing {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        let (refresh, export) = ctx.input(|input| {
            let refresh = input.modifiers.command && input.key_pressed(egui::Key::R);
            let export =
                input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::E);
            (refresh, export)
        });
        if refresh {
            self.scan();
        }
        if export {
            if let Some(path) = self.export_diagnostics() {
                self.show_privacy = true;
                tracing::info!(path = %path.display(), "diagnostics exported");
            }
        }
        egui::TopBottomPanel::top("topbar")
            .frame(
                egui::Frame::new()
                    .fill(theme::bg())
                    .inner_margin(egui::Margin::symmetric(22, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("bluu")
                            .size(23.0)
                            .strong()
                            .color(theme::text()),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Bluetooth audio, with evidence")
                            .size(11.5)
                            .color(theme::muted()),
                    );
                    ui.add_space(24.0);
                    for section in NavSection::ALL {
                        let selected = self.nav == section;
                        let color = if selected {
                            theme::text()
                        } else {
                            theme::muted()
                        };
                        let button = egui::Button::new(RichText::new(section.label()).color(color))
                            .fill(if selected {
                                theme::panel_raised()
                            } else {
                                Color32::TRANSPARENT
                            })
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8);
                        if ui.add(button).clicked() {
                            self.nav = section;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.demo {
                            ui.label(
                                RichText::new("DEMO")
                                    .size(10.5)
                                    .strong()
                                    .color(theme::warn()),
                            );
                        }
                        let refresh = ui
                            .add_enabled(
                                !self.scanning,
                                egui::Button::new(if self.scanning {
                                    "Refreshing…"
                                } else {
                                    "Refresh"
                                })
                                .fill(theme::panel_raised()),
                            )
                            .clicked();
                        if refresh {
                            self.scan();
                        }
                    });
                });
            });

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(250.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::bg())
                    .inner_margin(egui::Margin::symmetric(18, 10))
                    .stroke(egui::Stroke::new(1.0_f32, theme::border())),
            )
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("DEVICE")
                        .size(10.0)
                        .strong()
                        .color(theme::faint()),
                );
                ui.add_space(5.0);
                let devices = self
                    .snapshot
                    .as_ref()
                    .map(|s| s.devices.clone())
                    .unwrap_or_default();
                if devices.is_empty() {
                    ui.label(
                        RichText::new(if self.scanning {
                            "Discovering paired devices…"
                        } else {
                            "No paired Bluetooth devices"
                        })
                        .color(theme::muted()),
                    );
                }
                for device in devices {
                    let selected = self.selected_device.as_deref() == Some(device.id.as_str());
                    let response = ui
                        .scope(|ui| {
                            let frame = egui::Frame::new()
                                .fill(if selected {
                                    theme::panel_raised()
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(if selected {
                                    egui::Stroke::new(1.0_f32, theme::border())
                                } else {
                                    egui::Stroke::NONE
                                })
                                .corner_radius(10)
                                .inner_margin(egui::Margin::symmetric(10, 9));
                            frame
                                .show(ui, |ui| {
                                    ui.set_width(200.0);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(if device.connected {
                                                "●"
                                            } else {
                                                "○"
                                            })
                                            .color(if device.connected {
                                                theme::good()
                                            } else {
                                                theme::faint()
                                            })
                                            .size(11.0),
                                        );
                                        ui.label(
                                            RichText::new(&device.name)
                                                .size(13.0)
                                                .color(if selected {
                                                    theme::text()
                                                } else {
                                                    theme::muted()
                                                })
                                                .strong(),
                                        );
                                    });
                                    ui.label(
                                        RichText::new(if device.connected {
                                            "Connected"
                                        } else {
                                            "Paired · disconnected"
                                        })
                                        .size(10.5)
                                        .color(theme::faint()),
                                    );
                                })
                                .response
                        })
                        .response;
                    if response.interact(egui::Sense::click()).clicked() {
                        self.selected_device = Some(device.id.clone());
                        self.save_settings();
                    }
                }

                ui.add_space(18.0);
                ui.label(
                    RichText::new("BACKEND")
                        .size(10.0)
                        .strong()
                        .color(theme::faint()),
                );
                ui.add_space(5.0);
                if let Some(snapshot) = &self.snapshot {
                    ui.label(
                        RichText::new(&snapshot.backend.name)
                            .size(12.5)
                            .color(theme::text()),
                    );
                    ui.label(
                        RichText::new(if snapshot.backend.read_only {
                            "Read-only inspection"
                        } else {
                            "Configuration capable"
                        })
                        .size(10.5)
                        .color(theme::muted()),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "{} · build {}",
                            snapshot.system.os_name, snapshot.system.os_build
                        ))
                        .size(10.5)
                        .color(theme::faint()),
                    );
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("No telemetry · local only")
                            .size(10.0)
                            .color(theme::faint()),
                    );
                    ui.add_space(4.0);
                    if ui
                        .add(
                            egui::Button::new(RichText::new("About & limitations").size(11.0))
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.show_about = true;
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::bg())
                    .inner_margin(egui::Margin::symmetric(28, 16)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_max_width(1060.0);
                        if let Some(error) = &self.scan_error {
                            ui::error_banner(ui, "Scan failed", error);
                            ui.add_space(12.0);
                        }
                        match self.nav {
                            NavSection::Home => ui::home(self, ui),
                            NavSection::Devices => ui::devices(self, ui),
                            NavSection::Codecs => ui::codecs(self, ui),
                            NavSection::Diagnostics => ui::diagnostics(self, ui),
                            NavSection::Settings => ui::settings(self, ctx, ui),
                        }
                    });
            });

        self.about_window(ctx);
        self.privacy_window(ctx);
        self.remember_window(ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.settings.save(&self.settings_path);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_settings();
    }
}

impl BluuApp {
    fn remember_window(&mut self, ctx: &egui::Context) {
        let rect = ctx.input(|i| i.viewport().inner_rect);
        if let Some(rect) = rect {
            if rect.width() > 0.0 && rect.height() > 0.0 {
                self.settings.window.width = rect.width();
                self.settings.window.height = rect.height();
            }
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        egui::Window::new("About Bluu")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.heading("Bluu");
                ui.label(
                    RichText::new(format!(
                        "Version {} · {}-licensed open source",
                        bluu_core::VERSION,
                        "MIT OR Apache-2.0"
                    ))
                    .color(theme::muted()),
                );
                ui.add_space(10.0);
                ui.label(
                    "Bluu inspects Windows Bluetooth audio devices, cached A2DP/HFP profiles, Core Audio endpoints, and (with one-time elevation) the codec Windows actually negotiated.",
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("What Bluu does not do")
                        .strong()
                        .color(theme::warn()),
                );
                ui.label(
                    "It does not install drivers, edit the registry, disable Secure Boot or driver signature enforcement, replace the Bluetooth stack, or register codecs. Windows exposes no supported API to choose an A2DP codec, so Bluu does not offer a fake preference switch.",
                );
                ui.add_space(8.0);
                ui.label(
                    "A2DP tracing uses Microsoft's ETW provider and an undocumented event payload. Codec observation is labelled accordingly; it is not a guarantee of device-specific negotiation.",
                );
            });
        self.show_about = open;
    }

    fn privacy_window(&mut self, ctx: &egui::Context) {
        if !self.show_privacy {
            return;
        }
        let mut open = true;
        egui::Window::new("Diagnostic export privacy")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label("Exports are local JSON files. They never upload automatically.");
                ui.add_space(8.0);
                ui.label("Included:");
                ui.label("• Windows version/build, backend capabilities, adapter class, connection/profile observations, endpoint formats, codec inventory and diagnostics.");
                ui.add_space(6.0);
                ui.label("Redacted by default:");
                ui.label("• Bluetooth addresses, device names, endpoint/container IDs, and filesystem paths.");
                ui.add_space(6.0);
                ui.label("Not included: usernames, computer name, audio samples, packet captures, credentials, or network telemetry.");
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Review any export before sharing it. Redaction is a safety default, not a legal guarantee.")
                        .color(theme::muted())
                        .size(12.0),
                );
            });
        self.show_privacy = open;
    }
}
