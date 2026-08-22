use anyhow::Result;

use crate::render::{Color, HwndCanvas, Rect, TextAlign, TextStyle};

pub const PANEL_WIDTH: f32 = 540.0;
pub const PANEL_HEIGHT: f32 = 500.0;
pub const TOAST_WIDTH: f32 = 640.0;
pub const TOAST_HEIGHT: f32 = 62.0;

const PREVIOUS_BUTTON: Rect = Rect::new(26.0, 190.0, 172.0, 234.0);
const START_BUTTON: Rect = Rect::new(182.0, 190.0, 358.0, 234.0);
const NEXT_BUTTON: Rect = Rect::new(368.0, 190.0, 514.0, 234.0);
const POSITION_BUTTON: Rect = Rect::new(30.0, 278.0, 250.0, 316.0);
const RESET_BUTTON: Rect = Rect::new(270.0, 278.0, 510.0, 316.0);
const SCALE_BAR: Rect = Rect::new(214.0, 329.0, 510.0, 357.0);
const OPACITY_BAR: Rect = Rect::new(214.0, 368.0, 510.0, 396.0);
const VISIBLE_CHECK: Rect = Rect::new(30.0, 403.0, 170.0, 430.0);
const HOST_BUTTON: Rect = Rect::new(180.0, 401.0, 270.0, 432.0);
const ROOM_CODE_FIELD: Rect = Rect::new(280.0, 401.0, 405.0, 432.0);
const JOIN_BUTTON: Rect = Rect::new(415.0, 401.0, 510.0, 432.0);

