use anyhow::Result;
use std::{ffi::c_void, mem::size_of, ptr::null_mut};
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, POINT, RECT, SIZE},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED,
                    D2D1_PIXEL_FORMAT,
                },
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_BITMAP_PROPERTIES,
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES,
                D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
                D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
                D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1CreateFactory,
                ID2D1DCRenderTarget, ID2D1Factory, ID2D1HwndRenderTarget,
            },
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, IDWriteFactory,
                IDWriteTextFormat,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
                CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
                GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SelectObject,
            },
        },
        UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow},
    },
    core::w,
};

#[derive(Clone)]
pub struct D2dContext {
    factory: ID2D1Factory,
    dwrite: IDWriteFactory,
}

impl D2dContext {
    pub fn new() -> Result<Self> {
        unsafe {
            Ok(Self {
                factory: D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?,
                dwrite: windows::Win32::Graphics::DirectWrite::DWriteCreateFactory(
                    DWRITE_FACTORY_TYPE_SHARED,
                )?,
            })
        }
    }

    pub fn create_hwnd_canvas(
        &self,
        hwnd: HWND,
        width: u32,
        height: u32,
        dpi: f32,
    ) -> Result<HwndCanvas> {
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: dpi,
            dpiY: dpi,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U { width, height },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let target = unsafe {
            self.factory
                .CreateHwndRenderTarget(&properties, &hwnd_properties)?
        };
        let text = TextFormats::new(&self.dwrite)?;
        Ok(HwndCanvas { target, text })
    }

    pub fn create_layered_canvas(
        &self,
        hwnd: HWND,
        width: u32,
        height: u32,
    ) -> Result<LayeredCanvas> {
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = unsafe { self.factory.CreateDCRenderTarget(&properties)? };
        LayeredCanvas::new(hwnd, target, width, height)
    }
}

pub struct TextFormats {
    pub title: IDWriteTextFormat,
    pub body: IDWriteTextFormat,
    pub body_bold: IDWriteTextFormat,
    pub small: IDWriteTextFormat,
}

impl TextFormats {
    fn new(factory: &IDWriteFactory) -> Result<Self> {
        unsafe fn create(
            factory: &IDWriteFactory,
            size: f32,
            bold: bool,
        ) -> windows::core::Result<IDWriteTextFormat> {
            unsafe {
                factory.CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    if bold {
                        DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("en-US"),
                )
            }
        }

        unsafe {
            Ok(Self {
                title: create(factory, 20.0, true)?,
                body: create(factory, 13.0, false)?,
                body_bold: create(factory, 13.0, true)?,
                small: create(factory, 11.0, false)?,
            })
        }
    }
}

pub struct HwndCanvas {
    target: ID2D1HwndRenderTarget,
    pub text: TextFormats,
}

pub struct LayeredCanvas {
    hwnd: HWND,
    target: ID2D1DCRenderTarget,
    memory_dc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
    width: u32,
    height: u32,
}

impl LayeredCanvas {
    fn new(hwnd: HWND, target: ID2D1DCRenderTarget, width: u32, height: u32) -> Result<Self> {
        let mut canvas = Self {
            hwnd,
            target,
            memory_dc: HDC::default(),
            bitmap: HBITMAP::default(),
            previous_bitmap: HGDIOBJ::default(),
            width: 0,
            height: 0,
        };
        canvas.resize(width, height)?;
        Ok(canvas)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return Ok(());
        }
        self.release_surface();
        self.width = width;
        self.height = height;

