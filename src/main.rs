#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("lpq101-stage-8-helper currently supports Windows only");

mod config;
mod control;
mod gray_code;
mod online;
mod overlay;
mod platform;
mod render;

use std::{cell::RefCell, collections::VecDeque, ffi::c_void, mem::size_of};

use anyhow::{Context as _, Result};
use config::SettingsStore;
use control::{
    ControlHit, ControlUi, OnlineMode, PANEL_HEIGHT, PANEL_WIDTH, TOAST_HEIGHT, TOAST_WIDTH,
    draw_control, draw_toast,
};
use online::{OnlineAction, OnlineClient, OnlineEvent, OnlineRole};
use overlay::{OverlayImage, OverlayVisual};
use platform::{GlobalHotkeys, HOTKEY_NEXT, HOTKEY_PREVIOUS, copy_to_clipboard};
use render::{D2dContext, HwndCanvas, LayeredCanvas};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST,
            MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
            PAINTSTRUCT,
        },
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            HiDpi::{
                AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
            },
            Input::KeyboardAndMouse::{
                ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_NEXT,
                VK_PRIOR,
            },
            WindowsAndMessaging::{
                CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW,
                DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetClientRect,
                GetCursorPos, GetMessageW, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST,
                IDC_ARROW, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
                IsWindowVisible, KillTimer, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW,
                SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE,
                SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetCursor, SetTimer, SetWindowLongPtrW,
                SetWindowPos, ShowWindow, TranslateMessage, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE,
                WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDOWN,
                WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOVE, WM_NCCREATE, WM_PAINT, WM_SIZE, WM_TIMER,
                WNDCLASSEXW, WS_CAPTION, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_POPUP, WS_SYSMENU,
            },
        },
    },
    core::{Error, w},
};

const WINDOW_CLASS: windows::core::PCWSTR = w!("Lpq101Stage8HelperWindow");
const WM_MOUSELEAVE_NATIVE: u32 = 0x02a3;
const TOAST_TIMER: usize = 0x4a4d_5401;
const ONLINE_TIMER: usize = 0x4a4d_5402;

type EventQueue = RefCell<VecDeque<AppEvent>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowKind {
    Control,
    Overlay,
    Toast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResizeDirection {
    East,
    North,
    NorthEast,
    NorthWest,
    South,
    SouthEast,
    SouthWest,
    West,
}

#[derive(Clone, Copy)]
enum AppEvent {
    Paint(WindowKind),
    Resized(WindowKind, u32, u32),
    Moved(WindowKind),
    DpiChanged(WindowKind, u32, RECT),
    MouseMoved(WindowKind, i32, i32),
    MouseLeft(WindowKind),
    LeftButton(WindowKind, bool, i32, i32),
    CaptureChanged(WindowKind),
    Hotkey(i32),
    KeyDown(u32),
    Character(u16),
    Timer(WindowKind, usize),
    Close(WindowKind),
}

struct WindowContext {
    events: *const EventQueue,
    kind: WindowKind,
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        return LRESULT(1);
    }

    let context = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowContext };
    if context.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let kind = unsafe { (*context).kind };

    let enqueue = |event| unsafe {
        (*(*context).events).borrow_mut().push_back(event);
    };

    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            unsafe {
                BeginPaint(hwnd, &mut paint);
                let _ = EndPaint(hwnd, &paint);
            }
            enqueue(AppEvent::Paint(kind));
            LRESULT(0)
        }
        WM_SIZE => {
            let packed = lparam.0 as u32;
            enqueue(AppEvent::Resized(
                kind,
                packed & 0xffff,
                (packed >> 16) & 0xffff,
            ));
            LRESULT(0)
        }
        WM_MOVE => {
            enqueue(AppEvent::Moved(kind));
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let dpi = (wparam.0 as u32) & 0xffff;
            let suggested = unsafe { *(lparam.0 as *const RECT) };
            enqueue(AppEvent::DpiChanged(kind, dpi, suggested));
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = client_point(lparam);
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            unsafe {
                let _ = TrackMouseEvent(&mut tracking);
            }
            enqueue(AppEvent::MouseMoved(kind, x, y));
            LRESULT(0)
        }
        WM_MOUSELEAVE_NATIVE => {
            enqueue(AppEvent::MouseLeft(kind));
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP => {
            let (x, y) = client_point(lparam);
            enqueue(AppEvent::LeftButton(kind, message == WM_LBUTTONDOWN, x, y));
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            enqueue(AppEvent::CaptureChanged(kind));
            LRESULT(0)
        }
        WM_HOTKEY => {
            enqueue(AppEvent::Hotkey(wparam.0 as i32));
            LRESULT(0)
        }
        WM_KEYDOWN => {
            enqueue(AppEvent::KeyDown(wparam.0 as u32));
            LRESULT(0)
        }
        WM_CHAR => {
            enqueue(AppEvent::Character(wparam.0 as u16));
            LRESULT(0)
        }
        WM_TIMER => {
            enqueue(AppEvent::Timer(kind, wparam.0));
            LRESULT(0)
        }
        WM_CLOSE => {
            enqueue(AppEvent::Close(kind));
            LRESULT(0)
        }
        WM_DESTROY if kind == WindowKind::Control => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn client_point(lparam: LPARAM) -> (i32, i32) {
    let packed = lparam.0 as u32;
    (
        (packed & 0xffff) as i16 as i32,
        ((packed >> 16) & 0xffff) as i16 as i32,
    )
}

struct ControlWindow {
    hwnd: HWND,
    canvas: HwndCanvas,
    dpi: u32,
}

#[derive(Clone, Copy)]
struct OverlayDrag {
    direction: Option<ResizeDirection>,
    cursor: POINT,
    window: RECT,
}

struct OverlayWindow {
    hwnd: HWND,
    canvas: LayeredCanvas,
    image: OverlayImage,
    visual: OverlayVisual,
    scale: u16,
    opacity: u8,
    visible: bool,
    drag: Option<OverlayDrag>,
}

impl OverlayWindow {
    fn physical_size(&self) -> (u32, u32) {
        (
            (self.image.width() * self.scale as u32 / 100).max(1),
            (self.image.height() * self.scale as u32 / 100).max(1),
        )
    }

    fn render(&self) -> Result<()> {
        let pixels = self.image.compose_bgra(self.visual);
        self.canvas.draw_bgra(
            &pixels,
            self.image.width(),
            self.image.height(),
            self.opacity,
        )
    }

    fn set_visible(&mut self, visible: bool) -> Result<()> {
        if self.visible == visible {
            return Ok(());
        }
        unsafe {
            if visible {
                self.render()?;
                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )?;
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                self.render()?;
                anyhow::ensure!(IsWindowVisible(self.hwnd).as_bool(), "show overlay window");
            } else {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_HIDEWINDOW,
                )?;
                anyhow::ensure!(!IsWindowVisible(self.hwnd).as_bool(), "hide overlay window");
            }
        }
        self.visible = visible;
        Ok(())
    }
}

