use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

/// The interface is dark-first. The light palette is a deliberate inversion rather
/// than an automatic egui default; both are checked for contrast in the design notes.
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub violet: Color32,
}

static ACTIVE_LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static PALETTES: [Palette; 2] = [
    Palette {
        bg: Color32::from_rgb(13, 16, 22),
        panel: Color32::from_rgb(20, 25, 34),
        panel_raised: Color32::from_rgb(26, 32, 43),
        border: Color32::from_rgb(40, 50, 65),
        text: Color32::from_rgb(234, 240, 248),
        muted: Color32::from_rgb(145, 160, 181),
        faint: Color32::from_rgb(106, 120, 140),
        accent: Color32::from_rgb(125, 178, 255),
        accent_soft: Color32::from_rgb(34, 56, 90),
        good: Color32::from_rgb(100, 211, 166),
        warn: Color32::from_rgb(239, 187, 102),
        bad: Color32::from_rgb(239, 130, 137),
        violet: Color32::from_rgb(185, 166, 255),
    },
    Palette {
        bg: Color32::from_rgb(246, 248, 252),
        panel: Color32::from_rgb(255, 255, 255),
        panel_raised: Color32::from_rgb(236, 241, 248),
        border: Color32::from_rgb(201, 212, 226),
        text: Color32::from_rgb(23, 31, 44),
        muted: Color32::from_rgb(79, 94, 115),
        faint: Color32::from_rgb(105, 120, 140),
        accent: Color32::from_rgb(31, 92, 191),
        accent_soft: Color32::from_rgb(220, 233, 251),
        good: Color32::from_rgb(16, 117, 80),
        warn: Color32::from_rgb(140, 86, 8),
        bad: Color32::from_rgb(169, 40, 52),
        violet: Color32::from_rgb(94, 65, 170),
    },
];

pub fn palette() -> &'static Palette {
    if ACTIVE_LIGHT.load(std::sync::atomic::Ordering::Relaxed) {
        &PALETTES[1]
    } else {
        &PALETTES[0]
    }
}

fn palette_for(theme: egui::Theme) -> &'static Palette {
    match theme {
        egui::Theme::Dark => &PALETTES[0],
        egui::Theme::Light => &PALETTES[1],
    }
}

pub fn bg() -> Color32 {
    palette().bg
}
pub fn panel() -> Color32 {
    palette().panel
}
pub fn panel_raised() -> Color32 {
    palette().panel_raised
}
pub fn border() -> Color32 {
    palette().border
}
pub fn text() -> Color32 {
    palette().text
}
pub fn muted() -> Color32 {
    palette().muted
}
pub fn faint() -> Color32 {
    palette().faint
}
pub fn accent() -> Color32 {
    palette().accent
}
pub fn accent_soft() -> Color32 {
    palette().accent_soft
}
pub fn good() -> Color32 {
    palette().good
}
pub fn warn() -> Color32 {
    palette().warn
}
pub fn bad() -> Color32 {
    palette().bad
}
pub fn violet() -> Color32 {
    palette().violet
}

fn apply_theme_style(ctx: &egui::Context, theme: egui::Theme) {
    let dark = theme == egui::Theme::Dark;
    let mut style = (*ctx.style_of(theme)).clone();
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let p = palette_for(theme);
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.bg;
    visuals.faint_bg_color = p.panel;
    visuals.extreme_bg_color = if dark {
        Color32::from_rgb(9, 12, 17)
    } else {
        Color32::from_rgb(236, 241, 248)
    };
    visuals.widgets.noninteractive.bg_fill = p.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.inactive.bg_fill = p.panel_raised;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.hovered.bg_fill = p.accent_soft;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
    visuals.widgets.active.bg_fill = p.accent_soft;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
    visuals.selection.bg_fill = p.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.accent;
    visuals.window_corner_radius = CornerRadius::same(12);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(12.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.window_margin = egui::Margin::same(16);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(26.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style_of(theme, style);
}

pub fn configure(ctx: &egui::Context, preference: bluu_core::settings::ThemePreference) {
    match preference {
        bluu_core::settings::ThemePreference::Dark => ctx.set_theme(egui::Theme::Dark),
        bluu_core::settings::ThemePreference::Light => ctx.set_theme(egui::Theme::Light),
        bluu_core::settings::ThemePreference::System => {
            ctx.set_theme(egui::ThemePreference::System)
        }
    }
    apply_theme_style(ctx, egui::Theme::Dark);
    apply_theme_style(ctx, egui::Theme::Light);
    sync(ctx);
}

/// Update palette selection after egui resolves a System preference.
pub fn sync(ctx: &egui::Context) {
    ACTIVE_LIGHT.store(
        ctx.theme() == egui::Theme::Light,
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub fn subtle_panel() -> egui::Frame {
    egui::Frame::new()
        .fill(panel())
        .stroke(Stroke::new(1.0_f32, border()))
        .corner_radius(14)
        .inner_margin(egui::Margin::symmetric(18, 16))
}

pub fn hero_panel() -> egui::Frame {
    egui::Frame::new()
        .fill(panel_raised())
        .stroke(Stroke::new(1.0_f32, border()))
        .corner_radius(18)
        .inner_margin(egui::Margin::symmetric(22, 20))
}

pub fn state_color(state: bluu_core::CodecState) -> Color32 {
    use bluu_core::CodecState;
    match state {
        CodecState::ActiveVerified => good(),
        CodecState::ActiveProviderAttested => good(),
        CodecState::ActiveUnverified => warn(),
        CodecState::ProviderPreferenceApplied => accent(),
        CodecState::ProviderAvailable => violet(),
        CodecState::SupportedAvailable => accent(),
        CodecState::HardwareOnly => violet(),
        CodecState::UnavailableOnBackend | CodecState::Unsupported => bad(),
        CodecState::Unknown => muted(),
    }
}

pub fn severity_color(severity: bluu_core::diagnostics::Severity) -> Color32 {
    use bluu_core::diagnostics::Severity;
    match severity {
        Severity::Info => accent(),
        Severity::Attention => warn(),
        Severity::Limitation => violet(),
    }
}
