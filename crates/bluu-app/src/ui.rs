use bluu_core::{
    codec::{CodecReport, CodecState, Confidence},
    diagnostics::DiagnosticFinding,
    feasibility::*,
    model::{AudioEndpoint, BluetoothAudioDevice, EndpointState},
    settings::ThemePreference,
    ProviderAction,
};
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Stroke, Ui, Vec2};

use crate::{app::BluuApp, theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavSection {
    Home,
    Devices,
    Codecs,
    Providers,
    Diagnostics,
    Settings,
}

impl NavSection {
    pub const ALL: [Self; 6] = [
        Self::Home,
        Self::Devices,
        Self::Codecs,
        Self::Providers,
        Self::Diagnostics,
        Self::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Devices => "Devices",
            Self::Codecs => "Codecs",
            Self::Providers => "Providers",
            Self::Diagnostics => "Diagnostics",
            Self::Settings => "Settings",
        }
    }
}

pub fn page_heading(ui: &mut Ui, eyebrow: &str, title: &str, subtitle: &str) {
    ui.label(
        RichText::new(eyebrow.to_uppercase())
            .size(10.0)
            .strong()
            .color(theme::accent()),
    );
    ui.add_space(3.0);
    ui.label(
        RichText::new(title)
            .size(28.0)
            .strong()
            .color(theme::text()),
    );
    ui.label(RichText::new(subtitle).size(13.0).color(theme::muted()));
    ui.add_space(16.0);
}

fn pill(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            24,
        ))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90),
        ))
        .corner_radius(20)
        .inner_margin(egui::Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(10.5).strong().color(color));
        });
}

fn dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, color);
}

fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let center = rect.center();
    ui.painter()
        .circle_filled(center, size * 0.40, theme::accent_soft());
    ui.painter()
        .circle_stroke(center, size * 0.37, Stroke::new(1.2_f32, theme::accent()));
    let r = size * 0.12;
    ui.painter()
        .circle_filled(center + Vec2::new(-size * 0.13, 0.0), r, theme::accent());
    ui.painter()
        .circle_filled(center + Vec2::new(size * 0.13, 0.0), r, theme::good());
    ui.painter().line_segment(
        [
            center + Vec2::new(-size * 0.07, 0.0),
            center + Vec2::new(size * 0.07, 0.0),
        ],
        Stroke::new(1.2_f32, theme::text()),
    );
}

fn section_heading(ui: &mut Ui, title: &str, trailing: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .size(15.0)
                .strong()
                .color(theme::text()),
        );
        if let Some(trailing) = trailing {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(trailing).size(10.5).color(theme::faint()));
            });
        }
    });
    ui.add_space(8.0);
}

fn inbox_codec_card(app: &mut BluuApp, ui: &mut Ui) {
    let Some(snapshot) = app.snapshot.as_ref() else {
        return;
    };
    let toggle = snapshot.inbox_codec.clone();
    if !toggle.supported || !snapshot.system.is_windows_11() {
        return;
    }
    theme::subtle_panel().show(ui, |ui| {
        section_heading(
            ui,
            "Windows in-box AAC / aptX preference",
            Some("Undocumented · whole PC"),
        );
        ui.label(
            RichText::new(&toggle.note)
                .size(10.8)
                .color(theme::muted()),
        );
        ui.add_space(8.0);
        key_value(
            ui,
            "AAC preference",
            match toggle.aac_enabled {
                Some(true) => "Enabled",
                Some(false) => "Disabled",
                None => "Absent (Windows default)",
            },
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "This BthA2dp service value is build-sensitive and is not a codec-registration API. Disabling AAC can let aptX Classic win on aptX-capable hardware. It cannot enable LDAC, aptX HD, or aptX Adaptive. A change applies to every Bluetooth audio device on this PC and requires reconnection.",
            )
            .size(10.8)
            .color(theme::warn()),
        );
        ui.add_space(6.0);
        if !app.demo {
            ui.checkbox(
                &mut app.inbox_codec_consented,
                "I understand this writes an undocumented HKLM service parameter and affects the whole PC.",
            );
        } else {
            app.inbox_codec_consented = true;
        }
        ui.horizontal_wrapped(|ui| {
            let can_change = !app.inbox_codec_busy && app.inbox_codec_consented;
            if ui
                .add_enabled(
                    !app.inbox_codec_busy,
                    egui::Button::new("Preview change (no write)"),
                )
                .clicked()
            {
                app.inbox_codec_change(false, true);
            }
            if ui
                .add_enabled(
                    can_change,
                    egui::Button::new("Prefer aptX (disable AAC)").fill(theme::accent_soft()),
                )
                .clicked()
            {
                app.inbox_codec_change(false, false);
            }
            if ui
                .add_enabled(can_change, egui::Button::new("Re-enable AAC"))
                .clicked()
            {
                app.inbox_codec_change(true, false);
            }
            if ui
                .add_enabled(
                    can_change && !app.demo,
                    egui::Button::new("Restore previous value"),
                )
                .clicked()
            {
                app.restore_inbox_codec();
            }
        });
        ui.add_space(3.0);
        ui.label(
            RichText::new(format!("Registry scope: {}", toggle.registry_path))
                .size(9.8)
                .color(theme::faint()),
        );
        if let Some((ok, message)) = &app.inbox_codec_note {
            ui.add_space(6.0);
            notice_banner(ui, *ok, message);
        }
    });
    ui.add_space(14.0);
}

fn key_value(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).size(11.5).color(theme::muted()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .size(11.5)
                    .color(theme::text())
                    .strong(),
            );
        });
    });
}

fn render_endpoint(app: &BluuApp, device: Option<&BluetoothAudioDevice>) -> Option<AudioEndpoint> {
    app.snapshot
        .as_ref()?
        .selected_render_endpoint(device)
        .cloned()
}

fn capture_endpoint(app: &BluuApp, device: Option<&BluetoothAudioDevice>) -> Option<AudioEndpoint> {
    app.snapshot
        .as_ref()?
        .selected_capture_endpoint(device)
        .cloned()
}

