//! Translucent client rendering.
//!
//! When the window shows Windows acrylic through its client area, classic GDI
//! drawing cannot be used: GDI leaves the alpha channel at zero, so DWM treats
//! its pixels as transparent or additive. Everything visible on the glass is
//! therefore drawn with Direct2D into a 32-bit buffered-paint bitmap with
//! premultiplied alpha, then copied to the window with its alpha intact.
use std::cell::{Cell, RefCell};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_GAMMA_2_2,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_SOFTWARE, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Brush, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_METRICS, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::UI::Controls::{
    BP_PAINTPARAMS, BPBF_TOPDOWNDIB, BPPF_ERASE, BeginBufferedPaint, BufferedPaintInit,
    EndBufferedPaint,
};
use windows::core::w;
use windows_numerics::Vector2;

/// Straight (non-premultiplied) RGBA colour.
#[derive(Clone, Copy)]
pub struct Rgba(pub f32, pub f32, pub f32, pub f32);

pub const fn white(alpha: f32) -> Rgba {
    Rgba(1.0, 1.0, 1.0, alpha)
}

/// Dark wash laid over the acrylic: enough to keep white text legible over a
/// bright desktop while the windows behind stay visible.
pub const WASH: Rgba = Rgba(0.035, 0.04, 0.055, 0.30);
pub const TEXT: Rgba = white(0.96);
pub const TEXT_SECONDARY: Rgba = white(0.66);
pub const CARD: Rgba = white(0.065);
pub const CARD_HOVER: Rgba = white(0.10);
pub const CARD_PRESSED: Rgba = white(0.14);
/// Selected segment: a near-opaque light pill with dark content (the
/// "liquid glass" selected state), unmistakable over any backdrop.
pub const SELECTED: Rgba = Rgba(0.96, 0.97, 1.0, 0.90);
pub const ON_SELECTED: Rgba = Rgba(0.07, 0.08, 0.10, 1.0);
pub const RECORD: Rgba = Rgba(0.894, 0.239, 0.259, 0.96);
pub const RECORD_PRESSED: Rgba = Rgba(0.75, 0.18, 0.2, 0.98);
pub const LIVE: Rgba = Rgba(0.22, 0.78, 0.45, 1.0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Cached text formats keyed by (size × 100, weight, alignment).
type Formats = Vec<((u32, u16, u8), IDWriteTextFormat)>;

struct Renderer {
    _factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    dwrite: IDWriteFactory,
    formats: Formats,
}

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) };
}

/// True while the client area shows acrylic and must be drawn by this module.
pub fn enabled() -> bool {
    ENABLED.with(Cell::get)
}

pub fn set_enabled(on: bool) {
    ENABLED.with(|cell| cell.set(on));
}

fn create() -> windows::core::Result<Renderer> {
    unsafe {
        let _ = BufferedPaintInit();
        let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = factory.CreateDCRenderTarget(&properties)?;
        let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        Ok(Renderer {
            _factory: factory,
            target,
            dwrite,
            formats: Vec::new(),
        })
    }
}

/// Initialises Direct2D; returns false when it is unavailable, in which case
/// the caller keeps the opaque GDI interface.
pub fn available() -> bool {
    RENDERER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = create().ok();
        }
        slot.is_some()
    })
}

/// Releases cached Direct2D resources (window destruction).
pub fn shutdown() {
    RENDERER.with(|cell| cell.borrow_mut().take());
}

pub struct Canvas<'a> {
    target: &'a ID2D1DCRenderTarget,
    dwrite: &'a IDWriteFactory,
    formats: RefCell<&'a mut Formats>,
    dx: Cell<f32>,
    dy: Cell<f32>,
    pub scale: f32,
}

