//! Tiny headless renderer used by `cargo xtask screenshots` and CI. It is not a second
//! UI: it draws a stable, representative 1200x780 marketing frame from the same palette
//! and demo narrative as the egui interface so repository docs cannot drift wildly.

use image::{ImageBuffer, Rgb, RgbImage};

const W: u32 = 1200;
const H: u32 = 780;

struct Canvas {
    image: RgbImage,
}

impl Canvas {
    fn new() -> Self {
        Self {
            image: ImageBuffer::from_pixel(W, H, Rgb([13, 16, 22])),
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: [u8; 3]) {
        for py in y.max(0)..(y + h).min(H as i32) {
            for px in x.max(0)..(x + w).min(W as i32) {
                self.image.put_pixel(px as u32, py as u32, Rgb(color));
            }
        }
    }

    fn round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: [u8; 3]) {
        for py in y.max(0)..(y + h).min(H as i32) {
            for px in x.max(0)..(x + w).min(W as i32) {
                let dx = if px < x + radius {
                    x + radius - px
                } else if px >= x + w - radius {
                    px - (x + w - radius - 1)
                } else {
                    0
                };
                let dy = if py < y + radius {
                    y + radius - py
                } else if py >= y + h - radius {
                    py - (y + h - radius - 1)
                } else {
                    0
                };
                if dx == 0 || dy == 0 || dx * dx + dy * dy <= radius * radius {
                    self.image.put_pixel(px as u32, py as u32, Rgb(color));
                }
            }
        }
    }

    fn circle(&mut self, cx: i32, cy: i32, radius: i32, color: [u8; 3]) {
        for py in (cy - radius).max(0)..(cy + radius).min(H as i32) {
            for px in (cx - radius).max(0)..(cx + radius).min(W as i32) {
                let dx = px - cx;
                let dy = py - cy;
                if dx * dx + dy * dy <= radius * radius {
                    self.image.put_pixel(px as u32, py as u32, Rgb(color));
                }
            }
        }
    }

    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 3]) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            if x >= 0 && x < W as i32 && y >= 0 && y < H as i32 {
                self.image.put_pixel(x as u32, y as u32, Rgb(color));
            }
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, color: [u8; 3]) {
        let radius = (h / 2).max(1);
        self.round_rect(x, y, w, h, radius, color);
    }
}

const BG: [u8; 3] = [13, 16, 22];
const PANEL: [u8; 3] = [20, 25, 34];
const RAISED: [u8; 3] = [26, 32, 43];
const BORDER: [u8; 3] = [40, 50, 65];
const TEXT: [u8; 3] = [234, 240, 248];
const MUTED: [u8; 3] = [145, 160, 181];
const FAINT: [u8; 3] = [91, 105, 124];
const ACCENT: [u8; 3] = [112, 168, 255];
const ACCENT_DIM: [u8; 3] = [34, 56, 90];
const GOOD: [u8; 3] = [100, 211, 166];
const WARN: [u8; 3] = [239, 187, 102];
const BAD: [u8; 3] = [239, 130, 137];