pub fn home(app: &mut BluuApp, ui: &mut Ui) {
    let snapshot = app.snapshot.clone();
    let device = snapshot
        .as_ref()
        .and_then(|s| s.selected_device(app.selected_device.as_deref()))
        .cloned();
    let reports = app.reports();
    let findings = app.findings(&reports);
    let active = app.active_report(&reports).cloned();
    let render = render_endpoint(app, device.as_ref());
    let capture = capture_endpoint(app, device.as_ref());
    let observed = snapshot.as_ref().and_then(|s| s.active_codec.clone());

    page_heading(
        ui,
        "Overview",
        "A clear view of your wireless audio",
        "Connection state, Windows audio format, and the codec actually observed — without inventing controls Windows does not expose.",
    );

    if let Some(device) = &device {
        theme::hero_panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                logo(ui, 56.0);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&device.name)
                                .size(22.0)
                                .strong()
                                .color(theme::text()),
                        );
                        if device.connected {
                            pill(ui, "CONNECTED", theme::good());
                        } else {
                            pill(ui, "DISCONNECTED", theme::muted());
                        }
                    });
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            device.category.as_deref().unwrap_or("Bluetooth audio"),
                            device.transport.label()
                        ))
                        .size(12.0)
                        .color(theme::muted()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            !app.capturing,
                            egui::Button::new(if app.capturing {
                                "Listening…"
                            } else {
                                "Verify live codec"
                            })
                            .fill(theme::accent_soft())
                            .stroke(Stroke::new(1.0_f32, theme::accent()))
                            .corner_radius(9),
                        )
                        .on_hover_text(
                            "Starts a short elevated ETW trace. It does not change any setting.",
                        )
                        .clicked()
                    {
                        app.verify_codec();
                    }
                });
            });
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(12.0);
            ui.columns(3, |columns| {
                metric(
                    &mut columns[0],
                    "ACTIVE A2DP CODEC",
                    &active
                        .as_ref()
                        .map(|r| r.codec.label().to_string())
                        .or_else(|| observed.as_ref().map(|o| o.codec.label().to_string()))
                        .unwrap_or_else(|| "Not observed".into()),
                    active
                        .as_ref()
                        .map(|r| r.state.label())
                        .unwrap_or("Run verification during playback"),
                    active
                        .as_ref()
                        .map(|r| theme::state_color(r.state))
                        .unwrap_or(theme::muted()),
                );
                metric(
                    &mut columns[1],
                    "WINDOWS AUDIO FORMAT",
                    &render
                        .as_ref()
                        .and_then(|e| e.mix_format.as_ref().or(e.shared_mode_format.as_ref()))
                        .map(|f| f.label())
                        .unwrap_or_else(|| "Unavailable".into()),
                    "Engine format — not the Bluetooth codec",
                    theme::accent(),
                );
                metric(
                    &mut columns[2],
                    "LINK / ENDPOINT",
                    if device.connected {
                        "Connected"
                    } else {
                        "Offline"
                    },
                    &render
                        .as_ref()
                        .map(|e| format!("{} · {}", e.state.label(), e.form_factor))
                        .unwrap_or_else(|| "No associated output endpoint".into()),
                    if device.connected {
                        theme::good()
                    } else {
                        theme::muted()
                    },
                );
            });
        });
    } else {
        theme::hero_panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                logo(ui, 48.0);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("No Bluetooth audio device selected")
                            .size(20.0)
                            .strong(),
                    );
                    ui.label(
                        RichText::new(
                            "Pair and connect headphones in Windows Settings, then refresh.",
                        )
                        .color(theme::muted()),
                    );
                });
            });
        });
    }

    ui.add_space(14.0);
    if let Some((ok, note)) = &app.capture_note {
        notice_banner(ui, *ok, note);
        ui.add_space(12.0);
    }
    inbox_codec_card(app, ui);

    if let Some(device) = &device {
        ui.columns(2, |columns| {
            theme::subtle_panel().show(&mut columns[0], |ui| {
                section_heading(ui, "Negotiation status", Some("Evidence, not preference"));
                if let Some(active) = &active {
                    codec_summary(ui, active);
                } else {
                    ui.label(
                        RichText::new("The active codec has not been observed.")
                            .size(13.0)
                            .color(theme::text()),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "Windows does not report it through a normal API. Play audio, then use Verify live codec.",
                        )
                        .size(11.5)
                        .color(theme::muted()),
                    );
                }
                ui.add_space(12.0);
                key_value(
                    ui,
                    "A2DP profile",
                    match device.supports_a2dp() {
                        Some(true) => "Advertised",
                        Some(false) => "Not observed",
                        None => "Unknown",
                    },
                );
                key_value(
                    ui,
                    "Hands-Free",
                    if device.supports_hfp() {
                        "Advertised"
                    } else {
                        "Not observed"
                    },
                );
                key_value(ui, "Endpoint association", device.association.method.label());
            });

            theme::subtle_panel().show(&mut columns[1], |ui| {
                section_heading(ui, "Audio path", Some("Separate layers"));
                if let Some(endpoint) = &render {
                    key_value(ui, "Output endpoint", &endpoint.friendly_name);
                    key_value(ui, "State", endpoint.state.label());
                    if let Some(format) = &endpoint.mix_format {
                        key_value(ui, "Engine mix", &format.label());
                        key_value(ui, "Engine PCM rate", &format!("{} kbit/s", format.pcm_kbps()));
                    } else if let Some(format) = &endpoint.shared_mode_format {
                        key_value(ui, "Stored format", &format.label());
                    }
                    if endpoint.is_default_console {
                        key_value(ui, "Default for", "media");
                    }
                } else {
                    ui.label(
                        RichText::new("No output endpoint is associated with this device.")
                            .size(12.0)
                            .color(theme::warn()),
                    );
                }
                if let Some(capture) = &capture {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(
                            "A microphone endpoint exists. Opening it can switch the device from A2DP to HFP.",
                        )
                        .size(11.0)
                        .color(theme::muted()),
                    );
                    key_value(ui, "Capture state", capture.state.label());
                }
            });
        });
    }

    ui.add_space(14.0);
    theme::subtle_panel().show(ui, |ui| {
        section_heading(ui, "What Windows exposes", Some("Capability boundary"));
        ui.label(
            RichText::new(
                "Bluu reads codec inventory, remote profile records, endpoints and formats. Windows has no documented A2DP codec-registration API. The narrow AAC/aptX Classic preference and the optional native provider are shown separately, with their own safety and evidence labels.",
            )
            .size(12.0)
            .color(theme::muted()),
        );
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            pill(ui, "SBC", theme::good());
            pill(ui, "AAC on Windows 11", theme::good());
            pill(ui, "aptX in-box", theme::good());
            pill(ui, "aptX Adaptive: select Qualcomm OEMs / provider", theme::warn());
            pill(ui, "LDAC / aptX HD: experimental native provider", theme::violet());
            pill(ui, "LE Audio: vendor ACX path", theme::violet());
        });
    });

    ui.add_space(14.0);
    let top: Vec<_> = findings.iter().take(3).collect();
    if !top.is_empty() {
        section_heading(ui, "Health checks", Some("Actionable findings"));
        for finding in top {
            finding_card(ui, finding);
            ui.add_space(8.0);
        }
    }
}