struct ToastWindow {
    hwnd: HWND,
    canvas: HwndCanvas,
    message: String,
}

struct NativeApp {
    module: HINSTANCE,
    d2d: D2dContext,
    control: Option<ControlWindow>,
    overlay: Option<OverlayWindow>,
    toast: Option<ToastWindow>,
    ui: ControlUi,
    states: Vec<gray_code::GrayState>,
    current_movement: Option<gray_code::Movement>,
    smoke_test: bool,
    settings: SettingsStore,
    hotkeys: Option<GlobalHotkeys>,
    online: OnlineClient,
    invite_url: Option<String>,
}

impl NativeApp {
    fn new(module: HINSTANCE) -> Result<Self> {
        let states = gray_code::generate_states();
        let settings = SettingsStore::load();
        Ok(Self {
            module,
            d2d: D2dContext::new()?,
            control: None,
            overlay: None,
            toast: None,
            ui: ControlUi {
                state_index: None,
                state_count: states.len(),
                positioning: true,
                overlay_visible: settings.data.visible,
                scale: settings.data.scale,
                opacity: settings.data.opacity,
                online_mode: OnlineMode::Offline,
                online_connecting: false,
                room_code: None,
                room_code_input: String::new(),
                room_code_focused: false,
                online_status: "Online is optional · Host a room or enter a code to follow.".into(),
                instruction: "Align the overlay, then press Start".into(),
                hotkeys_enabled: false,
                cursor: (0.0, 0.0),
                hovered: None,
                pressed: None,
            },
            states,
            current_movement: None,
            smoke_test: std::env::var_os("LPQ101_STAGE_8_HELPER_SMOKE_TEST").is_some()
                || std::env::var_os("LUDI_PQ_STAGE_8_TOOL_SMOKE_TEST").is_some(),
            settings,
            hotkeys: None,
            online: OnlineClient::new(),
            invite_url: None,
        })
    }

    fn initialize(
        &mut self,
        events: *const EventQueue,
        contexts: &mut Vec<WindowContext>,
    ) -> Result<()> {
        self.create_overlay(events, contexts)?;
        self.create_toast(events, contexts)?;
        self.create_control(events, contexts)?;

        if let Some(control) = &self.control {
            let hotkeys = GlobalHotkeys::register(Some(control.hwnd), !self.smoke_test);
            self.ui.hotkeys_enabled = hotkeys.is_registered();
            self.hotkeys = Some(hotkeys);
            if !self.smoke_test {
                unsafe {
                    let _ = ShowWindow(control.hwnd, SW_SHOW);
                    SetTimer(Some(control.hwnd), ONLINE_TIMER, 200, None);
                }
                self.request_control_redraw();
            }
        }
        self.apply_ui_to_overlay();
        Ok(())
    }