pub fn render_overview() -> RgbImage {
    let mut c = Canvas::new();
    // Top bar.
    c.rect(0, 0, W as i32, 60, BG);
    glyph_logo(&mut c, 30, 18, 24);
    text(&mut c, 64, 19, "bluu", TEXT, 2);
    text(&mut c, 126, 25, "Bluetooth audio, with evidence", FAINT, 1);
    nav(&mut c, 500);
    c.round_rect(1112, 17, 58, 26, 7, RAISED);
    text(&mut c, 1124, 23, "Refresh", MUTED, 1);

    // Sidebar.
    c.rect(0, 60, 250, H as i32 - 60, BG);
    c.rect(249, 60, 1, H as i32 - 60, [29, 36, 47]);
    text(&mut c, 20, 92, "DEVICE", FAINT, 1);
    device_item(&mut c, 16, 112, "Studio Headphones", true, true);
    device_item(&mut c, 16, 186, "Living Room Speaker", false, false);
    text(&mut c, 20, 292, "BACKEND", FAINT, 1);
    text(&mut c, 20, 312, "Windows Bluetooth + Core Audio", MUTED, 1);
    text(&mut c, 20, 332, "Read-only inspection", FAINT, 1);
    text(&mut c, 20, 352, "Windows 11 · build 26100", FAINT, 1);
    text(&mut c, 20, 722, "No telemetry · local only", FAINT, 1);
    text(&mut c, 20, 744, "About & limitations", MUTED, 1);

    // Main page title.
    text(&mut c, 282, 90, "OVERVIEW", ACCENT, 1);
    text(
        &mut c,
        282,
        108,
        "A clear view of your wireless audio",
        TEXT,
        2,
    );
    text(
        &mut c,
        282,
        142,
        "Connection state, audio endpoint format, and the codec actually observed — without invented controls.",
        MUTED,
        1,
    );

    // Hero card.
    c.round_rect(282, 178, 888, 190, 16, RAISED);
    c.round_rect(282, 178, 888, 190, 16, RAISED);
    glyph_logo(&mut c, 304, 202, 52);
    text(&mut c, 374, 202, "Studio Headphones", TEXT, 2);
    c.bar(374, 228, 78, 18, [27, 55, 46]);
    text(&mut c, 383, 231, "CONNECTED", GOOD, 1);
    text(
        &mut c,
        374,
        252,
        "Headphones · Dual-mode Bluetooth",
        MUTED,
        1,
    );
    c.round_rect(928, 206, 218, 36, 8, ACCENT_DIM);
    c.round_rect(928, 206, 218, 36, 8, ACCENT_DIM);
    text(&mut c, 952, 216, "Verify live codec", TEXT, 1);
    c.line(282, 276, 1170, 276, BORDER);
    metric(
        &mut c,
        306,
        292,
        "ACTIVE A2DP CODEC",
        "AAC",
        "Supported · not observed",
        ACCENT,
    );
    metric(
        &mut c,
        606,
        292,
        "WINDOWS AUDIO FORMAT",
        "48.0 kHz · 2 ch · 24 bit",
        "Engine format, not codec",
        TEXT,
    );
    metric(
        &mut c,
        906,
        292,
        "LINK / ENDPOINT",
        "Connected",
        "Active · Headphones",
        GOOD,
    );

    // Two content cards.
    card(
        &mut c,
        282,
        386,
        436,
        218,
        "Negotiation status",
        "Evidence, not preference",
    );
    c.circle(306, 440, 4, MUTED);
    text(&mut c, 320, 430, "No active codec observed", TEXT, 2);
    text(
        &mut c,
        306,
        464,
        "Windows does not report it through a normal API.",
        MUTED,
        1,
    );
    text(
        &mut c,
        306,
        484,
        "Play audio, then verify the live codec.",
        MUTED,
        1,
    );
    c.line(306, 508, 694, 508, BORDER);
    kv(&mut c, 306, 522, "A2DP profile", "Advertised");
    kv(&mut c, 306, 548, "Hands-Free", "Advertised");
    kv(&mut c, 306, 574, "Endpoint matching", "Name heuristic");

    card(&mut c, 734, 386, 436, 218, "Audio path", "Separate layers");
    kv(
        &mut c,
        758,
        438,
        "Output endpoint",
        "Studio Headphones · output",
    );
    kv(&mut c, 758, 464, "State", "Active");
    kv(
        &mut c,
        758,
        490,
        "Engine mix",
        "48.0 kHz · 2 ch · 24 bit float",
    );
    kv(&mut c, 758, 516, "Engine PCM rate", "2304 kbit/s");
    text(
        &mut c,
        758,
        548,
        "A microphone endpoint exists; opening it",
        MUTED,
        1,
    );
    text(&mut c, 758, 568, "can switch A2DP to HFP.", MUTED, 1);

    // Capability strip.
    c.round_rect(282, 622, 888, 118, 12, PANEL);
    text(&mut c, 306, 638, "What Windows exposes", TEXT, 2);
    text(
        &mut c,
        306,
        662,
        "No documented A2DP preference or codec-registration API.",
        MUTED,
        1,
    );
    c.bar(306, 690, 112, 22, [27, 55, 46]);
    text(&mut c, 316, 694, "SBC", GOOD, 1);
    c.bar(426, 690, 156, 22, [27, 55, 46]);
    text(&mut c, 436, 694, "AAC on Windows 11", GOOD, 1);
    c.bar(590, 690, 126, 22, [48, 43, 31]);
    text(&mut c, 600, 694, "aptX Adaptive", WARN, 1);
    c.bar(724, 690, 204, 22, [57, 34, 38]);
    text(&mut c, 734, 694, "LDAC / aptX HD: not in-box", BAD, 1);
    c.bar(936, 690, 210, 22, [42, 38, 62]);
    text(
        &mut c,
        946,
        694,
        "LE Audio: vendor ACX path",
        [180, 160, 255],
        1,
    );
    c.image
}