fn metric(ui: &mut Ui, label: &str, value: &str, sub: &str, color: Color32) {
    ui.label(
        RichText::new(label)
            .size(9.5)
            .strong()
            .color(theme::faint()),
    );
    ui.add_space(3.0);
    ui.label(RichText::new(value).size(18.0).strong().color(color));
    ui.label(RichText::new(sub).size(10.5).color(theme::muted()));
}

fn codec_summary(ui: &mut Ui, report: &CodecReport) {
    ui.horizontal(|ui| {
        dot(ui, theme::state_color(report.state));
        ui.label(
            RichText::new(report.codec.label())
                .size(20.0)
                .strong()
                .color(theme::text()),
        );
        pill(
            ui,
            report.confidence.label(),
            theme::state_color(report.state),
        );
    });
    ui.add_space(3.0);
    ui.label(
        RichText::new(&report.summary)
            .size(11.5)
            .color(theme::muted()),
    );
    if let Some(at) = report.observed_at {
        ui.label(
            RichText::new(format!(
                "Observed {}",
                at.with_timezone(&chrono::Local).format("%H:%M:%S")
            ))
            .size(10.5)
            .color(theme::faint()),
        );
    }
}

pub fn devices(app: &mut BluuApp, ui: &mut Ui) {
    let snapshot = app.snapshot.clone();
    page_heading(
        ui,
        "Devices",
        "Paired and connected hardware",
        "Connection state comes from Windows. Profile records are decoded from cached SDP; codec capability is never inferred from a product name.",
    );
    let Some(snapshot) = snapshot else {
        ui.label(RichText::new("Discovery in progress…").color(theme::muted()));
        return;
    };

    let selected = snapshot
        .selected_device(app.selected_device.as_deref())
        .cloned();
    if let Some(device) = selected {
        theme::subtle_panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(&device.name)
                            .size(22.0)
                            .strong()
                            .color(theme::text()),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            device
                                .address
                                .as_ref()
                                .map(|a| a.display_masked())
                                .unwrap_or_else(|| "Address unavailable".into()),
                            device.transport.label()
                        ))
                        .size(11.5)
                        .color(theme::muted()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Select in sidebar").clicked() {
                        app.selected_device = Some(device.id.clone());
                        app.refresh_provider_selection();
                        app.save_settings();
                    }
                    if device.connected {
                        pill(ui, "CONNECTED", theme::good());
                    } else {
                        pill(ui, "PAIRED / OFFLINE", theme::muted());
                    }
                });
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            ui.columns(3, |columns| {
                info_block(
                    &mut columns[0],
                    "Category",
                    device.category.as_deref().unwrap_or("Unknown"),
                );
                info_block(
                    &mut columns[1],
                    "A2DP",
                    match device.supports_a2dp() {
                        Some(true) => "Advertised",
                        Some(false) => "Not advertised",
                        None => "Not determined",
                    },
                );
                info_block(
                    &mut columns[2],
                    "Endpoint matching",
                    device.association.method.label(),
                );
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(&device.association.note)
                    .size(10.5)
                    .color(theme::faint()),
            );
        });
        ui.add_space(14.0);
        theme::subtle_panel().show(ui, |ui| {
            section_heading(ui, "Advertised profiles", Some("Cached SDP records"));
            if device.profiles.is_empty() {
                ui.label(
                    RichText::new(
                        "No profile records were available. This can happen for LE-only devices or an empty Windows cache.",
                    )
                    .color(theme::muted()),
                );
            }
            for profile in &device.profiles {
                ui.horizontal(|ui| {
                    dot(ui, theme::accent());
                    ui.label(
                        RichText::new(profile.profile.label())
                            .size(13.0)
                            .color(theme::text())
                            .strong(),
                    );
                    if let Some(version) = &profile.version {
                        ui.label(
                            RichText::new(format!("v{version}")).size(11.0).color(theme::muted()),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(profile.evidence.label())
                                .size(10.0)
                                .color(theme::faint()),
                        );
                    });
                });
                ui.label(
                    RichText::new(&profile.note).size(10.5).color(theme::faint()),
                );
                ui.add_space(5.0);
            }
        });
        ui.add_space(14.0);
        theme::subtle_panel().show(ui, |ui| {
            section_heading(ui, "Audio endpoints", Some("Windows Core Audio"));
            if device.endpoint_ids.is_empty() {
                ui.label(
                    RichText::new("No associated endpoints. Check Windows Sound settings.")
                        .color(theme::warn()),
                );
            }
            for id in &device.endpoint_ids {
                if let Some(endpoint) = snapshot.endpoint(id) {
                    endpoint_row(ui, endpoint);
                }
            }
        });
    }

    ui.add_space(14.0);
    section_heading(
        ui,
        "All paired devices",
        Some(&format!("{} found", snapshot.devices.len())),
    );
    for device in &snapshot.devices {
        let selected = app.selected_device.as_deref() == Some(device.id.as_str());
        let response = theme::subtle_panel()
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    dot(
                        ui,
                        if device.connected {
                            theme::good()
                        } else {
                            theme::faint()
                        },
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&device.name).size(14.0).strong().color(
                            if selected {
                                theme::text()
                            } else {
                                theme::muted()
                            },
                        ));
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                if device.connected {
                                    "Connected"
                                } else {
                                    "Disconnected"
                                },
                                device
                                    .address
                                    .as_ref()
                                    .map(|a| a.display_masked())
                                    .unwrap_or_else(|| "address unavailable".into())
                            ))
                            .size(10.5)
                            .color(theme::faint()),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(if device.connected {
                                "A2DP music path"
                            } else {
                                "Not streaming"
                            })
                            .size(10.5)
                            .color(theme::muted()),
                        );
                    });
                });
            })
            .response;
        if response.interact(Sense::click()).clicked() {
            app.selected_device = Some(device.id.clone());
            app.refresh_provider_selection();
            app.save_settings();
        }
        ui.add_space(6.0);
    }
}

