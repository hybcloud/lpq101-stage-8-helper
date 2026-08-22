use anyhow::Result;

use crate::render::{Color, HwndCanvas, Rect, TextAlign, TextStyle};

pub const PANEL_WIDTH: f32 = 540.0;
pub const PANEL_HEIGHT: f32 = 425.0;
pub const TOAST_WIDTH: f32 = 640.0;
pub const TOAST_HEIGHT: f32 = 62.0;

const PREVIOUS_BUTTON: Rect = Rect::new(20.0, 122.0, 165.0, 162.0);
const START_BUTTON: Rect = Rect::new(175.0, 122.0, 365.0, 162.0);
const NEXT_BUTTON: Rect = Rect::new(375.0, 122.0, 520.0, 162.0);
const POSITION_BUTTON: Rect = Rect::new(30.0, 205.0, 250.0, 243.0);
const RESET_BUTTON: Rect = Rect::new(270.0, 205.0, 510.0, 243.0);
const SCALE_BAR: Rect = Rect::new(220.0, 255.0, 510.0, 285.0);
const OPACITY_BAR: Rect = Rect::new(220.0, 299.0, 510.0, 329.0);
const VISIBLE_CHECK: Rect = Rect::new(30.0, 342.0, 220.0, 369.0);

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
}

#[derive(Clone)]
pub struct ControlUi {
    pub state_index: Option<usize>,
    pub state_count: usize,
    pub positioning: bool,
    pub overlay_visible: bool,
    pub scale: u16,
    pub opacity: u8,
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
    canvas.begin(Color::rgb(0x171a21));

    canvas.text(
        "LPQ Stage 8 · Constant-weight Gray code",
        Rect::new(20.0, 15.0, 520.0, 45.0),
        Color::rgb(0xf2f5f8),
        TextStyle::Title,
        TextAlign::Left,
    );

    let progress = match (ui.positioning, ui.state_index) {
        (true, _) => "Grayscale positioning mode · Drag or resize the overlay".to_owned(),
        (false, None) => "Ready · Press Start to copy the initial boxes".to_owned(),
        (false, Some(index)) => format!(
            "State {}/{} · Five occupied boxes",
            index + 1,
            ui.state_count
        ),
    };
    canvas.text(
        &progress,
        Rect::new(20.0, 50.0, 520.0, 72.0),
        Color::rgb(0x72e39f),
        TextStyle::BodyBold,
        TextAlign::Left,
    );

    canvas.text(
        "GRAY Empty",
        Rect::new(20.0, 72.0, 130.0, 92.0),
        Color::rgb(0x9aa0a6),
        TextStyle::Small,
        TextAlign::Left,
    );
    canvas.text(
        "BLUE Stay",
        Rect::new(140.0, 72.0, 245.0, 92.0),
        Color::rgb(0x35bce8),
        TextStyle::Small,
        TextAlign::Left,
    );
    canvas.text(
        "RED Leave",
        Rect::new(260.0, 72.0, 365.0, 92.0),
        Color::rgb(0xff5a5f),
        TextStyle::Small,
        TextAlign::Left,
    );
    canvas.text(
        "GREEN Enter",
        Rect::new(380.0, 72.0, 520.0, 92.0),
        Color::rgb(0x34d17a),
        TextStyle::Small,
        TextAlign::Left,
    );
    canvas.text(
        &ui.instruction,
        Rect::new(20.0, 94.0, 520.0, 116.0),
        Color::rgb(0x99a3b1),
        TextStyle::Small,
        TextAlign::Left,
    );

    draw_button(
        canvas,
        ui,
        ControlHit::Previous,
        PREVIOUS_BUTTON,
        "Previous(PgUp)",
        false,
    );
    draw_button(
        canvas,
        ui,
        ControlHit::Start,
        START_BUTTON,
        if ui.state_index.is_some() {
            "Restart"
        } else {
            "Start & Copy setup"
        },
        true,
    );
    draw_button(
        canvas,
        ui,
        ControlHit::Next,
        NEXT_BUTTON,
        "Next(PgDn)",
        false,
    );