pub fn render_codecs() -> RgbImage {
    let mut c = Canvas::new();
    c.rect(0, 0, W as i32, 60, BG);
    glyph_logo(&mut c, 30, 18, 24);
    text(&mut c, 64, 19, "bluu", TEXT, 2);
    nav_codec(&mut c, 500);
    c.rect(0, 60, 250, H as i32 - 60, BG);
    c.rect(249, 60, 1, H as i32 - 60, [29, 36, 47]);
    text(&mut c, 20, 92, "DEVICE", FAINT, 1);
    device_item(&mut c, 16, 112, "Studio Headphones", true, true);
    text(&mut c, 282, 90, "CODECS", ACCENT, 1);
    text(
        &mut c,
        282,
        108,
        "Supported, negotiated, verified — kept separate",
        TEXT,
        2,
    );
    text(
        &mut c,
        282,
        142,
        "A codec can be in the Windows inventory without being active.",
        MUTED,
        1,
    );

    let rows = [
        (
            "AAC",
            "Supported by this Windows stack",
            ACCENT,
            "Not observed",
        ),
        (
            "aptX Classic",
            "Supported by this Windows stack",
            ACCENT,
            "Not observed",
        ),
        (
            "SBC",
            "Supported by this Windows stack",
            ACCENT,
            "Fallback codec",
        ),
        (
            "aptX Adaptive",
            "Select Qualcomm hardware only",
            WARN,
            "Not generic",
        ),
        (
            "aptX HD",
            "Not available through this backend",
            BAD,
            "Needs vendor stack",
        ),
        (
            "LDAC",
            "Not available through this backend",
            BAD,
            "Sony licence required",
        ),
        (
            "LC3",
            "LE Audio · separate transport",
            [180, 160, 255],
            "Vendor ACX path",
        ),
    ];
    let mut y = 176;
    for (name, state, color, detail) in rows {
        c.round_rect(282, y, 888, 58, 10, PANEL);
        c.circle(306, y + 29, 4, color);
        text(&mut c, 322, y + 17, name, TEXT, 2);
        c.bar(470, y + 20, 250, 20, [30, 37, 49]);
        text(&mut c, 482, y + 23, state, color, 1);
        text(&mut c, 800, y + 22, detail, FAINT, 1);
        y += 68;
    }
    c.round_rect(282, 676, 888, 62, 10, RAISED);
    text(
        &mut c,
        306,
        692,
        "A supported codec is never displayed as active.",
        TEXT,
        1,
    );
    text(
        &mut c,
        306,
        712,
        "Verification requires an independent A2DP stream observation and evidence labels.",
        MUTED,
        1,
    );
    c.image
}

fn card(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, title: &str, trailing: &str) {
    c.round_rect(x, y, w, h, 12, PANEL);
    text(c, x + 22, y + 18, title, TEXT, 2);
    text(c, x + w - 168, y + 21, trailing, FAINT, 1);
}

fn kv(c: &mut Canvas, x: i32, y: i32, key: &str, value: &str) {
    text(c, x, y, key, MUTED, 1);
    text(c, x + 290, y, value, TEXT, 1);
}

fn metric(c: &mut Canvas, x: i32, y: i32, label: &str, value: &str, sub: &str, color: [u8; 3]) {
    text(c, x, y, label, FAINT, 1);
    text(c, x, y + 16, value, color, 2);
    text(c, x, y + 42, sub, MUTED, 1);
}

fn nav(c: &mut Canvas, x: i32) {
    c.bar(x, 15, 62, 30, RAISED);
    text(c, x + 20, 22, "Home", TEXT, 1);
    text(c, x + 78, 22, "Devices", MUTED, 1);
    text(c, x + 156, 22, "Codecs", MUTED, 1);
    text(c, x + 230, 22, "Diagnostics", MUTED, 1);
    text(c, x + 332, 22, "Settings", MUTED, 1);
}

fn nav_codec(c: &mut Canvas, x: i32) {
    c.bar(x, 15, 62, 30, RAISED);
    text(c, x + 20, 22, "Home", MUTED, 1);
    text(c, x + 78, 22, "Devices", MUTED, 1);
    text(c, x + 156, 22, "Codecs", TEXT, 1);
    text(c, x + 230, 22, "Diagnostics", MUTED, 1);
    text(c, x + 332, 22, "Settings", MUTED, 1);
}