fn info_block(ui: &mut Ui, label: &str, value: &str) {
    ui.label(
        RichText::new(label.to_uppercase())
            .size(9.0)
            .strong()
            .color(theme::faint()),
    );
    ui.label(RichText::new(value).size(12.5).color(theme::text()));
}

fn endpoint_row(ui: &mut Ui, endpoint: &AudioEndpoint) {
    ui.horizontal(|ui| {
        dot(
            ui,
            if endpoint.state == EndpointState::Active {
                theme::good()
            } else {
                theme::faint()
            },
        );
        ui.label(
            RichText::new(&endpoint.friendly_name)
                .size(12.5)
                .color(theme::text()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(endpoint.state.label())
                    .size(10.5)
                    .color(theme::muted()),
            );
        });
    });
    if let Some(format) = endpoint
        .mix_format
        .as_ref()
        .or(endpoint.shared_mode_format.as_ref())
    {
        ui.label(
            RichText::new(format!(
                "{} · {}",
                format.label(),
                if endpoint.mix_format.is_some() {
                    "audio engine mix"
                } else {
                    "stored endpoint format"
                }
            ))
            .size(10.5)
            .color(theme::faint()),
        );
    }
    ui.add_space(5.0);
}

pub fn codecs(app: &mut BluuApp, ui: &mut Ui) {
    let device = app.current_device().cloned();
    let reports = app.reports();
    page_heading(
        ui,
        "Codecs",
        "Supported, negotiated, verified — kept separate",
        "A codec can be in the Windows inventory without being active, and a remote device can support codecs Windows never offers.",
    );
    let Some(device) = device else {
        empty_state(
            ui,
            "No Bluetooth device",
            "Connect a device to inspect codec states.",
        );
        inbox_codec_card(app, ui);
        return;
    };
    inbox_codec_card(app, ui);

    let active = reports.iter().find(|r| r.state.is_active()).cloned();
    let selected = app
        .selected_codec
        .and_then(|c| reports.iter().find(|r| r.codec == c).cloned())
        .or(active.clone())
        .or_else(|| {
            reports
                .iter()
                .find(|r| r.state == CodecState::SupportedAvailable)
                .cloned()
        });

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{} · {}", device.name, device.transport.label()))
                .size(12.0)
                .color(theme::muted()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled(
                    !app.capturing,
                    egui::Button::new(if app.capturing {
                        "Listening…"
                    } else {
                        "Observe live codec"
                    })
                    .fill(theme::accent_soft()),
                )
                .clicked()
            {
                app.verify_codec();
            }
        });
    });
    if let Some((ok, note)) = &app.capture_note {
        ui.add_space(8.0);
        notice_banner(ui, *ok, note);
    }
    ui.add_space(14.0);

    for report in &reports {
        let is_selected = selected.as_ref().map(|s| s.codec) == Some(report.codec);
        let state_color = theme::state_color(report.state);
        let response = egui::Frame::new()
            .fill(if is_selected {
                theme::panel_raised()
            } else {
                theme::panel()
            })
            .stroke(Stroke::new(
                if is_selected { 1.2_f32 } else { 1.0_f32 },
                if is_selected {
                    theme::accent()
                } else {
                    theme::border()
                },
            ))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    dot(ui, state_color);
                    ui.label(
                        RichText::new(report.codec.label())
                            .size(15.0)
                            .strong()
                            .color(theme::text()),
                    );
                    pill(ui, report.state.label(), state_color);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(report.confidence.label())
                                .size(10.0)
                                .color(theme::faint()),
                        );
                    });
                });
                ui.add_space(3.0);
                ui.label(
                    RichText::new(&report.summary)
                        .size(11.5)
                        .color(theme::muted()),
                );
            })
            .response;
        if response.interact(Sense::click()).clicked() {
            app.selected_codec = Some(report.codec);
        }
        ui.add_space(6.0);
    }

    if let Some(report) = selected {
        ui.add_space(10.0);
        theme::subtle_panel().show(ui, |ui| {
            section_heading(
                ui,
                &format!("{} details", report.codec.label()),
                Some("Separate fields"),
            );
            ui.columns(2, |columns| {
                fact_block(
                    &mut columns[0],
                    "Codec capability",
                    &format!(
                        "{}{}",
                        report
                            .capability
                            .value
                            .map(|v| if v {
                                "In-box on this Windows build"
                            } else {
                                "Not in-box"
                            })
                            .unwrap_or("Unknown"),
                        ""
                    ),
                    &report.capability.note,
                    report.capability.confidence,
                );
                fact_block(
                    &mut columns[1],
                    "Configuration",
                    report
                        .configuration
                        .value
                        .as_deref()
                        .unwrap_or("Not exposed"),
                    &report.configuration.note,
                    report.configuration.confidence,
                );
                columns[0].add_space(8.0);
                fact_block(
                    &mut columns[0],
                    "Negotiated codec",
                    report
                        .negotiated
                        .value
                        .map(|c| c.label())
                        .unwrap_or("Not observed"),
                    &report.negotiated.note,
                    report.negotiated.confidence,
                );
                fact_block(
                    &mut columns[1],
                    "Observed active",
                    report
                        .observed_active
                        .value
                        .map(|c| c.label())
                        .unwrap_or("Not observed"),
                    &format!(
                        "Method: {}{}",
                        report.verification.label(),
                        report
                            .observed_at
                            .map(|t| format!(
                                " · {}",
                                t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")
                            ))
                            .unwrap_or_default()
                    ),
                    report.confidence,
                );
            });
            ui.add_space(12.0);
            ui.label(
                RichText::new("Limitations")
                    .size(11.0)
                    .strong()
                    .color(theme::warn()),
            );
            for limitation in &report.limitations {
                ui.label(
                    RichText::new(format!("• {limitation}"))
                        .size(10.8)
                        .color(theme::muted()),
                );
            }
        });
    }

    ui.add_space(16.0);
    theme::subtle_panel().show(ui, |ui| {
        section_heading(ui, "Codec feasibility", Some("Desktop research matrix"));
        ui.label(
            RichText::new(
                "This matrix describes the generic Windows 10/11 path. OEM hardware can differ; verify the exact PC, radio, driver and headset.",
            )
            .size(10.5)
            .color(theme::muted()),
        );
        ui.add_space(8.0);
        for row in feasibility_matrix() {
            let color = match row.status {
                FeasibilityStatus::NativelySupported => theme::good(),
                FeasibilityStatus::ConfigurableOnSupportedSystems
                | FeasibilityStatus::ConditionallyImplementable => theme::accent(),
                FeasibilityStatus::RequiresSubstantialSystemIntegration => theme::violet(),
                FeasibilityStatus::NotFeasibleSafeSupported => theme::bad(),
                FeasibilityStatus::UnknownRequiresHardwareTesting => theme::warn(),
            };
            egui::CollapsingHeader::new(
                RichText::new(format!("{}  ·  {}", row.codec.label(), row.status.label()))
                    .size(12.0)
                    .color(color),
            )
            .id_salt(row.codec.key())
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("Windows: {}", row.windows_native))
                        .size(10.8)
                        .color(theme::muted()),
                );
                ui.label(
                    RichText::new(format!("OEM/driver: {}", row.oem_driver_dependent))
                        .size(10.8)
                        .color(theme::muted()),
                );
                ui.label(
                    RichText::new(format!("Configuration only: {}", row.config_only))
                        .size(10.8)
                        .color(theme::muted()),
                );
                ui.label(
                    RichText::new(format!("Licensing: {}", row.licensing))
                        .size(10.8)
                        .color(theme::muted()),
                );
            });
        }
    });
}