const PAGE: Color = Color::rgb(0xe8cf9b);
const PAPER: Color = Color::rgb(0xf8edcf);
const INK: Color = Color::rgb(0x3e291e);
const TEXT: Color = Color::rgb(0x523d2d);
const MUTED: Color = Color::rgb(0x765d43);
const FAINT: Color = Color::rgb(0x927653);
const BORDER: Color = Color::rgb(0xb99b67);
const SURFACE: Color = Color::rgb(0xe6d2a6);
const ACCENT: Color = Color::rgb(0x8d402d);
const ACCENT_HOVER: Color = Color::rgb(0xa34d35);
const ACCENT_PRESSED: Color = Color::rgb(0x773522);
const ACCENT_TEXT: Color = Color::rgb(0xfff2d2);
const BLUE: Color = Color::rgb(0x2c6d78);
const RED: Color = Color::rgb(0xad4635);
const GREEN: Color = Color::rgb(0x657d32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlHit {
    Previous,
    Start,
    Next,
    Position,
    Reset,
    Scale,
    Opacity,
    Visible,
    Host,
    RoomCode,
    Join,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineMode {
    Offline,
    Host,
    Viewer,
}

#[derive(Clone)]
pub struct ControlUi {
    pub state_index: Option<usize>,
    pub state_count: usize,
    pub positioning: bool,
    pub overlay_visible: bool,
    pub scale: u16,
    pub opacity: u8,
    pub online_mode: OnlineMode,
    pub online_connecting: bool,
    pub room_code: Option<String>,
    pub room_code_input: String,
    pub room_code_focused: bool,
    pub online_status: String,
    pub instruction: String,
    pub hotkeys_enabled: bool,
    pub cursor: (f32, f32),
    pub hovered: Option<ControlHit>,
    pub pressed: Option<ControlHit>,
}

impl ControlUi {
    pub fn hit_test(&self, x: f32, y: f32) -> Option<ControlHit> {
        [
            (ControlHit::Previous, PREVIOUS_BUTTON),
            (ControlHit::Start, START_BUTTON),
            (ControlHit::Next, NEXT_BUTTON),
            (ControlHit::Position, POSITION_BUTTON),
            (ControlHit::Reset, RESET_BUTTON),
            (ControlHit::Scale, SCALE_BAR),
            (ControlHit::Opacity, OPACITY_BAR),
            (ControlHit::Visible, VISIBLE_CHECK),
            (ControlHit::Host, HOST_BUTTON),
            (ControlHit::RoomCode, ROOM_CODE_FIELD),
            (ControlHit::Join, JOIN_BUTTON),
        ]
        .into_iter()
        .find_map(|(hit, rect)| rect.contains(x, y).then_some(hit))
    }

    pub fn scale_from_x(x: f32) -> u16 {
        slider_value(x, SCALE_BAR, 50, 300) as u16
    }

    pub fn opacity_from_x(x: f32) -> u8 {
        slider_value(x, OPACITY_BAR, 15, 100) as u8
    }
}

fn slider_value(x: f32, rect: Rect, minimum: i32, maximum: i32) -> i32 {
    let ratio = ((x - rect.left) / (rect.right - rect.left)).clamp(0.0, 1.0);
    (minimum as f32 + ratio * (maximum - minimum) as f32).round() as i32
}

pub fn draw_control(canvas: &HwndCanvas, ui: &ControlUi) -> Result<()> {
    let viewer_mode = ui.online_mode == OnlineMode::Viewer;
    canvas.begin(PAGE);
    canvas.fill_rounded(
        Rect::new(12.0, 12.0, PANEL_WIDTH - 12.0, PANEL_HEIGHT - 12.0),
        18.0,
        PAPER,
    );
    canvas.stroke_rounded(
        Rect::new(12.5, 12.5, PANEL_WIDTH - 12.5, PANEL_HEIGHT - 12.5),
        18.0,
        1.0,
        BORDER,
    );

    canvas.text(
        "Ludibrium Party Quest",
        Rect::new(28.0, 26.0, 512.0, 48.0),
        ACCENT,
        TextStyle::BodyBold,
        TextAlign::Left,
    );
    canvas.text(
        "Stage 8 Helper",
        Rect::new(28.0, 48.0, 512.0, 84.0),
        INK,
        TextStyle::Title,
        TextAlign::Left,
    );

    let progress = match (ui.positioning, ui.state_index) {
        (true, _) => "Positioning · Drag to move; resize or use Scale to fit".to_owned(),
        (false, None) => "Ready · Press Start to copy the initial setup".to_owned(),
        (false, Some(index)) => format!(
            "State {} / {} · Five occupied boxes",
            index + 1,
            ui.state_count
        ),
    };
    canvas.text(
        &progress,
        Rect::new(28.0, 91.0, 512.0, 113.0),
        if ui.positioning { ACCENT } else { GREEN },
        TextStyle::BodyBold,
        TextAlign::Left,
    );

    canvas.text(
        "● Empty",
        Rect::new(28.0, 116.0, 128.0, 136.0),
        MUTED,
        TextStyle::Small,
        TextAlign::Left,
    );
    canvas.text(
        "● Occupied",
        Rect::new(135.0, 116.0, 245.0, 136.0),
        BLUE,
        TextStyle::Small,
        TextAlign::Left,
    );
    canvas.text(
        "● Leave",
        Rect::new(257.0, 116.0, 355.0, 136.0),
        RED,
        TextStyle::Small,
        TextAlign::Left,
    );
    canvas.text(
        "● Enter",
        Rect::new(370.0, 116.0, 512.0, 136.0),
        GREEN,
        TextStyle::Small,
        TextAlign::Left,
    );

    canvas.fill_rounded(Rect::new(26.0, 140.0, 514.0, 180.0), 8.0, SURFACE);
    canvas.stroke_rounded(Rect::new(26.5, 140.5, 513.5, 179.5), 8.0, 1.0, BORDER);
    canvas.text(
        &ui.instruction,
        Rect::new(38.0, 149.0, 502.0, 175.0),
        TEXT,
        TextStyle::Small,
        TextAlign::Left,
    );

    draw_button(
        canvas,
        ui,
        ControlHit::Previous,
        PREVIOUS_BUTTON,
        "← Previous · PgUp",
        false,
        !viewer_mode,
    );
    draw_button(
        canvas,
        ui,
        ControlHit::Start,
        START_BUTTON,
        if viewer_mode {
            "Viewer Mode"
        } else if ui.state_index.is_some() {
            "Reset"
        } else {
            "Start & Copy"
        },
        true,
        !viewer_mode,
    );
    draw_button(
        canvas,
        ui,
        ControlHit::Next,
        NEXT_BUTTON,
        "Next · PgDn →",
        false,
        !viewer_mode,
    );

    canvas.fill_rounded(Rect::new(20.0, 246.0, 520.0, 438.0), 10.0, PAPER);
    canvas.stroke_rounded(Rect::new(20.5, 246.5, 519.5, 437.5), 10.0, 1.0, BORDER);
    canvas.text(
        "OVERLAY SETTINGS",
        Rect::new(30.0, 255.0, 190.0, 275.0),
        FAINT,
        TextStyle::Small,
        TextAlign::Left,
    );

    draw_button(
        canvas,
        ui,
        ControlHit::Position,
        POSITION_BUTTON,
        if ui.positioning {
            "Finish Positioning"
        } else {
            "Reposition"
        },
        false,
        true,
    );
    draw_button(
        canvas,
        ui,
        ControlHit::Reset,
        RESET_BUTTON,
        "Reset Position",
        false,
        true,
    );

    canvas.text(
        "Overlay Scale",
        Rect::new(30.0, 329.0, 198.0, 357.0),
        TEXT,
        TextStyle::Body,
        TextAlign::Left,
    );
    draw_value_bar(
        canvas,
        ui,
        ControlHit::Scale,
        SCALE_BAR,
        ui.scale as i32,
        50,
        300,
    );

    canvas.text(
        "Image Opacity",
        Rect::new(30.0, 368.0, 198.0, 396.0),
        TEXT,
        TextStyle::Body,
        TextAlign::Left,
    );
    draw_value_bar(
        canvas,
        ui,
        ControlHit::Opacity,
        OPACITY_BAR,
        ui.opacity as i32,
        15,
        100,
    );

    let checkbox = Rect::new(31.0, 408.0, 47.0, 424.0);
    canvas.fill_rounded(checkbox, 3.0, SURFACE);
    canvas.stroke_rounded(checkbox, 3.0, 1.0, BORDER);
    if ui.overlay_visible {
        canvas.fill_rounded(Rect::new(34.0, 411.0, 44.0, 421.0), 2.0, GREEN);
    }
    canvas.text(
        "Show Overlay",
        Rect::new(55.0, 403.0, 170.0, 430.0),
        TEXT,
        TextStyle::Body,
        TextAlign::Left,
    );

    let online_active = ui.online_connecting || ui.online_mode != OnlineMode::Offline;
    draw_button(
        canvas,
        ui,
        ControlHit::Host,
        HOST_BUTTON,
        if online_active { "Leave" } else { "Host" },
        false,
        true,
    );

    let displayed_code = ui.room_code.as_ref().unwrap_or(&ui.room_code_input);
    canvas.fill_rounded(ROOM_CODE_FIELD, 7.0, Color::rgb(0xead5a5));
    canvas.stroke_rounded(
        ROOM_CODE_FIELD,
        7.0,
        if ui.room_code_focused && !online_active {
            2.0
        } else {
            1.0
        },
        if ui.room_code_focused && !online_active {
            ACCENT
        } else {
            BORDER
        },
    );
    canvas.text(
        if displayed_code.is_empty() {
            "CODE"
        } else {
            displayed_code
        },
        ROOM_CODE_FIELD,
        if displayed_code.is_empty() {
            FAINT
        } else {
            TEXT
        },
        TextStyle::BodyBold,
        TextAlign::Center,
    );

    if ui.online_mode == OnlineMode::Host && !ui.online_connecting {
        draw_button(
            canvas,
            ui,
            ControlHit::Join,
            JOIN_BUTTON,
            "Copy Invite",
            false,
            true,
        );
    } else if ui.online_mode == OnlineMode::Viewer && !ui.online_connecting {
        draw_button(
            canvas,
            ui,
            ControlHit::Join,
            JOIN_BUTTON,
            "Viewer",
            false,
            false,
        );
    } else {
        draw_button(
            canvas,
            ui,
            ControlHit::Join,
            JOIN_BUTTON,
            if ui.online_connecting {
                "Wait…"
            } else {
                "Join"
            },
            false,
            !ui.online_connecting && ui.room_code_input.len() == 4,
        );
    }

    canvas.text(
        &ui.online_status,
        Rect::new(28.0, 446.0, 512.0, 466.0),
        if online_active { GREEN } else { MUTED },
        TextStyle::Small,
        TextAlign::Left,
    );

    canvas.text(
        if viewer_mode {
            "Viewer mode: step controls and PageUp / PageDown are disabled."
        } else if ui.hotkeys_enabled {
            "Global hotkeys enabled: PageUp / PageDown work while the game is focused."
        } else {
            "Global hotkeys unavailable; keep this window focused to use PageUp / PageDown."
        },
        Rect::new(28.0, 468.0, 512.0, 488.0),
        MUTED,
        TextStyle::Small,
        TextAlign::Left,
    );

    canvas.end()
}

pub fn draw_toast(canvas: &HwndCanvas, message: &str) -> Result<()> {
    canvas.begin(INK);
    canvas.fill_rounded(
        Rect::new(1.0, 1.0, TOAST_WIDTH - 1.0, TOAST_HEIGHT - 1.0),
        8.0,
        INK,
    );
    canvas.stroke_rounded(
        Rect::new(1.0, 1.0, TOAST_WIDTH - 1.0, TOAST_HEIGHT - 1.0),
        8.0,
        1.0,
        Color::rgb(0x8f6b43),
    );
    canvas.text(
        message,
        Rect::new(18.0, 8.0, TOAST_WIDTH - 18.0, TOAST_HEIGHT - 8.0),
        ACCENT_TEXT,
        TextStyle::BodyBold,
        TextAlign::Center,
    );
    canvas.end()
}

fn draw_button(
    canvas: &HwndCanvas,
    ui: &ControlUi,
    hit: ControlHit,
    rect: Rect,
    label: &str,
    primary: bool,
    enabled: bool,
) {
    if !enabled {
        canvas.fill_rounded(rect, 8.0, Color::rgb(0xead9b1));
        canvas.stroke_rounded(rect, 8.0, 1.0, Color::rgb(0xcfb57f));
        canvas.text(
            label,
            rect,
            Color::rgb(0xaa9068),
            TextStyle::BodyBold,
            TextAlign::Center,
        );
        return;
    }
    let background = if ui.pressed == Some(hit) {
        if primary {
            ACCENT_PRESSED
        } else {
            Color::rgb(0xcfad70)
        }
    } else if ui.hovered == Some(hit) {
        if primary {
            ACCENT_HOVER
        } else {
            Color::rgb(0xdac08b)
        }
    } else if primary {
        ACCENT
    } else {
        SURFACE
    };
    canvas.fill_rounded(rect, 8.0, background);
    canvas.stroke_rounded(rect, 8.0, 1.0, if primary { ACCENT } else { BORDER });
    canvas.text(
        label,
        rect,
        if primary { ACCENT_TEXT } else { TEXT },
        TextStyle::BodyBold,
        TextAlign::Center,
    );
}

fn draw_value_bar(
    canvas: &HwndCanvas,
    ui: &ControlUi,
    hit: ControlHit,
    rect: Rect,
    value: i32,
    minimum: i32,
    maximum: i32,
) {
    let track = Rect::new(
        rect.left,
        rect.top + 10.0,
        rect.right - 62.0,
        rect.bottom - 10.0,
    );
    canvas.fill_rounded(track, 4.0, Color::rgb(0xead9b1));
    let ratio = (value - minimum) as f32 / (maximum - minimum) as f32;
    let value_x = track.left + (track.right - track.left) * ratio;
    canvas.fill_rounded(
        Rect::new(track.left, track.top, value_x, track.bottom),
        4.0,
        if ui.hovered == Some(hit) {
            ACCENT_HOVER
        } else {
            ACCENT
        },
    );
    canvas.stroke_rounded(track, 4.0, 1.0, BORDER);
    canvas.fill_rounded(
        Rect::new(
            value_x - 5.0,
            rect.top + 5.0,
            value_x + 5.0,
            rect.bottom - 5.0,
        ),
        4.0,
        if ui.pressed == Some(hit) {
            ACCENT_PRESSED
        } else {
            ACCENT
        },
    );
    canvas.text(
        &format!("{value}%"),
        Rect::new(rect.right - 56.0, rect.top, rect.right, rect.bottom),
        TEXT,
        TextStyle::BodyBold,
        TextAlign::Center,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_scale_slider_keeps_native_fit_range() {
        assert_eq!(ControlUi::scale_from_x(SCALE_BAR.left - 100.0), 50);
        assert_eq!(ControlUi::scale_from_x(SCALE_BAR.right + 100.0), 300);
        assert_eq!(
            ControlUi::scale_from_x((SCALE_BAR.left + SCALE_BAR.right) / 2.0),
            175
        );
    }

    #[test]
    fn native_controls_remain_hit_testable() {
        let ui = ControlUi {
            state_index: None,
            state_count: 126,
            positioning: true,
            overlay_visible: true,
            scale: 100,
            opacity: 72,
            online_mode: OnlineMode::Offline,
            online_connecting: false,
            room_code: None,
            room_code_input: String::new(),
            room_code_focused: false,
            online_status: String::new(),
            instruction: String::new(),
            hotkeys_enabled: true,
            cursor: (0.0, 0.0),
            hovered: None,
            pressed: None,
        };
        assert_eq!(
            ui.hit_test(START_BUTTON.left + 1.0, START_BUTTON.top + 1.0),
            Some(ControlHit::Start)
        );
        assert_eq!(
            ui.hit_test(SCALE_BAR.right - 1.0, SCALE_BAR.top + 1.0),
            Some(ControlHit::Scale)
        );
        assert_eq!(ui.hit_test(5.0, 5.0), None);
    }
}