fn device_item(c: &mut Canvas, x: i32, y: i32, name: &str, connected: bool, selected: bool) {
    if selected {
        c.round_rect(x, y, 218, 60, 9, RAISED);
    }
    c.circle(x + 14, y + 25, 3, if connected { GOOD } else { FAINT });
    text(
        c,
        x + 26,
        y + 17,
        name,
        if selected { TEXT } else { MUTED },
        1,
    );
    text(
        c,
        x + 26,
        y + 37,
        if connected {
            "Connected"
        } else {
            "Paired · disconnected"
        },
        FAINT,
        1,
    );
}

fn glyph_logo(c: &mut Canvas, x: i32, y: i32, size: i32) {
    c.circle(x + size / 2, y + size / 2, size / 2, ACCENT_DIM);
    c.circle(x + size / 2, y + size / 2, size / 2 - 1, ACCENT_DIM);
    // Little audio-link glyph.
    c.circle(x + size / 2 - 5, y + size / 2, 2, ACCENT);
    c.circle(x + size / 2 + 5, y + size / 2, 2, GOOD);
    c.line(
        x + size / 2 - 3,
        y + size / 2,
        x + size / 2 + 3,
        y + size / 2,
        TEXT,
    );
}

// A compact 5x7 bitmap font keeps the headless renderer dependency-free. Uppercase,
// digits and punctuation used in the shots are enough; unknown characters render blank.
fn text(c: &mut Canvas, x: i32, y: i32, s: &str, color: [u8; 3], scale: i32) {
    let mut cx = x;
    for ch in s.chars() {
        if ch == ' ' {
            cx += 4 * scale;
            continue;
        }
        let bits = glyph(ch);
        for (row, line) in bits.iter().enumerate() {
            for col in 0..5 {
                if line & (1 << (4 - col)) != 0 {
                    c.rect(
                        cx + col * scale,
                        y + row as i32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cx += 6 * scale;
    }
}

fn glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1F],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x0A, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x0E, 0x10, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x01, 0x0E],
        '·' => [0, 0, 0, 4, 0, 0, 0],
        '—' | '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        ':' => [0, 4, 0, 0, 4, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0, 4],
        '/' => [0, 1, 2, 4, 8, 16, 0],
        '(' => [2, 4, 8, 8, 8, 4, 2],
        ')' => [8, 4, 2, 2, 2, 4, 8],
        ',' => [0, 0, 0, 0, 4, 4, 8],
        '&' => [0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D],
        _ => [0; 7],
    }
}

pub fn write_png(path: &std::path::Path, codecs: bool) -> Result<(), image::ImageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if codecs {
        render_codecs().save(path)
    } else {
        render_overview().save(path)
    }
}

/// A 256×256 transparent PNG with the two-node Bluetooth audio glyph. Used by the
/// portable package / future installer; generated from code so it can be reproduced.
pub fn render_icon() -> image::RgbaImage {
    let mut img = image::RgbaImage::from_pixel(256, 256, image::Rgba([0, 0, 0, 0]));
    for y in 0..256i32 {
        for x in 0..256i32 {
            let dx = x - 128;
            let dy = y - 128;
            let d2 = dx * dx + dy * dy;
            if d2 <= 112 * 112 {
                img.put_pixel(x as u32, y as u32, image::Rgba([34, 56, 90, 255]));
            }
            if (101 * 101..=108 * 108).contains(&d2) {
                img.put_pixel(x as u32, y as u32, image::Rgba([112, 168, 255, 255]));
            }
        }
    }
    for (cx, cy, r, c) in [
        (95i32, 128i32, 20i32, [112u8, 168, 255, 255]),
        (161, 128, 20, [100, 211, 166, 255]),
    ] {
        for y in (cy - r)..=(cy + r) {
            for x in (cx - r)..=(cx + r) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r * r {
                    img.put_pixel(x as u32, y as u32, image::Rgba(c));
                }
            }
        }
    }
    for x in 105..152 {
        for y in 126..131 {
            img.put_pixel(x as u32, y as u32, image::Rgba([234, 240, 248, 255]));
        }
    }
    img
}

pub fn write_icon(path: &std::path::Path) -> Result<(), image::ImageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    render_icon().save(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn overview_has_expected_dimensions_and_not_blank() {
        let image = super::render_overview();
        assert_eq!(image.dimensions(), (1200, 780));
        let colors = image
            .pixels()
            .map(|p| p.0)
            .collect::<std::collections::HashSet<_>>();
        assert!(colors.len() > 10);
    }
}