fn fact_block(ui: &mut Ui, label: &str, value: &str, note: &str, confidence: Confidence) {
    ui.label(
        RichText::new(label.to_uppercase())
            .size(9.0)
            .strong()
            .color(theme::faint()),
    );
    ui.label(
        RichText::new(value)
            .size(13.0)
            .strong()
            .color(theme::text()),
    );
    ui.label(
        RichText::new(format!("{} · {}", note, confidence.label()))
            .size(10.0)
            .color(theme::muted()),
    );
    ui.add_space(8.0);
}

pub fn diagnostics(app: &mut BluuApp, ui: &mut Ui) {
    let snapshot = app.snapshot.clone();
    let reports = app.reports();
    let findings = app.findings(&reports);
    page_heading(
        ui,
        "Diagnostics",
        "Explain the result, including the gaps",
        "Bluu distinguishes absence of evidence from evidence of absence. Windows does not expose every layer needed for a definitive codec diagnosis.",
    );
    let Some(snapshot) = snapshot else {
        empty_state(ui, "No data", "Refresh to collect diagnostics.");
        return;
    };
    if let Some(device) = snapshot.selected_device(app.selected_device.as_deref()) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Scope: {}", device.name))
                    .size(12.0)
                    .color(theme::muted()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Export redacted diagnostics").clicked() {
                    app.export_diagnostics();
                }
                if ui.button("Privacy details").clicked() {
                    app.show_privacy = true;
                }
            });
        });
    }
    if let Some(message) = &app.last_saved {
        ui.add_space(8.0);
        notice_banner(ui, !message.starts_with("Export failed"), message);
    }
    ui.add_space(14.0);

    ui.columns(2, |columns| {
        theme::subtle_panel().show(&mut columns[0], |ui| {
            section_heading(ui, "System", None);
            key_value(ui, "Operating system", &snapshot.system.os_name);
            key_value(ui, "Build", &snapshot.system.os_build.to_string());
            key_value(ui, "Architecture", &snapshot.system.architecture);
            key_value(ui, "Elevated UI", if snapshot.system.elevated { "Yes" } else { "No" });
            key_value(ui, "Backend", &snapshot.backend.name);
            key_value(
                ui,
                "Backend mode",
                if snapshot.backend.read_only {
                    "Read-only"
                } else {
                    "Writable"
                },
            );
        });
        theme::subtle_panel().show(&mut columns[1], |ui| {
            section_heading(ui, "Bluetooth", None);
            if let Some(adapter) = &snapshot.adapter {
                key_value(ui, "Adapter", &adapter.name);
                if let Some(driver) = &adapter.driver_provider {
                    key_value(ui, "Driver", driver);
                }
                if let Some(version) = &adapter.driver_version {
                    key_value(ui, "Driver version", version);
                }
            } else {
                ui.label(
                    RichText::new(
                        "Adapter/driver identity is optional; generic WinRT APIs do not expose it consistently.",
                    )
                    .size(10.5)
                    .color(theme::muted()),
                );
            }
            key_value(ui, "Paired devices", &snapshot.devices.len().to_string());
            key_value(
                ui,
                "Connected audio devices",
                &snapshot.connected_devices().count().to_string(),
            );
            key_value(ui, "Audio endpoints", &snapshot.endpoints.len().to_string());
            key_value(
                ui,
                "Observed codec",
                snapshot
                    .active_codec
                    .as_ref()
                    .map(|o| o.codec.label())
                    .unwrap_or("Not observed"),
            );
        });
    });
    ui.add_space(14.0);
    section_heading(ui, "Findings", Some("Most specific first"));
    for finding in &findings {
        finding_card(ui, finding);
        ui.add_space(8.0);
    }
    if findings.is_empty() {
        ui.label(
            RichText::new("No diagnostics were generated.")
                .size(12.0)
                .color(theme::muted()),
        );
    }
    ui.add_space(14.0);
    theme::subtle_panel().show(ui, |ui| {
        section_heading(ui, "What each layer means", None);
        key_value(ui, "Advertising / SDP", "What the remote device says it supports");
        key_value(ui, "Windows inventory", "What the local in-box stack documents as available");
        key_value(ui, "Negotiation", "What the two endpoints selected");
        key_value(ui, "Observation", "What an independent trace reports for a stream");
        key_value(ui, "Verification", "Observed stream tied to the selected device");
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Bluu only claims the last two when it has the corresponding evidence. A supported codec is never displayed as active.",
            )
            .size(10.8)
            .color(theme::muted()),
        );
    });
    if !snapshot.notes.is_empty() || !snapshot.warnings.is_empty() {
        ui.add_space(14.0);
        theme::subtle_panel().show(ui, |ui| {
            section_heading(ui, "Backend notes", None);
            for note in snapshot.warnings.iter().chain(snapshot.notes.iter()) {
                ui.label(
                    RichText::new(format!("• {note}"))
                        .size(10.8)
                        .color(theme::muted()),
                );
            }
        });
    }
}