impl Canvas<'_> {
    /// Logical (96-DPI) length to device pixels.
    pub fn k(&self, n: f32) -> f32 {
        n * self.scale
    }
    /// Shifts subsequent drawing; used to paint the parent backdrop beneath a
    /// child control in parent coordinates.
    pub fn set_offset(&self, x: f32, y: f32) {
        self.dx.set(x);
        self.dy.set(y);
    }
    fn brush(&self, c: Rgba) -> Option<ID2D1SolidColorBrush> {
        let color = D2D1_COLOR_F {
            r: c.0,
            g: c.1,
            b: c.2,
            a: c.3,
        };
        unsafe { self.target.CreateSolidColorBrush(&color, None).ok() }
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
        D2D_RECT_F {
            left: x + self.dx.get(),
            top: y + self.dy.get(),
            right: x + w + self.dx.get(),
            bottom: y + h + self.dy.get(),
        }
    }
    pub fn fill(&self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
        if let Some(b) = self.brush(c) {
            unsafe { self.target.FillRectangle(&self.rect(x, y, w, h), &b) }
        }
    }
    pub fn fill_round(&self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
        if let Some(b) = self.brush(c) {
            let rr = D2D1_ROUNDED_RECT {
                rect: self.rect(x, y, w, h),
                radiusX: r,
                radiusY: r,
            };
            unsafe { self.target.FillRoundedRectangle(&rr, &b) }
        }
    }
    /// Strokes a rounded rectangle inside the given bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn stroke_round(&self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgba) {
        if let Some(b) = self.brush(c) {
            let half = width / 2.0;
            let rr = D2D1_ROUNDED_RECT {
                rect: self.rect(x + half, y + half, w - width, h - width),
                radiusX: (r - half).max(0.0),
                radiusY: (r - half).max(0.0),
            };
            unsafe { self.target.DrawRoundedRectangle(&rr, &b, width, None) }
        }
    }
    pub fn line(&self, points: &[(f32, f32)], width: f32, c: Rgba) {
        if let Some(b) = self.brush(c) {
            for pair in points.windows(2) {
                let p = |(x, y): (f32, f32)| Vector2 {
                    X: x + self.dx.get(),
                    Y: y + self.dy.get(),
                };
                unsafe {
                    self.target
                        .DrawLine(p(pair[0]), p(pair[1]), &b, width, None)
                }
            }
        }
    }
    pub fn circle(&self, cx: f32, cy: f32, r: f32, c: Rgba) {
        if let Some(b) = self.brush(c) {
            let e = self.ellipse(cx, cy, r);
            unsafe { self.target.FillEllipse(&e, &b) }
        }
    }
    pub fn ring(&self, cx: f32, cy: f32, r: f32, width: f32, c: Rgba) {
        if let Some(b) = self.brush(c) {
            let e = self.ellipse(cx, cy, r);
            unsafe { self.target.DrawEllipse(&e, &b, width, None) }
        }
    }
    fn ellipse(&self, cx: f32, cy: f32, r: f32) -> D2D1_ELLIPSE {
        D2D1_ELLIPSE {
            point: Vector2 {
                X: cx + self.dx.get(),
                Y: cy + self.dy.get(),
            },
            radiusX: r,
            radiusY: r,
        }
    }
    fn format(&self, size: f32, weight: u16, align: Align) -> Option<IDWriteTextFormat> {
        let key = ((size * 100.0) as u32, weight, align as u8);
        let mut formats = self.formats.borrow_mut();
        if let Some((_, f)) = formats.iter().find(|(k, _)| *k == key) {
            return Some(f.clone());
        }
        let format = unsafe {
            let f = self
                .dwrite
                .CreateTextFormat(
                    w!("Segoe UI Variable Text"),
                    None,
                    DWRITE_FONT_WEIGHT(weight as i32),
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("en-us"),
                )
                .ok()?;
            let _ = f.SetTextAlignment(DWRITE_TEXT_ALIGNMENT(match align {
                Align::Left => 0,
                Align::Right => 1,
                Align::Center => 2,
            }));
            let _ = f.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            if let Ok(sign) = self.dwrite.CreateEllipsisTrimmingSign(&f) {
                let trimming = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                let _ = f.SetTrimming(&trimming, &sign);
            }
            f
        };
        formats.push((key, format.clone()));
        Some(format)
    }
    fn gradient(
        &self,
        from: (f32, f32),
        to: (f32, f32),
        stops: &[(f32, Rgba)],
    ) -> Option<ID2D1Brush> {
        let stops: Vec<D2D1_GRADIENT_STOP> = stops
            .iter()
            .map(|(position, c)| D2D1_GRADIENT_STOP {
                position: *position,
                color: D2D1_COLOR_F {
                    r: c.0,
                    g: c.1,
                    b: c.2,
                    a: c.3,
                },
            })
            .collect();
        let p = |(x, y): (f32, f32)| Vector2 {
            X: x + self.dx.get(),
            Y: y + self.dy.get(),
        };
        unsafe {
            let collection = self
                .target
                .CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                .ok()?;
            let properties = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: p(from),
                endPoint: p(to),
            };
            self.target
                .CreateLinearGradientBrush(&properties, None, &collection)
                .ok()
                .map(Into::into)
        }
    }
    /// Glass surface in the liquid-glass manner, without sampling the
    /// backdrop: tinted body, a sheen over the top 45 %, a hairline and a
    /// specular rim lit from the top-left that fades toward the bottom-right.
    #[allow(clippy::too_many_arguments)]
    pub fn glass_card(&self, x: f32, y: f32, w: f32, h: f32, r: f32, body: Rgba, rim: f32) {
        self.fill_round(x, y, w, h, r, body);
        let rr = |inset: f32| D2D1_ROUNDED_RECT {
            rect: self.rect(x + inset, y + inset, w - 2.0 * inset, h - 2.0 * inset),
            radiusX: (r - inset).max(0.0),
            radiusY: (r - inset).max(0.0),
        };
        if let Some(sheen) = self.gradient(
            (x, y),
            (x, y + h),
            &[(0.0, white(0.075)), (0.45, white(0.0))],
        ) {
            unsafe { self.target.FillRoundedRectangle(&rr(0.0), &sheen) }
        }
        let line = self.k(1.0);
        self.stroke_round(x, y, w, h, r, line, white(0.07 * rim));
        if let Some(spec) = self.gradient(
            (x, y),
            (x + w, y + h),
            &[
                (0.0, white(0.55 * rim)),
                (0.25, white(0.20 * rim)),
                (0.5, white(0.05 * rim)),
                (0.8, white(0.04 * rim)),
                (1.0, white(0.12 * rim)),
            ],
        ) {
            unsafe {
                self.target
                    .DrawRoundedRectangle(&rr(line / 2.0), &spec, line, None)
            }
        }
    }
    /// Width of single-line text in device pixels.
    pub fn measure(&self, s: &str, size: f32, weight: u16) -> f32 {
        let Some(format) = self.format(self.k(size), weight, Align::Left) else {
            return 0.0;
        };
        let wide: Vec<u16> = s.encode_utf16().collect();
        unsafe {
            self.dwrite
                .CreateTextLayout(&wide, &format, 100_000.0, 10_000.0)
                .and_then(|layout| {
                    let mut metrics = DWRITE_TEXT_METRICS::default();
                    layout.GetMetrics(&mut metrics).map(|_| metrics.width)
                })
                .unwrap_or(0.0)
        }
    }
    /// Draws single-line text vertically centred in the box. `size` is in
    /// logical points (scaled internally); `weight` is a DirectWrite weight.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &self,
        s: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        size: f32,
        weight: u16,
        align: Align,
        c: Rgba,
    ) {
        let (Some(format), Some(brush)) = (self.format(self.k(size), weight, align), self.brush(c))
        else {
            return;
        };
        let wide: Vec<u16> = s.encode_utf16().collect();
        unsafe {
            self.target.DrawText(
                &wide,
                &format,
                &self.rect(x, y, w, h),
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

/// Paints `width` × `height` device pixels of `dc` (origin 0,0) through a
/// 32-bit premultiplied buffer. The buffer starts fully transparent.
pub fn paint(dc: isize, width: i32, height: i32, dpi: u32, draw: impl FnOnce(&Canvas)) -> bool {
    if width <= 0 || height <= 0 {
        return true;
    }
    RENDERER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = create().ok();
        }
        let Some(renderer) = slot.as_mut() else {
            return false;
        };
        let bounds = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let params = BP_PAINTPARAMS {
            cbSize: std::mem::size_of::<BP_PAINTPARAMS>() as u32,
            dwFlags: BPPF_ERASE,
            prcExclude: std::ptr::null_mut(),
            pBlendFunction: std::ptr::null_mut(),
        };
        let mut memory = HDC::default();
        let buffer = unsafe {
            BeginBufferedPaint(
                HDC(dc as *mut _),
                &bounds,
                BPBF_TOPDOWNDIB,
                Some(&params),
                &mut memory,
            )
        };
        if buffer == 0 {
            return false;
        }
        let mut ok = false;
        unsafe {
            if renderer.target.BindDC(memory, &bounds).is_ok() {
                renderer.target.BeginDraw();
                renderer
                    .target
                    .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
                let canvas = Canvas {
                    target: &renderer.target,
                    dwrite: &renderer.dwrite,
                    formats: RefCell::new(&mut renderer.formats),
                    dx: Cell::new(0.0),
                    dy: Cell::new(0.0),
                    scale: dpi.max(96) as f32 / 96.0,
                };
                draw(&canvas);
                ok = renderer.target.EndDraw(None, None).is_ok();
            }
            let _ = EndBufferedPaint(buffer, ok);
        }
        if !ok {
            // Device loss or a bad DC: rebuild on the next paint.
            *slot = None;
        }
        ok
    })
}