    canvas.stroke_rounded(
        Rect::new(20.0, 182.0, 520.0, 378.0),
        7.0,
        1.0,
        Color::rgb(0x353b48),
    );
    canvas.text(
        "Overlay Settings",
        Rect::new(30.0, 173.0, 180.0, 195.0),
        Color::rgb(0xaeb7c4),
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
    );
    draw_button(
        canvas,
        ui,
        ControlHit::Reset,
        RESET_BUTTON,
        "Reset Position",
        false,
    );

    canvas.text(
        "Overlay Scale",
        Rect::new(30.0, 255.0, 205.0, 285.0),
        Color::rgb(0xeef1f5),
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
        Rect::new(30.0, 299.0, 205.0, 329.0),
        Color::rgb(0xeef1f5),
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

    let checkbox = Rect::new(31.0, 347.0, 47.0, 363.0);
    canvas.fill_rounded(checkbox, 3.0, Color::rgb(0x222731));
    canvas.stroke_rounded(checkbox, 3.0, 1.0, Color::rgb(0x596372));
    if ui.overlay_visible {
        canvas.fill_rounded(
            Rect::new(34.0, 350.0, 44.0, 360.0),
            2.0,
            Color::rgb(0x72e39f),
        );
    }
    canvas.text(
        "Show Overlay",
        Rect::new(55.0, 342.0, 220.0, 369.0),
        Color::rgb(0xeef1f5),
        TextStyle::Body,
        TextAlign::Left,
    );

    canvas.text(
        if ui.hotkeys_enabled {
            "Global hotkeys enabled: PageUp / PageDown work while the game is focused."
        } else {
            "Global hotkeys unavailable; keep this window focused to use PageUp / PageDown."
        },
        Rect::new(20.0, 392.0, 520.0, 418.0),
        Color::rgb(0x99a3b1),
        TextStyle::Small,
        TextAlign::Left,
    );

    canvas.end()
}

pub fn draw_toast(canvas: &HwndCanvas, message: &str) -> Result<()> {
    canvas.begin(Color::rgb(0x20252e));
    canvas.fill_rounded(
        Rect::new(1.0, 1.0, TOAST_WIDTH - 1.0, TOAST_HEIGHT - 1.0),
        8.0,
        Color::rgb(0x20252e),
    );
    canvas.stroke_rounded(
        Rect::new(1.0, 1.0, TOAST_WIDTH - 1.0, TOAST_HEIGHT - 1.0),
        8.0,
        1.0,
        Color::rgb(0x4c5666),
    );
    canvas.text(
        message,
        Rect::new(18.0, 8.0, TOAST_WIDTH - 18.0, TOAST_HEIGHT - 8.0),
        Color::rgb(0xf2f5f8),
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
) {
    let background = if ui.pressed == Some(hit) {
        if primary { 0x175d38 } else { 0x222731 }
    } else if ui.hovered == Some(hit) {
        if primary { 0x278f56 } else { 0x343c49 }
    } else if primary {
        0x1f7a48
    } else {
        0x2a303b
    };
    canvas.fill_rounded(rect, 6.0, Color::rgb(background));
    canvas.stroke_rounded(
        rect,
        6.0,
        1.0,
        Color::rgb(if primary { 0x31a965 } else { 0x424b5a }),
    );
    canvas.text(
        label,
        rect,
        Color::rgb(0xf2f5f8),
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
    canvas.fill_rounded(rect, 5.0, Color::rgb(0x222731));
    let ratio = (value - minimum) as f32 / (maximum - minimum) as f32;
    let fill = Rect::new(
        rect.left,
        rect.top,
        rect.left + (rect.right - rect.left) * ratio,
        rect.bottom,
    );
    canvas.fill_rounded(
        fill,
        5.0,
        Color::rgb(if ui.hovered == Some(hit) {
            0x3f82b7
        } else {
            0x356f9d
        }),
    );
    canvas.stroke_rounded(rect, 5.0, 1.0, Color::rgb(0x424b5a));
    canvas.text(
        &format!("{value}%"),
        rect,
        Color::rgb(0xf2f5f8),
        TextStyle::BodyBold,
        TextAlign::Center,
    );
}