fn finding_card(ui: &mut Ui, finding: &DiagnosticFinding) {
    let color = theme::severity_color(finding.severity);
    theme::subtle_panel().show(ui, |ui| {
        ui.horizontal(|ui| {
            dot(ui, color);
            ui.label(
                RichText::new(&finding.title)
                    .size(13.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(finding.evidence.label())
                        .size(9.5)
                        .color(theme::faint()),
                );
            });
        });
        ui.label(
            RichText::new(&finding.detail)
                .size(11.0)
                .color(theme::muted()),
        );
        ui.add_space(3.0);
        ui.label(
            RichText::new(format!("→ {}", finding.action))
                .size(10.8)
                .color(theme::text()),
        );
    });
}

pub fn settings(app: &mut BluuApp, ctx: &egui::Context, ui: &mut Ui) {
    page_heading(
        ui,
        "Settings",
        "Quiet by default",
        "Appearance and local data controls. No cloud account, telemetry, or background service.",
    );

    ui.columns(2, |columns| {
        theme::subtle_panel().show(&mut columns[0], |ui| {
            section_heading(ui, "Appearance", None);
            ui.horizontal(|ui| {
                for (label, value) in [
                    ("Dark", ThemePreference::Dark),
                    ("Light", ThemePreference::Light),
                    ("System", ThemePreference::System),
                ] {
                    let selected = app.settings.theme == value;
                    if ui
                        .add(
                            egui::Button::new(RichText::new(label).color(if selected {
                                theme::text()
                            } else {
                                theme::muted()
                            }))
                            .fill(if selected {
                                theme::accent_soft()
                            } else {
                                theme::panel_raised()
                            }),
                        )
                        .clicked()
                    {
                        app.set_theme(ctx, value);
                    }
                }
            });
            ui.add_space(12.0);
            ui.label(
                RichText::new("Capture duration")
                    .size(11.5)
                    .color(theme::muted()),
            );
            let mut seconds = app.capture_seconds;
            if ui
                .add(egui::Slider::new(&mut seconds, 5..=60).suffix(" s"))
                .changed()
            {
                app.capture_seconds = seconds;
            }
            ui.label(
                RichText::new(
                    "A longer window gives more time to start playback. The helper exits automatically.",
                )
                .size(10.2)
                .color(theme::faint()),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "Startup and background notifications are not installed by this portable build.",
                )
                .size(10.2)
                .color(theme::muted()),
            );
            if ui
                .checkbox(
                    &mut app.settings.include_device_names_in_export,
                    "Include device names in diagnostic exports",
                )
                .changed()
            {
                app.save_settings();
            }
            if ui
                .checkbox(
                    &mut app.settings.include_addresses_in_export,
                    "Include Bluetooth addresses in diagnostic exports",
                )
                .changed()
            {
                app.save_settings();
            }
            ui.label(
                RichText::new(
                    "The default is redacted. Turning either option on is an explicit privacy choice.",
                )
                .size(9.8)
                .color(theme::faint()),
            );
        });

        theme::subtle_panel().show(&mut columns[1], |ui| {
            section_heading(ui, "Diagnostics & privacy", None);
            ui.label(
                RichText::new(
                    "Diagnostic exports are local JSON files. Device names, addresses, usernames, computer name, endpoint IDs and paths are redacted by default.",
                )
                .size(11.0)
                .color(theme::muted()),
            );
            ui.add_space(8.0);
            if ui.button("Export redacted diagnostics").clicked() {
                app.export_diagnostics();
            }
            if ui.button("Explain export contents").clicked() {
                app.show_privacy = true;
            }
            if ui.button("Open local data folder").clicked() {
                let dir = bluu_core::settings::app_data_dir();
                let _ = std::fs::create_dir_all(&dir);
                reveal_folder(&dir);
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new("Logging")
                    .size(11.5)
                    .strong()
                    .color(theme::text()),
            );
            ui.label(
                RichText::new(format!(
                    "{}",
                    bluu_core::settings::log_dir().display()
                ))
                .size(10.0)
                .color(theme::faint()),
            );
            if ui
                .checkbox(&mut app.settings.verbose_logging, "Verbose local logging")
                .changed()
            {
                app.save_settings();
            }
            ui.label(
                RichText::new("Verbose logging changes require a restart.")
                    .size(9.8)
                    .color(theme::faint()),
            );
        });
    });

    ui.add_space(14.0);
    theme::subtle_panel().show(ui, |ui| {
        section_heading(ui, "Safety posture", None);
        ui.horizontal_wrapped(|ui| {
            pill(ui, "Standard user UI", theme::good());
            pill(ui, "One opt-in HKLM toggle", theme::warn());
            pill(ui, "Provider-gated driver path", theme::warn());
            pill(ui, "No Bluu background daemon", theme::good());
            pill(ui, "No network code", theme::good());
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Bluu's own elevation is limited to the short-lived helper (ETW or the explicitly requested BthA2dp AAC value) and, for an advanced provider, the provider's separately declared elevation prompt. A provider may install/control a vendor driver or service under its own signed installer and licence.",
            )
            .size(10.8)
            .color(theme::muted()),
        );
    });
    ui.add_space(14.0);
    theme::subtle_panel().show(ui, |ui| {
        section_heading(ui, "Reset", None);
        ui.horizontal(|ui| {
            if ui.button("Restore defaults").clicked() {
                app.restore_defaults(ctx);
            }
            if ui.button("Open diagnostics folder").clicked() {
                let dir = bluu_core::settings::export_dir();
                let _ = std::fs::create_dir_all(&dir);
                reveal_folder(&dir);
            }
        });
        if let Some(status) = &app.last_saved {
            ui.add_space(6.0);
            ui.label(RichText::new(status).size(10.5).color(theme::muted()));
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Installation: Bluu ships as a portable directory with bluu.exe and bluu-etw-helper.exe side by side. The optional native provider is packaged but not installed; a driver/service requires its separate explicit installer and administrator approval.",
            )
            .size(10.5)
            .color(theme::muted()),
        );
    });
}