        unsafe {
            self.memory_dc = CreateCompatibleDC(None);
            anyhow::ensure!(!self.memory_dc.is_invalid(), "CreateCompatibleDC failed");

            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.width as i32,
                    biHeight: -(self.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = null_mut();
            self.bitmap = CreateDIBSection(
                Some(self.memory_dc),
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )?;
            self.previous_bitmap = SelectObject(self.memory_dc, HGDIOBJ(self.bitmap.0));
            self.target.BindDC(
                self.memory_dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: self.width as i32,
                    bottom: self.height as i32,
                },
            )?;
        }
        Ok(())
    }

    pub fn draw_bgra(
        &self,
        pixels: &[u8],
        source_width: u32,
        source_height: u32,
        opacity: u8,
    ) -> Result<()> {
        anyhow::ensure!(
            pixels.len() == (source_width * source_height * 4) as usize,
            "unexpected overlay pixel buffer size"
        );
        let bitmap_properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };

        unsafe {
            let bitmap = self.target.CreateBitmap(
                D2D_SIZE_U {
                    width: source_width,
                    height: source_height,
                },
                Some(pixels.as_ptr().cast()),
                source_width * 4,
                &bitmap_properties,
            )?;
            self.target.BeginDraw();
            self.target
                .Clear(Some(&Color::rgba(0x000000, 0.0).into_d2d()));
            self.target.DrawBitmap(
                &bitmap,
                Some(&D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: self.width as f32,
                    bottom: self.height as f32,
                }),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
            );
            self.target.EndDraw(None, None)?;

            let screen_dc = GetDC(None);
            anyhow::ensure!(!screen_dc.is_invalid(), "GetDC failed");
            let source = POINT { x: 0, y: 0 };
            let size = SIZE {
                cx: self.width as i32,
                cy: self.height as i32,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: ((opacity as u16 * 255 + 50) / 100).min(255) as u8,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let update_result = UpdateLayeredWindow(
                self.hwnd,
                Some(screen_dc),
                None,
                Some(&size),
                Some(self.memory_dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            ReleaseDC(None, screen_dc);
            update_result?;
        }
        Ok(())
    }

    fn release_surface(&mut self) {
        unsafe {
            if !self.memory_dc.is_invalid() {
                if !self.previous_bitmap.is_invalid() {
                    SelectObject(self.memory_dc, self.previous_bitmap);
                }
                if !self.bitmap.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
                }
                let _ = DeleteDC(self.memory_dc);
            }
        }
        self.memory_dc = HDC::default();
        self.bitmap = HBITMAP::default();
        self.previous_bitmap = HGDIOBJ::default();
    }
}

impl Drop for LayeredCanvas {
    fn drop(&mut self) {
        self.release_surface();
    }
}

impl HwndCanvas {
    pub fn resize(&self, width: u32, height: u32) -> Result<()> {
        unsafe {
            self.target.Resize(&D2D_SIZE_U { width, height })?;
        }
        Ok(())
    }

    pub fn set_dpi(&self, dpi: f32) {
        unsafe { self.target.SetDpi(dpi, dpi) }
    }

    pub fn begin(&self, color: Color) {
        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&color.into_d2d()));
        }
    }

    pub fn end(&self) -> Result<()> {
        unsafe { self.target.EndDraw(None, None)? };
        Ok(())
    }

    pub fn fill_rounded(&self, rect: Rect, radius: f32, color: Color) {
        unsafe {
            let brush = self
                .target
                .CreateSolidColorBrush(&color.into_d2d(), None)
                .expect("create Direct2D brush");
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: rect.into_d2d(),
                    radiusX: radius,
                    radiusY: radius,
                },
                &brush,
            );
        }
    }

    pub fn stroke_rounded(&self, rect: Rect, radius: f32, width: f32, color: Color) {
        unsafe {
            let brush = self
                .target
                .CreateSolidColorBrush(&color.into_d2d(), None)
                .expect("create Direct2D brush");
            self.target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: rect.into_d2d(),
                    radiusX: radius,
                    radiusY: radius,
                },
                &brush,
                width,
                None,
            );
        }
    }

    pub fn text(&self, text: &str, rect: Rect, color: Color, style: TextStyle, align: TextAlign) {
        let wide = text.encode_utf16().collect::<Vec<_>>();
        let format = match style {
            TextStyle::Title => &self.text.title,
            TextStyle::Body => &self.text.body,
            TextStyle::BodyBold => &self.text.body_bold,
            TextStyle::Small => &self.text.small,
        };
        unsafe {
            let _ = format.SetTextAlignment(match align {
                TextAlign::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                TextAlign::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            });
            let _ = format.SetParagraphAlignment(match align {
                TextAlign::Left => DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                TextAlign::Center => DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            });
            let brush = self
                .target
                .CreateSolidColorBrush(&color.into_d2d(), None)
                .expect("create Direct2D brush");
            self.target.DrawText(
                &wide,
                format,
                &rect.into_d2d(),
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

#[derive(Clone, Copy)]
pub enum TextStyle {
    Title,
    Body,
    BodyBold,
    Small,
}

#[derive(Clone, Copy)]
pub enum TextAlign {
    Left,
    Center,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }

    fn into_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xff) as f32 / 255.0,
            g: ((hex >> 8) & 0xff) as f32 / 255.0,
            b: (hex & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    pub const fn rgba(hex: u32, alpha: f32) -> Self {
        Self {
            a: alpha,
            ..Self::rgb(hex)
        }
    }

    fn into_d2d(self) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
        windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}