    fn create_control(
        &mut self,
        events: *const EventQueue,
        contexts: &mut Vec<WindowContext>,
    ) -> Result<()> {
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let ex_style = WS_EX_TOPMOST;
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: (PANEL_WIDTH * dpi as f32 / 96.0).round() as i32,
            bottom: (PANEL_HEIGHT * dpi as f32 / 96.0).round() as i32,
        };
        unsafe {
            AdjustWindowRectExForDpi(&mut bounds, style, false, ex_style, dpi)?;
        }
        let context = add_context(contexts, events, WindowKind::Control);
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                WINDOW_CLASS,
                w!("Ludibrium Party Quest · Stage 8 Helper"),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                None,
                None,
                Some(self.module),
                Some(context),
            )?
        };
        let (width, height) = client_size(hwnd)?;
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let canvas = self
            .d2d
            .create_hwnd_canvas(hwnd, width, height, dpi as f32)?;
        self.control = Some(ControlWindow { hwnd, canvas, dpi });
        Ok(())
    }

    fn create_overlay(
        &mut self,
        events: *const EventQueue,
        contexts: &mut Vec<WindowContext>,
    ) -> Result<()> {
        let image = OverlayImage::load()?;
        let width = image.width() * self.ui.scale as u32 / 100;
        let height = image.height() * self.ui.scale as u32 / 100;
        let work = primary_work_area();
        let default_position = centered_position(work, width, height);
        let position = POINT {
            x: self.settings.data.overlay_x.unwrap_or(default_position.x),
            y: self.settings.data.overlay_y.unwrap_or(default_position.y),
        };
        let context = add_context(contexts, events, WindowKind::Overlay);
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                WINDOW_CLASS,
                w!("lpq101-stage-8-helper · Stage 8 Overlay"),
                WS_POPUP,
                position.x,
                position.y,
                width as i32,
                height as i32,
                None,
                None,
                Some(self.module),
                Some(context),
            )?
        };
        let canvas = self.d2d.create_layered_canvas(hwnd, width, height)?;
        let overlay = OverlayWindow {
            hwnd,
            canvas,
            image,
            visual: OverlayVisual::Setup,
            scale: self.ui.scale,
            opacity: self.ui.opacity,
            visible: false,
            drag: None,
        };
        overlay.render()?;
        self.overlay = Some(overlay);
        Ok(())
    }

    fn create_toast(
        &mut self,
        events: *const EventQueue,
        contexts: &mut Vec<WindowContext>,
    ) -> Result<()> {
        let width = TOAST_WIDTH as u32;
        let height = TOAST_HEIGHT as u32;
        let context = add_context(contexts, events, WindowKind::Toast);
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                WINDOW_CLASS,
                w!("lpq101-stage-8-helper · Gray Code Move"),
                WS_POPUP,
                0,
                0,
                width as i32,
                height as i32,
                None,
                None,
                Some(self.module),
                Some(context),
            )?
        };
        let canvas = self.d2d.create_hwnd_canvas(hwnd, width, height, 96.0)?;
        self.toast = Some(ToastWindow {
            hwnd,
            canvas,
            message: String::new(),
        });
        Ok(())
    }

    fn process_events(&mut self, events: &EventQueue) {
        while let Some(event) = { events.borrow_mut().pop_front() } {
            self.process_event(event);
        }
    }

    fn process_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Paint(WindowKind::Control) => {
                if let Some(control) = &self.control {
                    let _ = draw_control(&control.canvas, &self.ui);
                }
            }
            AppEvent::Paint(WindowKind::Overlay) => {
                if let Some(overlay) = &self.overlay {
                    let _ = overlay.render();
                }
            }
            AppEvent::Paint(WindowKind::Toast) => {
                if let Some(toast) = &self.toast {
                    let _ = draw_toast(&toast.canvas, &toast.message);
                }
            }
            AppEvent::Resized(kind, width, height) if width > 0 && height > 0 => {
                self.handle_resize(kind, width, height);
            }
            AppEvent::Moved(WindowKind::Overlay) => self.remember_overlay_position(),
            AppEvent::DpiChanged(WindowKind::Control, dpi, bounds) => {
                if let Some(control) = &mut self.control {
                    control.dpi = dpi.max(96);
                    control.canvas.set_dpi(control.dpi as f32);
                    unsafe {
                        let _ = SetWindowPos(
                            control.hwnd,
                            None,
                            bounds.left,
                            bounds.top,
                            bounds.right - bounds.left,
                            bounds.bottom - bounds.top,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
                self.request_control_redraw();
            }
            AppEvent::MouseMoved(WindowKind::Control, x, y) => self.control_mouse_moved(x, y),
            AppEvent::MouseLeft(WindowKind::Control) => {
                self.ui.hovered = None;
                self.request_control_redraw();
            }
            AppEvent::LeftButton(WindowKind::Control, down, x, y) => {
                self.control_left_button(down, x, y);
            }
            AppEvent::MouseMoved(WindowKind::Overlay, x, y) => self.overlay_mouse_moved(x, y),
            AppEvent::MouseLeft(WindowKind::Overlay) => self.overlay_mouse_left(),
            AppEvent::LeftButton(WindowKind::Overlay, down, x, y) => {
                self.overlay_left_button(down, x, y);
            }
            AppEvent::CaptureChanged(WindowKind::Overlay) => {
                if let Some(overlay) = &mut self.overlay {
                    overlay.drag = None;
                }
            }
            AppEvent::CaptureChanged(WindowKind::Control) => {
                self.ui.pressed = None;
                self.request_control_redraw();
            }
            AppEvent::Hotkey(HOTKEY_PREVIOUS) => self.previous_step(),
            AppEvent::Hotkey(HOTKEY_NEXT) => self.next_step(),
            AppEvent::KeyDown(key) if !self.ui.hotkeys_enabled && key == VK_PRIOR.0 as u32 => {
                self.previous_step();
            }
            AppEvent::KeyDown(key) if !self.ui.hotkeys_enabled && key == VK_NEXT.0 as u32 => {
                self.next_step();
            }
            AppEvent::Character(character) => self.room_code_character(character),
            AppEvent::Timer(WindowKind::Toast, TOAST_TIMER) => self.hide_toast(),
            AppEvent::Timer(WindowKind::Control, ONLINE_TIMER) => self.poll_online(),
            AppEvent::Close(WindowKind::Control) => self.shutdown(),
            _ => {}
        }
    }

    fn handle_resize(&mut self, kind: WindowKind, width: u32, height: u32) {
        match kind {
            WindowKind::Control => {
                if let Some(control) = &self.control {
                    let _ = control.canvas.resize(width, height);
                }
                self.request_control_redraw();
            }
            WindowKind::Overlay => {
                if let Some(overlay) = &mut self.overlay {
                    let _ = overlay.canvas.resize(width, height);
                    let _ = overlay.render();
                    overlay.scale = ((width * 100 + overlay.image.width() / 2)
                        / overlay.image.width())
                    .clamp(50, 300) as u16;
                    self.ui.scale = overlay.scale;
                    self.settings.data.scale = overlay.scale;
                }
                self.request_control_redraw();
            }
            WindowKind::Toast => {
                if let Some(toast) = &self.toast {
                    let _ = toast.canvas.resize(width, height);
                }
            }
        }
    }

    fn control_mouse_moved(&mut self, x: i32, y: i32) {
        let dpi = self.control.as_ref().map_or(96, |control| control.dpi);
        let logical_x = x as f32 * 96.0 / dpi as f32;
        let logical_y = y as f32 * 96.0 / dpi as f32;
        self.ui.cursor = (logical_x, logical_y);
        self.ui.hovered = self.ui.hit_test(logical_x, logical_y);
        if let Some(hit @ (ControlHit::Scale | ControlHit::Opacity)) = self.ui.pressed {
            self.update_slider(hit, logical_x);
        }
        self.request_control_redraw();
    }

    fn control_left_button(&mut self, down: bool, x: i32, y: i32) {
        self.control_mouse_moved(x, y);
        let Some(hwnd) = self.control.as_ref().map(|control| control.hwnd) else {
            return;
        };
        if down {
            unsafe {
                SetCapture(hwnd);
            }
            self.ui.pressed = self.ui.hovered;
            if let Some(hit @ (ControlHit::Scale | ControlHit::Opacity)) = self.ui.pressed {
                self.update_slider(hit, self.ui.cursor.0);
            }
        } else {
            unsafe {
                let _ = ReleaseCapture();
            }
            let pressed = self.ui.pressed.take();
            if let Some(hit) = pressed.filter(|hit| Some(*hit) == self.ui.hovered) {
                self.activate(hit);
            }
        }
        self.request_control_redraw();
    }

    fn overlay_mouse_moved(&mut self, x: i32, y: i32) {
        let Some(overlay) = &mut self.overlay else {
            return;
        };
        if let Some(drag) = overlay.drag {
            let mut cursor = POINT::default();
            if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
                let bounds = drag_bounds(drag, cursor);
                unsafe {
                    let _ = SetWindowPos(
                        overlay.hwnd,
                        None,
                        bounds.left,
                        bounds.top,
                        bounds.right - bounds.left,
                        bounds.bottom - bounds.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            return;
        }

        let (width, height) = client_size(overlay.hwnd).unwrap_or((1, 1));
        let direction = overlay_resize_direction(x, y, width, height);
        set_overlay_cursor(direction);
    }

    fn overlay_mouse_left(&mut self) {
        if self
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.drag.is_none())
        {
            set_overlay_cursor(None);
        }
    }

    fn overlay_left_button(&mut self, down: bool, x: i32, y: i32) {
        let Some(overlay) = &mut self.overlay else {
            return;
        };
        if down {
            let (width, height) = client_size(overlay.hwnd).unwrap_or((1, 1));
            let direction = overlay_resize_direction(x, y, width, height);
            let mut cursor = POINT::default();
            let mut window = RECT::default();
            if unsafe { GetCursorPos(&mut cursor) }.is_ok()
                && unsafe { GetWindowRect(overlay.hwnd, &mut window) }.is_ok()
            {
                overlay.drag = Some(OverlayDrag {
                    direction,
                    cursor,
                    window,
                });
                unsafe {
                    SetCapture(overlay.hwnd);
                }
            }
        } else {
            overlay.drag = None;
            unsafe {
                let _ = ReleaseCapture();
            }
            self.remember_overlay_position();
            self.save_settings();
        }
    }

    fn update_slider(&mut self, hit: ControlHit, x: f32) {
        match hit {
            ControlHit::Scale => self.ui.scale = ControlUi::scale_from_x(x),
            ControlHit::Opacity => self.ui.opacity = ControlUi::opacity_from_x(x),
            _ => {}
        }
        self.apply_ui_to_overlay();
        self.request_control_redraw();
    }

    fn activate(&mut self, hit: ControlHit) {
        if hit != ControlHit::RoomCode {
            self.ui.room_code_focused = false;
        }
        match hit {
            ControlHit::Previous => self.previous_step(),
            ControlHit::Start if self.ui.state_index.is_some() => self.restart_session(),
            ControlHit::Start => self.start_session(),
            ControlHit::Next => self.next_step(),
            ControlHit::Position => self.ui.positioning = !self.ui.positioning,
            ControlHit::Reset => self.reset_overlay_position(),
            ControlHit::Visible => self.ui.overlay_visible = !self.ui.overlay_visible,
            ControlHit::Host
                if self.ui.online_connecting || self.ui.online_mode != OnlineMode::Offline =>
            {
                self.leave_online()
            }
            ControlHit::Host => self.host_online(),
            ControlHit::RoomCode
                if !self.ui.online_connecting && self.ui.online_mode == OnlineMode::Offline =>
            {
                self.ui.room_code_focused = true;
            }
            ControlHit::Join if self.ui.online_mode == OnlineMode::Host => self.copy_invite(),
            ControlHit::Join
                if !self.ui.online_connecting
                    && self.ui.online_mode == OnlineMode::Offline
                    && self.ui.room_code_input.len() == 4 =>
            {
                self.join_online()
            }
            ControlHit::RoomCode | ControlHit::Join => {}
            ControlHit::Scale | ControlHit::Opacity => {}
        }
        self.apply_ui_to_overlay();
        self.save_settings();
        self.request_control_redraw();
    }

    fn apply_ui_to_overlay(&mut self) {
        let visual = match (self.ui.positioning, self.ui.state_index) {
            (true, _) | (_, None) => OverlayVisual::Setup,
            (false, Some(index)) => OverlayVisual::State {
                occupied_mask: self.states[index].occupied_mask(),
                movement: self
                    .current_movement
                    .map(|movement| (movement.from_box, movement.to_box)),
            },
        };
        let Some(overlay) = &mut self.overlay else {
            return;
        };

        if overlay.scale != self.ui.scale {
            overlay.scale = self.ui.scale;
            let (width, height) = overlay.physical_size();
            unsafe {
                let _ = SetWindowPos(
                    overlay.hwnd,
                    None,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            let _ = overlay.canvas.resize(width, height);
        }
        overlay.visual = visual;
        overlay.opacity = self.ui.opacity;
        let _ = overlay.render();

        let should_show = self.ui.overlay_visible && !self.smoke_test;
        let _ = overlay.set_visible(should_show);

        self.settings.data.scale = self.ui.scale;
        self.settings.data.opacity = self.ui.opacity;
        self.settings.data.visible = self.ui.overlay_visible;
    }

    fn start_session(&mut self) {
        if self.ui.online_mode == OnlineMode::Viewer {
            return;
        }
        self.ui.state_index = Some(0);
        self.ui.positioning = false;
        self.current_movement = None;
        self.online.send(OnlineAction::Sync(0));
        self.publish_instruction(gray_code::format_init(self.states[0]));
        self.apply_ui_to_overlay();
        self.request_control_redraw();
    }

    fn restart_session(&mut self) {
        if self.ui.online_mode == OnlineMode::Viewer {
            return;
        }
        self.ui.state_index = None;
        self.ui.positioning = true;
        self.current_movement = None;
        self.ui.instruction = "Align the overlay, then press Start".into();
        self.online.send(OnlineAction::Reset);
        self.apply_ui_to_overlay();
        self.request_control_redraw();
    }

    fn next_step(&mut self) {
        if self.ui.online_mode == OnlineMode::Viewer {
            return;
        }
        let Some(index) = self.ui.state_index else {
            self.start_session();
            return;
        };
        if index + 1 >= self.states.len() {
            return;
        }
        let next = index + 1;
        self.current_movement = Some(self.states[index].movement_to(self.states[next]));
        let instruction = gray_code::format_move(next + 1, self.states[index], self.states[next]);
        self.ui.state_index = Some(next);
        self.online.send(OnlineAction::Next);
        self.publish_instruction(instruction);
        self.apply_ui_to_overlay();
        self.request_control_redraw();
    }

    fn previous_step(&mut self) {
        if self.ui.online_mode == OnlineMode::Viewer {
            return;
        }
        let Some(index) = self.ui.state_index else {
            return;
        };
        if index == 0 {
            self.current_movement = None;
            self.publish_instruction(gray_code::format_init(self.states[0]));
            self.apply_ui_to_overlay();
            self.request_control_redraw();
            return;
        }
        let previous = index - 1;
        self.current_movement = Some(self.states[index].movement_to(self.states[previous]));
        let instruction =
            gray_code::format_move(previous + 1, self.states[index], self.states[previous]);
        self.ui.state_index = Some(previous);
        self.online.send(OnlineAction::Previous);
        self.publish_instruction(instruction);
        self.apply_ui_to_overlay();
        self.request_control_redraw();
    }

    fn publish_instruction(&mut self, instruction: String) {
        self.ui.instruction = instruction.clone();
        let owner = self.control.as_ref().map(|control| control.hwnd);
        let toast = match copy_to_clipboard(owner, &instruction) {
            Ok(()) => format!("Copied · {instruction}"),
            Err(_) => format!("Clipboard unavailable · {instruction}"),
        };
        self.show_toast(toast);
    }

    fn host_online(&mut self) {
        let owner_guid = self
            .settings
            .data
            .owner_guid
            .get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
            .clone();
        self.save_settings();
        self.ui.online_mode = OnlineMode::Host;
        self.ui.online_connecting = true;
        self.ui.room_code = None;
        self.ui.room_code_focused = false;
        self.ui.online_status = "Creating or restoring your room…".into();
        self.invite_url = None;
        self.online.host(
            online_service_url(),
            owner_guid,
            self.ui.state_index.unwrap_or(0),
        );
    }

    fn join_online(&mut self) {
        let code = self.ui.room_code_input.clone();
        self.ui.online_mode = OnlineMode::Viewer;
        self.ui.online_connecting = true;
        self.ui.room_code = Some(code.clone());
        self.ui.room_code_focused = false;
        self.ui.positioning = false;
        self.ui.online_status = format!("Joining room {code} as a viewer…");
        self.invite_url = None;
        self.online.view(online_service_url(), code);
    }

    fn leave_online(&mut self) {
        self.online.disconnect();
        self.ui.online_mode = OnlineMode::Offline;
        self.ui.online_connecting = false;
        self.ui.room_code = None;
        self.ui.online_status =
            "Online is optional · Host a room or enter a code to follow.".into();
        self.invite_url = None;
    }

    fn poll_online(&mut self) {
        let events = self.online.poll().collect::<Vec<_>>();
        if events.is_empty() {
            return;
        }
        for event in events {
            match event {
                OnlineEvent::Connected {
                    role,
                    code,
                    invite_url,
                } => {
                    self.ui.online_connecting = false;
                    self.ui.online_mode = match role {
                        OnlineRole::Host => OnlineMode::Host,
                        OnlineRole::Viewer => OnlineMode::Viewer,
                    };
                    self.ui.room_code = Some(code.clone());
                    self.ui.room_code_input = code.clone();
                    self.invite_url = invite_url.clone();
                    match role {
                        OnlineRole::Host => {
                            self.ui.online_status =
                                format!("Room {code} · Native controls are synced to viewers.");
                            if let Some(invite_url) = invite_url {
                                let owner = self.control.as_ref().map(|control| control.hwnd);
                                let copied = copy_to_clipboard(owner, &invite_url).is_ok();
                                self.show_toast(if copied {
                                    format!("Room {code} · Viewer invite copied")
                                } else {
                                    format!("Room {code} · Clipboard unavailable")
                                });
                            }
                        }
                        OnlineRole::Viewer => {
                            self.ui.online_status =
                                format!("Room {code} · Viewer mode · Controlled by the owner.");
                        }
                    }
                }
                OnlineEvent::State {
                    index,
                    movement,
                    instruction,
                } if self.ui.online_mode == OnlineMode::Viewer && index < self.states.len() => {
                    self.ui.state_index = Some(index);
                    self.current_movement = movement.map(|movement| gray_code::Movement {
                        from_box: movement.from_box,
                        to_box: movement.to_box,
                    });
                    self.ui.instruction = instruction;
                    self.apply_ui_to_overlay();
                }
                OnlineEvent::State { .. } => {}
                OnlineEvent::Error(message) => {
                    self.online.disconnect();
                    self.ui.online_mode = OnlineMode::Offline;
                    self.ui.online_connecting = false;
                    self.ui.room_code = None;
                    self.ui.online_status = message.clone();
                    self.invite_url = None;
                    self.show_toast(message);
                }
            }
        }
        self.request_control_redraw();
    }

    fn copy_invite(&mut self) {
        let Some(invite_url) = self.invite_url.clone() else {
            return;
        };
        let owner = self.control.as_ref().map(|control| control.hwnd);
        let copied = copy_to_clipboard(owner, &invite_url).is_ok();
        self.show_toast(if copied {
            "Viewer invite copied".into()
        } else {
            "Clipboard unavailable".into()
        });
    }

    fn room_code_character(&mut self, character: u16) {
        if !self.ui.room_code_focused
            || self.ui.online_connecting
            || self.ui.online_mode != OnlineMode::Offline
        {
            return;
        }
        match character {
            8 => {
                self.ui.room_code_input.pop();
            }
            13 if self.ui.room_code_input.len() == 4 => self.join_online(),
            _ if self.ui.room_code_input.len() < 4 => {
                let Some(value) = char::from_u32(character as u32) else {
                    return;
                };
                let value = value.to_ascii_uppercase();
                if value.is_ascii_uppercase() || value.is_ascii_digit() {
                    self.ui.room_code_input.push(value);
                }
            }
            _ => {}
        }
        self.request_control_redraw();
    }

    fn show_toast(&mut self, message: String) {
        let overlay_hwnd = self.overlay.as_ref().map(|overlay| overlay.hwnd);
        let Some(toast) = &mut self.toast else {
            return;
        };
        toast.message = message;
        let _ = draw_toast(&toast.canvas, &toast.message);

        let work = overlay_hwnd.map_or_else(primary_work_area, monitor_work_area);
        let width = TOAST_WIDTH as i32;
        let height = TOAST_HEIGHT as i32;
        let x = work.left + ((work.right - work.left - width).max(0) / 2);
        let y = work.top + 48;
        unsafe {
            let _ = SetWindowPos(
                toast.hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = ShowWindow(toast.hwnd, SW_SHOWNOACTIVATE);
            SetTimer(Some(toast.hwnd), TOAST_TIMER, 1350, None);
        }
        let _ = draw_toast(&toast.canvas, &toast.message);
    }

    fn hide_toast(&mut self) {
        if let Some(toast) = &self.toast {
            unsafe {
                let _ = KillTimer(Some(toast.hwnd), TOAST_TIMER);
                let _ = ShowWindow(toast.hwnd, SW_HIDE);
            }
        }
    }

    fn reset_overlay_position(&mut self) {
        if let Some(overlay) = &self.overlay {
            let work = monitor_work_area(overlay.hwnd);
            let (width, height) = overlay.physical_size();
            let position = centered_position(work, width, height);
            unsafe {
                let _ = SetWindowPos(
                    overlay.hwnd,
                    None,
                    position.x,
                    position.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        self.remember_overlay_position();
    }

    fn remember_overlay_position(&mut self) {
        if self.smoke_test {
            return;
        }
        if let Some(overlay) = &self.overlay {
            let mut bounds = RECT::default();
            if unsafe { GetWindowRect(overlay.hwnd, &mut bounds) }.is_ok() {
                self.settings.data.overlay_x = Some(bounds.left);
                self.settings.data.overlay_y = Some(bounds.top);
            }
        }
    }

    fn request_control_redraw(&self) {
        if let Some(control) = &self.control {
            unsafe {
                let _ = InvalidateRect(Some(control.hwnd), None, false);
            }
        }
    }

    fn save_settings(&self) {
        let _ = self.settings.save();
    }

    fn smoke(&mut self) -> Result<()> {
        self.ui.state_index = Some(0);
        self.ui.positioning = false;
        self.apply_ui_to_overlay();
        self.restart_session();
        anyhow::ensure!(
            self.ui.state_index.is_none(),
            "restart returns to initial state"
        );
        anyhow::ensure!(self.ui.positioning, "restart returns to grayscale mode");
        if let Some(control) = &self.control {
            draw_control(&control.canvas, &self.ui)?;
        }
        if let Some(toast) = &self.toast {
            draw_toast(&toast.canvas, "Copied · smoke test")?;
        }
        if let Some(overlay) = &mut self.overlay {
            unsafe {
                SetWindowPos(
                    overlay.hwnd,
                    None,
                    -32_000,
                    -32_000,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )?;
            }
            overlay.set_visible(true)?;
            overlay.set_visible(false)?;
            overlay.set_visible(true)?;
            overlay.set_visible(false)?;
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.remember_overlay_position();
        self.save_settings();
        self.hotkeys.take();
        self.online.disconnect();
        unsafe {
            if let Some(toast) = &self.toast {
                let _ = DestroyWindow(toast.hwnd);
            }
            if let Some(overlay) = &self.overlay {
                let _ = DestroyWindow(overlay.hwnd);
            }
            if let Some(control) = &self.control {
                let _ = KillTimer(Some(control.hwnd), ONLINE_TIMER);
                let _ = DestroyWindow(control.hwnd);
            }
        }
    }
}

fn online_service_url() -> String {
    std::env::var("LPQ_SERVICE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| config::service_origin().to_owned())
}

fn add_context(
    contexts: &mut Vec<WindowContext>,
    events: *const EventQueue,
    kind: WindowKind,
) -> *const c_void {
    assert!(contexts.len() < contexts.capacity());
    contexts.push(WindowContext { events, kind });
    (contexts.last_mut().expect("window context") as *mut WindowContext).cast()
}

fn register_window_class(module: HINSTANCE) -> Result<()> {
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: module,
        hCursor: cursor,
        lpszClassName: WINDOW_CLASS,
        ..Default::default()
    };
    let atom = unsafe { RegisterClassExW(&class) };
    anyhow::ensure!(
        atom != 0,
        "RegisterClassExW failed: {}",
        Error::from_thread()
    );
    Ok(())
}

fn client_size(hwnd: HWND) -> Result<(u32, u32)> {
    let mut bounds = RECT::default();
    unsafe { GetClientRect(hwnd, &mut bounds)? };
    Ok((
        (bounds.right - bounds.left).max(1) as u32,
        (bounds.bottom - bounds.top).max(1) as u32,
    ))
}

fn primary_work_area() -> RECT {
    let monitor = unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) };
    monitor_info(monitor).unwrap_or(RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    })
}

fn centered_position(work: RECT, width: u32, height: u32) -> POINT {
    POINT {
        x: work.left + ((work.right - work.left - width as i32).max(0) / 2),
        y: work.top + ((work.bottom - work.top - height as i32).max(0) / 2),
    }
}

fn monitor_work_area(hwnd: HWND) -> RECT {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    monitor_info(monitor).unwrap_or_else(primary_work_area)
}

fn monitor_info(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> Option<RECT> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
        .then_some(info.rcWork)
}

fn overlay_resize_direction(x: i32, y: i32, width: u32, height: u32) -> Option<ResizeDirection> {
    const MARGIN: i32 = 9;
    let west = x <= MARGIN;
    let east = x >= width as i32 - MARGIN;
    let north = y <= MARGIN;
    let south = y >= height as i32 - MARGIN;
    match (west, east, north, south) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

fn set_overlay_cursor(direction: Option<ResizeDirection>) {
    let resource = match direction {
        Some(ResizeDirection::East | ResizeDirection::West) => IDC_SIZEWE,
        Some(ResizeDirection::North | ResizeDirection::South) => IDC_SIZENS,
        Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast) => IDC_SIZENWSE,
        Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest) => IDC_SIZENESW,
        None => IDC_SIZEALL,
    };
    if let Ok(cursor) = unsafe { LoadCursorW(None, resource) } {
        unsafe {
            SetCursor(Some(cursor));
        }
    }
}

fn drag_bounds(drag: OverlayDrag, cursor: POINT) -> RECT {
    const IMAGE_WIDTH: i32 = 268;
    const IMAGE_HEIGHT: i32 = 119;
    const MIN_WIDTH: i32 = IMAGE_WIDTH / 2;
    const MAX_WIDTH: i32 = IMAGE_WIDTH * 3;
    const MIN_HEIGHT: i32 = (IMAGE_HEIGHT + 1) / 2;
    const MAX_HEIGHT: i32 = IMAGE_HEIGHT * 3;

    let dx = cursor.x - drag.cursor.x;
    let dy = cursor.y - drag.cursor.y;
    let mut result = drag.window;
    let Some(direction) = drag.direction else {
        result.left += dx;
        result.right += dx;
        result.top += dy;
        result.bottom += dy;
        return result;
    };

    let west = matches!(
        direction,
        ResizeDirection::West | ResizeDirection::NorthWest | ResizeDirection::SouthWest
    );
    let east = matches!(
        direction,
        ResizeDirection::East | ResizeDirection::NorthEast | ResizeDirection::SouthEast
    );
    let north = matches!(
        direction,
        ResizeDirection::North | ResizeDirection::NorthEast | ResizeDirection::NorthWest
    );
    let vertical_only = matches!(direction, ResizeDirection::North | ResizeDirection::South);

    let (width, height) = if vertical_only {
        let initial = drag.window.bottom - drag.window.top;
        let height = (initial + if north { -dy } else { dy }).clamp(MIN_HEIGHT, MAX_HEIGHT);
        let width =
            ((height * IMAGE_WIDTH + IMAGE_HEIGHT / 2) / IMAGE_HEIGHT).clamp(MIN_WIDTH, MAX_WIDTH);
        (width, height)
    } else {
        let initial = drag.window.right - drag.window.left;
        let width = (initial + if west { -dx } else { dx }).clamp(MIN_WIDTH, MAX_WIDTH);
        let height =
            ((width * IMAGE_HEIGHT + IMAGE_WIDTH / 2) / IMAGE_WIDTH).clamp(MIN_HEIGHT, MAX_HEIGHT);
        (width, height)
    };

    if west {
        result.left = drag.window.right - width;
        result.right = drag.window.right;
    } else {
        result.left = drag.window.left;
        result.right = drag.window.left + width;
    }
    if north {
        result.top = drag.window.bottom - height;
        result.bottom = drag.window.bottom;
    } else {
        result.top = drag.window.top;
        result.bottom = drag.window.top + height;
    }
    if !west && !east {
        result.left = drag.window.left;
        result.right = drag.window.left + width;
    }
    result
}

fn main() -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }

    let module_handle = unsafe { GetModuleHandleW(None)? };
    let module = HINSTANCE(module_handle.0);
    register_window_class(module)?;

    let events = Box::new(EventQueue::new(VecDeque::new()));
    // The WndProc stores pointers to these contexts, so reserve their final
    // capacity up front and never allow the vector to reallocate.
    let mut contexts = Vec::<WindowContext>::with_capacity(3);
    let mut app = NativeApp::new(module)?;
    app.initialize(events.as_ref(), &mut contexts)?;
    app.process_events(&events);

    if app.smoke_test {
        app.smoke()?;
        app.shutdown();
    } else {
        let mut message = MSG::default();
        loop {
            let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if status.0 == -1 {
                return Err(Error::from_thread()).context("GetMessageW failed");
            }
            if status.0 == 0 {
                break;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            app.process_events(&events);
        }
    }

    drop(app);
    drop(contexts);
    unsafe { CoUninitialize() };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_edges_select_resize_directions() {
        assert_eq!(
            overlay_resize_direction(2, 2, 268, 119),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            overlay_resize_direction(267, 60, 268, 119),
            Some(ResizeDirection::East)
        );
        assert_eq!(overlay_resize_direction(130, 60, 268, 119), None);
    }

    #[test]
    fn overlay_resize_preserves_aspect_ratio() {
        let drag = OverlayDrag {
            direction: Some(ResizeDirection::SouthEast),
            cursor: POINT { x: 100, y: 100 },
            window: RECT {
                left: 50,
                top: 50,
                right: 318,
                bottom: 169,
            },
        };
        let result = drag_bounds(drag, POINT { x: 234, y: 170 });
        assert_eq!(result.right - result.left, 402);
        assert_eq!(result.bottom - result.top, 179);
    }

    #[test]
    fn centered_position_uses_current_overlay_size() {
        let work = RECT {
            left: 100,
            top: 50,
            right: 1100,
            bottom: 650,
        };
        assert_eq!(centered_position(work, 400, 200), POINT { x: 400, y: 250 });
        assert_eq!(centered_position(work, 800, 400), POINT { x: 200, y: 150 });
    }
}