fn reveal_folder(path: &std::path::Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
}

fn empty_state(ui: &mut Ui, title: &str, body: &str) {
    theme::subtle_panel().show(ui, |ui| {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            logo(ui, 54.0);
            ui.add_space(8.0);
            ui.label(RichText::new(title).size(17.0).strong());
            ui.label(RichText::new(body).size(11.5).color(theme::muted()));
        });
        ui.add_space(20.0);
    });
}

fn notice_banner(ui: &mut Ui, ok: bool, text: &str) {
    let color = if ok { theme::good() } else { theme::warn() };
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            18,
        ))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 100),
        ))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                dot(ui, color);
                ui.label(RichText::new(text).size(11.0).color(theme::text()));
            });
        });
}

pub fn error_banner(ui: &mut Ui, title: &str, detail: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            theme::bad().r(),
            theme::bad().g(),
            theme::bad().b(),
            18,
        ))
        .stroke(Stroke::new(1.0_f32, theme::bad()))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(12.5).strong().color(theme::bad()));
            ui.label(RichText::new(detail).size(11.0).color(theme::text()));
        });
}

/// Advanced provider management. This page is intentionally explicit that the
/// provider owns driver/transport/licensing; Bluu only validates and invokes it.
pub fn providers(app: &mut BluuApp, ctx: &egui::Context, ui: &mut Ui) {
    let snapshot = app.snapshot.clone();
    let device = app.current_device().cloned();
    page_heading(
        ui,
        "Advanced providers",
        "Codec preference through an explicit provider",
        "Windows has no public codec-registration API. A provider supplies the encoder/transport and may replace the A2DP driver. Bluu validates and invokes it; it does not bundle proprietary codecs.",
    );
    let Some(snapshot) = snapshot else {
        empty_state(
            ui,
            "No scan yet",
            "Refresh to discover providers and devices.",
        );
        return;
    };
    app.ensure_provider_selection();
    ui.horizontal_wrapped(|ui| {
        if ui.button("Install provider folder…").clicked() {
            app.find_provider_package();
        }
        if app.selected_provider.is_some() && ui.button("Remove provider package").clicked() {
            app.remove_selected_provider();
        }
    });
    if let Some((ok, message)) = &app.provider_note {
        ui.add_space(6.0);
        notice_banner(ui, *ok, message);
    }
    ui.add_space(10.0);
    if snapshot.providers.is_empty() {
        theme::subtle_panel().show(ui, |ui| {
            section_heading(
                ui,
                "No provider installed",
                Some("This is expected for a clean build"),
            );
            ui.label(
                RichText::new(
                    "Use Install provider folder… or place a provider directory under %LOCALAPPDATA%\\Bluu\\providers or beside bluu.exe. A provider manifest can advertise LDAC, aptX HD, aptX Adaptive, or a vendor codec only when its driver/service implements that codec and transport.",
                )
                .size(11.5)
                .color(theme::muted()),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "Search path: {}",
                    bluu_core::settings::provider_dir().display()
                ))
                .size(10.5)
                .color(theme::faint()),
            );
        });
        return;
    }

    ui.label(
        RichText::new("PROVIDERS")
            .size(10.0)
            .strong()
            .color(theme::faint()),
    );
    for record in &snapshot.providers {
        let selected = app.selected_provider.as_deref() == Some(record.manifest.id.as_str());
        let color = if selected {
            theme::accent()
        } else if record.is_usable() {
            theme::good()
        } else {
            theme::bad()
        };
        let response = theme::subtle_panel()
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    dot(ui, color);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&record.manifest.name)
                                .size(14.5)
                                .strong()
                                .color(theme::text()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · {} · {}",
                                record.manifest.version,
                                record.manifest.stack.label(),
                                record.manifest.publisher,
                                if record.manifest.requires_elevation {
                                    "UAC required"
                                } else {
                                    "standard user"
                                }
                            ))
                            .size(10.5)
                            .color(theme::muted()),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        pill(ui, &record.manifest.id, color);
                    });
                });
                if !record.issues.is_empty() {
                    ui.add_space(5.0);
                    for issue in &record.issues {
                        ui.label(
                            RichText::new(format!("• {issue}"))
                                .size(10.5)
                                .color(theme::bad()),
                        );
                    }
                }
                ui.add_space(5.0);
                ui.label(
                    RichText::new(format!(
                        "Codecs: {}",
                        record
                            .manifest
                            .codecs
                            .iter()
                            .map(|c| c.codec.label())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .size(10.5)
                    .color(theme::muted()),
                );
            })
            .response;
        if response.interact(Sense::click()).clicked() {
            app.select_provider(record.manifest.id.clone());
        }
        ui.add_space(7.0);
    }
    ui.add_space(10.0);

    let Some(record) = app.selected_provider_record().cloned() else {
        return;
    };
    let Some(device) = device else {
        empty_state(
            ui,
            "No device selected",
            "Connect and select headphones first.",
        );
        return;
    };
    if !device.connected {
        notice_banner(
            ui,
            false,
            "The selected device is disconnected; Apply/Verify are disabled.",
        );
        ui.add_space(8.0);
    }

    theme::hero_panel().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("{} → {}", record.manifest.name, device.name))
                        .size(18.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(
                        "The provider owns codec encoding, A2DP transport, driver signing, and licensing. A preference is not an active-codec claim.",
                    )
                    .size(10.8)
                    .color(theme::muted()),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(ui, record.manifest.stack.label(), theme::violet());
            });
        });
        ui.add_space(10.0);
        if !record.simulated {
            ui.checkbox(
                &mut app.provider_consented,
                "I understand this provider may change system/audio settings or control a driver/service, under its publisher's terms.",
            );
        }
        ui.add_space(8.0);
        section_heading(ui, "Preference order", Some("First codec has highest priority"));
        let all = record.manifest.codecs.clone();
        for (index, codec) in app.provider_order.clone().iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}. {}", index + 1, codec.label()))
                        .size(12.0)
                        .color(theme::text()),
                );
                if index > 0 && ui.small_button("Up").clicked() {
                    app.provider_order.swap(index, index - 1);
                }
                if index + 1 < app.provider_order.len() && ui.small_button("Down").clicked() {
                    app.provider_order.swap(index, index + 1);
                }
                if ui.small_button("Remove").clicked() {
                    app.provider_order.remove(index);
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            for codec in &all {
                if !app.provider_order.contains(&codec.codec)
                    && ui.small_button(format!("+ {}", codec.codec.label())).clicked()
                {
                    app.provider_order.push(codec.codec);
                }
            }
        });
        ui.add_space(8.0);
        for codec in app.provider_order.clone() {
            if let Some(entry) = record.manifest.codecs.iter().find(|c| c.codec == codec) {
                if entry.parameters.is_empty() {
                    continue;
                }
                ui.label(
                    RichText::new(format!("{} parameters", codec.label()))
                        .size(11.0)
                        .strong()
                        .color(theme::text()),
                );
                for parameter in &entry.parameters {
                    match parameter {
                        bluu_core::ProviderParameter::Choice {
                            key,
                            label,
                            options,
                            note,
                            ..
                        } => {
                            let current = app
                                .provider_parameters
                                .get(key)
                                .cloned()
                                .unwrap_or_else(|| parameter.default_value());
                            egui::ComboBox::from_label(label.as_str())
                                .selected_text(current.clone())
                                .show_ui(ui, |ui| {
                                    for option in options {
                                        if ui
                                            .selectable_label(current == *option, option)
                                            .clicked()
                                        {
                                            app.provider_parameters
                                                .insert(key.clone(), option.clone());
                                        }
                                    }
                                });
                            if !note.is_empty() {
                                ui.label(
                                    RichText::new(note).size(9.8).color(theme::faint()),
                                );
                            }
                        }
                        bluu_core::ProviderParameter::Integer {
                            key,
                            label,
                            min,
                            max,
                            unit,
                            note,
                            ..
                        } => {
                            let mut current = app
                                .provider_parameters
                                .get(key)
                                .and_then(|v| v.parse::<i64>().ok())
                                .unwrap_or(*min);
                            if ui
                                .add(egui::Slider::new(&mut current, *min..=*max).text(label))
                                .changed()
                            {
                                app.provider_parameters
                                    .insert(key.clone(), current.to_string());
                            }
                            let _ = unit;
                            if !note.is_empty() {
                                ui.label(
                                    RichText::new(note).size(9.8).color(theme::faint()),
                                );
                            }
                        }
                        bluu_core::ProviderParameter::Boolean {
                            key,
                            label,
                            note,
                            ..
                        } => {
                            let mut current = app
                                .provider_parameters
                                .get(key)
                                .map(|v| v == "true")
                                .unwrap_or(false);
                            if ui.checkbox(&mut current, label).changed() {
                                app.provider_parameters
                                    .insert(key.clone(), current.to_string());
                            }
                            if !note.is_empty() {
                                ui.label(
                                    RichText::new(note).size(9.8).color(theme::faint()),
                                );
                            }
                        }
                    }
                }
            }
        }
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            let can_apply = !app.provider_busy
                && device.connected
                && record.is_usable()
                && (record.simulated || app.provider_consented);
            if ui
                .add_enabled(
                    can_apply,
                    egui::Button::new("Apply preference").fill(theme::accent_soft()),
                )
                .clicked()
            {
                app.provider_operation(ProviderAction::Apply);
            }
            if ui
                .add_enabled(
                    can_apply,
                    egui::Button::new("Verify active codec"),
                )
                .clicked()
            {
                app.provider_operation(ProviderAction::Verify);
            }
            if ui
                .add_enabled(!app.provider_busy, egui::Button::new("Rollback"))
                .clicked()
            {
                app.provider_rollback();
            }
            if app.provider_busy {
                ui.spinner();
                ui.label(RichText::new("Provider operation running…").color(theme::muted()));
            }
        });
        if let Some((ok, note)) = &app.provider_note {
            ui.add_space(8.0);
            notice_banner(ui, *ok, note);
        }
    });

    ui.add_space(12.0);
    theme::subtle_panel().show(ui, |ui| {
        section_heading(ui, "Provider identity and evidence", None);
        key_value(ui, "Manifest", &record.manifest_path);
        key_value(ui, "Provider", &record.manifest.name);
        key_value(
            ui,
            "Executable SHA-256",
            record.executable_sha256.as_deref().unwrap_or("not available"),
        );
        key_value(ui, "Stack", record.manifest.stack.label());
        key_value(
            ui,
            "Simulated",
            if record.simulated { "Yes — no real transport" } else { "No" },
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Advanced providers are outside the Windows in-box stack. A provider must be installed and supported by its publisher; Bluu does not download or bundle it.",
            )
            .size(10.8)
            .color(theme::muted()),
        );
    });
    let _ = ctx;
}
