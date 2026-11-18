use std::{
    cell::RefCell,
    ffi::c_void,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Instant,
};
use windows_capture::monitor::Monitor;

type Handle = isize;
#[repr(C)]
struct SystemTimeWin {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Msg {
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    private: u32,
}
#[repr(C)]
struct WndClass {
    style: u32,
    proc: Option<unsafe extern "system" fn(Handle, u32, usize, isize) -> isize>,
    cls_extra: i32,
    wnd_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu: *const u16,
    class: *const u16,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(c: *const WndClass) -> u16;
    fn CreateWindowExW(
        ex: u32,
        cls: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        param: *const c_void,
    ) -> Handle;
    fn DefWindowProcW(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize;
    fn ShowWindow(hwnd: Handle, how: i32) -> i32;
    fn UpdateWindow(hwnd: Handle) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn IsDialogMessageW(hwnd: Handle, msg: *mut Msg) -> i32;
    fn PostQuitMessage(code: i32);
    fn PostMessageW(hwnd: Handle, msg: u32, w: usize, l: isize) -> i32;
    fn GetDlgItem(hwnd: Handle, id: i32) -> Handle;
    fn GetDlgCtrlID(hwnd: Handle) -> i32;
    fn SetWindowDisplayAffinity(hwnd: Handle, affinity: u32) -> i32;
    fn SetWindowTextW(hwnd: Handle, text: *const u16) -> i32;
    fn GetWindowTextW(hwnd: Handle, text: *mut u16, max: i32) -> i32;
    fn SendMessageW(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize;
    fn EnableWindow(hwnd: Handle, enable: i32) -> i32;
    fn SetTimer(hwnd: Handle, id: usize, ms: u32, callback: *const c_void) -> usize;
    fn KillTimer(hwnd: Handle, id: usize) -> i32;
    fn RegisterHotKey(hwnd: Handle, id: i32, mods: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: Handle, id: i32) -> i32;
    fn DestroyWindow(hwnd: Handle) -> i32;
    fn MessageBoxW(hwnd: Handle, text: *const u16, title: *const u16, style: u32) -> i32;
    fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    fn LoadIconW(instance: Handle, name: *const u16) -> Handle;
    fn LoadImageW(
        instance: Handle,
        name: *const u16,
        kind: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Handle;
    fn DestroyIcon(icon: Handle) -> i32;
    fn SetProcessDpiAwarenessContext(context: isize) -> i32;
    fn GetDpiForWindow(hwnd: Handle) -> u32;
    fn GetDpiForSystem() -> u32;
    fn MoveWindow(hwnd: Handle, x: i32, y: i32, width: i32, height: i32, repaint: i32) -> i32;
    fn SetWindowPos(
        hwnd: Handle,
        after: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    #[allow(clashing_extern_declarations)]
    fn InvalidateRect(hwnd: Handle, rect: *const c_void, erase: i32) -> i32;
    fn SystemParametersInfoW(action: u32, param: u32, data: *mut c_void, flags: u32) -> i32;
    fn GetSysColor(index: i32) -> u32;
    fn MonitorFromWindow(hwnd: Handle, flags: u32) -> Handle;
    #[allow(clashing_extern_declarations)]
    fn GetMonitorInfoW(monitor: Handle, info: *mut MonitorInfo) -> i32;
    fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn GetClientRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn IsWindow(hwnd: Handle) -> i32;
    fn SetForegroundWindow(hwnd: Handle) -> i32;
    fn SetActiveWindow(hwnd: Handle) -> Handle;
    fn SetFocus(hwnd: Handle) -> Handle;
    fn SetClassLongPtrW(hwnd: Handle, index: i32, value: isize) -> usize;
    fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
    fn DrawTextW(dc: Handle, text: *const u16, len: i32, rect: *mut Rect, flags: u32) -> i32;
    fn GetDC(hwnd: Handle) -> Handle;
    fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
    fn BeginPaint(hwnd: Handle, paint: *mut PaintStruct) -> Handle;
    fn EndPaint(hwnd: Handle, paint: *const PaintStruct) -> i32;
    fn MapWindowPoints(from: Handle, to: Handle, points: *mut Point, count: u32) -> i32;
    fn GetParent(hwnd: Handle) -> Handle;
    fn GetFocus() -> Handle;
    fn AdjustWindowRectExForDpi(rect: *mut Rect, style: u32, menu: i32, ex: u32, dpi: u32) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        key: isize,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> i32;
}
#[repr(C)]
struct PaintStruct {
    dc: Handle,
    erase: i32,
    paint: Rect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetLocalTime(time: *mut SystemTimeWin);
    fn MoveFileExW(old: *const u16, new: *const u16, flags: u32) -> i32;
    fn GetDiskFreeSpaceExW(
        path: *const u16,
        available: *mut u64,
        total: *mut u64,
        free: *mut u64,
    ) -> i32;
}
#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: Handle,
        op: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
    ) -> Handle;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateFontW(
        height: i32,
        width: i32,
        esc: i32,
        orient: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out: u32,
        clip: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> Handle;
    fn DeleteObject(object: Handle) -> i32;
    fn CreateSolidBrush(color: u32) -> Handle;
    fn SetTextColor(dc: Handle, color: u32) -> u32;
    fn SetBkColor(dc: Handle, color: u32) -> u32;
    fn SetBkMode(dc: Handle, mode: i32) -> i32;
    fn SelectObject(dc: Handle, obj: Handle) -> Handle;
    fn RoundRect(dc: Handle, left: i32, top: i32, right: i32, bottom: i32, w: i32, h: i32) -> i32;
    fn CreatePen(style: i32, width: i32, color: u32) -> Handle;
    fn GetStockObject(index: i32) -> Handle;
    fn Polygon(dc: Handle, points: *const Point, count: i32) -> i32;
    fn Ellipse(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn MoveToEx(dc: Handle, x: i32, y: i32, old: *mut Point) -> i32;
    fn LineTo(dc: Handle, x: i32, y: i32) -> i32;
    fn Rectangle(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
}
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmExtendFrameIntoClientArea(hwnd: Handle, margins: *const Margins) -> i32;
    fn DwmSetWindowAttribute(hwnd: Handle, attribute: u32, value: *const c_void, size: u32) -> i32;
}
#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(
        hwnd: Handle,
        proc: Option<unsafe extern "system" fn(Handle, u32, usize, isize, usize, usize) -> isize>,
        id: usize,
        data: usize,
    ) -> i32;
    fn DefSubclassProc(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize;
    fn RemoveWindowSubclass(
        hwnd: Handle,
        proc: Option<unsafe extern "system" fn(Handle, u32, usize, isize, usize, usize) -> isize>,
        id: usize,
    ) -> i32;
}
#[repr(C)]
struct Margins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}
#[link(name = "uxtheme")]
unsafe extern "system" {
    fn SetWindowTheme(hwnd: Handle, app: *const u16, id: *const u16) -> i32;
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct HighContrast {
    size: u32,
    flags: u32,
    scheme: *mut u16,
}
#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}
#[repr(C)]
struct DrawItem {
    kind: u32,
    id: u32,
    item_id: u32,
    action: u32,
    state: u32,
    hwnd: Handle,
    dc: Handle,
    rect: Rect,
    data: usize,
}
const BG: u32 = 0x00221f1e;
const SURFACE: u32 = 0x0034302f;
const TEXT: u32 = 0x00f2efed;
const ACCENT: u32 = 0x004548d5;
// Acrylic backdrop and Direct2D are both available (not high contrast, and
// Windows transparency effects are on).
thread_local! { static ACRYLIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
thread_local! { static BRUSH: std::cell::Cell<Handle> = const { std::cell::Cell::new(0) }; }
thread_local! { static VIEW: std::cell::Cell<View> = const { std::cell::Cell::new(View::Panel) }; }
/// Child controls that are repainted when the material changes.
const CONTROL_IDS: [i32; 27] = [
    101, 102, 103, 104, 110, 111, 112, 113, 114, 115, 116, 117, 201, 202, 203, 204, 205, 206, 207,
    208, 209, 210, 211, 301, 302, 303, 304,
];
thread_local! { static DETAIL_FONT: std::cell::Cell<Handle> = const { std::cell::Cell::new(0) }; }
thread_local! { static PRIMARY_FONT: std::cell::Cell<Handle> = const { std::cell::Cell::new(0) }; }
thread_local! { static FONT: std::cell::Cell<Handle> = const { std::cell::Cell::new(0) }; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn set(hwnd: Handle, s: &str) {
    unsafe {
        SetWindowTextW(hwnd, wide(s).as_ptr());
        if hwnd != 0 {
            // Owner-drawn labels do not always repaint on WM_SETTEXT.
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    }
}
fn text(hwnd: Handle) -> String {
    let mut b = [0u16; 32768];
    let n = unsafe { GetWindowTextW(hwnd, b.as_mut_ptr(), b.len() as i32) };
    String::from_utf16_lossy(&b[..n.max(0) as usize])
}
#[allow(clippy::too_many_arguments)]
fn control(
    parent: Handle,
    instance: Handle,
    cls: &str,
    label: &str,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
) -> Handle {
    let dpi = unsafe { GetDpiForWindow(parent).max(96) } as i32;
    let (x, y, w, h) = (x * dpi / 96, y * dpi / 96, w * dpi / 96, h * dpi / 96);
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            wide(cls).as_ptr(),
            wide(label).as_ptr(),
            0x50000000
                | style
                | if cls == "STATIC" { 0 } else { 0x00010000 }
                | if cls == "BUTTON" { 0x0b } else { 0 },
            x,
            y,
            w,
            h,
            parent,
            id,
            instance,
            std::ptr::null(),
        )
    };
    unsafe {
        SendMessageW(hwnd, 0x30, font_for(id as u32) as usize, 1);
        if cls == "COMBOBOX" {
            SetWindowSubclass(hwnd, Some(combo_subclass), 1, 0);
        }
        if (cls == "COMBOBOX" || cls == "EDIT") && !high_contrast() {
            SetWindowTheme(hwnd, wide("DarkMode_Explorer").as_ptr(), std::ptr::null());
        }
    }
    hwnd
}
fn font_for(id: u32) -> Handle {
    if id == 101 {
        PRIMARY_FONT.with(|f| f.get())
    } else if [204, 208, 209, 211].contains(&id) {
        DETAIL_FONT.with(|f| f.get())
    } else {
        FONT.with(|f| f.get())
    }
}
fn reset_fonts(hwnd: Handle, dpi: u32) {
    let create = |size: u32, weight: i32| unsafe {
        CreateFontW(
            -((size * dpi / 96) as i32),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe UI").as_ptr(),
        )
    };
    let old = FONT.with(|f| f.replace(create(14, 500)));
    let old_detail = DETAIL_FONT.with(|f| f.replace(create(13, 400)));
    let old_primary = PRIMARY_FONT.with(|f| f.replace(create(15, 600)));
    unsafe {
        for id in [
            101, 102, 103, 104, 110, 111, 112, 113, 114, 115, 116, 117, 201, 202, 203, 204, 205,
            206, 207, 208, 209, 210, 211, 301, 302, 303, 304,
        ] {
            SendMessageW(GetDlgItem(hwnd, id), 0x30, font_for(id as u32) as usize, 1);
        }
        for font in [old, old_detail, old_primary] {
            if font != 0 {
                DeleteObject(font);
            }
        }
    }
}
fn high_contrast() -> bool {
    let mut hc = HighContrast {
        size: std::mem::size_of::<HighContrast>() as u32,
        flags: 0,
        scheme: std::ptr::null_mut(),
    };
    unsafe {
        SystemParametersInfoW(0x0042, hc.size, (&mut hc as *mut HighContrast).cast(), 0) != 0
            && hc.flags & 1 != 0
    }
}
fn update_appearance(hwnd: Handle) {
    unsafe {
        let high = high_contrast();
        let dark: i32 = if high { 0 } else { 1 };
        DwmSetWindowAttribute(hwnd, 20, (&dark as *const i32).cast(), 4);
        let wanted = !high && transparency_enabled() && crate::glass::available();
        // DWMSBT_TRANSIENTWINDOW: desktop acrylic. Windows draws it solid
        // while the window is inactive unless the frame stays active (see
        // WM_NCACTIVATE in `wnd`), which a recorder panel needs because it
        // usually sits unfocused beside the app being recorded.
        let acrylic_type: i32 = 3;
        let acrylic =
            wanted && DwmSetWindowAttribute(hwnd, 38, (&acrylic_type as *const i32).cast(), 4) == 0;
        if !acrylic {
            let backdrop: i32 = if high { 1 } else { 2 };
            DwmSetWindowAttribute(hwnd, 38, (&backdrop as *const i32).cast(), 4);
        }
        ACRYLIC.with(|cell| cell.set(acrylic));
        apply_material(hwnd);
        let brush = CreateSolidBrush(bg_color());
        SetClassLongPtrW(hwnd, -10, brush);
        BRUSH.with(|cell| {
            let old = cell.replace(brush);
            if old != 0 {
                DeleteObject(old);
            }
        });
        for id in [201, 202, 203, 205, 206, 207] {
            let control = GetDlgItem(hwnd, id);
            if high {
                SetWindowTheme(control, std::ptr::null(), std::ptr::null());
            } else {
                SetWindowTheme(
                    control,
                    wide("DarkMode_Explorer").as_ptr(),
                    std::ptr::null(),
                );
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}
/// Windows "Transparency effects" setting (Personalization > Colors).
fn transparency_enabled() -> bool {
    let mut value: u32 = 1;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            0x8000_0001u32 as i32 as isize,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("EnableTransparency").as_ptr(),
            0x10,
            std::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    status != 0 || value != 0
}
/// Every view is owner-drawn, so all of them can sit on glass.
fn glass_view(target: View) -> bool {
    matches!(target, View::Panel | View::Settings | View::Toolbar)
}
/// Extends acrylic over the whole client for glass views, or removes it.
fn apply_material(hwnd: Handle) {
    let on = ACRYLIC.with(|cell| cell.get()) && glass_view(VIEW.with(|v| v.get()));
    let edge = if on { -1 } else { 0 };
    let margins = Margins {
        left: edge,
        right: edge,
        top: edge,
        bottom: edge,
    };
    unsafe {
        let extended = DwmExtendFrameIntoClientArea(hwnd, &margins) == 0;
        crate::glass::set_enabled(on && extended);
        InvalidateRect(hwnd, std::ptr::null(), 1);
        for id in CONTROL_IDS {
            let child = GetDlgItem(hwnd, id);
            if child != 0 {
                InvalidateRect(child, std::ptr::null(), 1);
            }
        }
    }
}
fn bg_color() -> u32 {
    if high_contrast() {
        unsafe { GetSysColor(5) }
    } else {
        BG
    }
}
fn fg_color() -> u32 {
    if high_contrast() {
        unsafe { GetSysColor(8) }
    } else {
        TEXT
    }
}
fn draw_glyph(dc: Handle, id: u32, left: i32, top: i32, dpi: i32, color: u32) {
    let k = |n: i32| n * dpi / 96;
    let line = |points: &[(i32, i32)]| unsafe {
        if let Some((first, rest)) = points.split_first() {
            MoveToEx(
                dc,
                left + k(first.0),
                top + k(first.1),
                std::ptr::null_mut(),
            );
            for (x, y) in rest {
                LineTo(dc, left + k(*x), top + k(*y));
            }
        }
    };
    unsafe {
        let pen = CreatePen(0, k(1).max(1), color);
        let prev = SelectObject(dc, pen);
        let oldbrush = SelectObject(dc, GetStockObject(5));
        match id {
            110 => {
                Rectangle(dc, left + k(1), top + k(2), left + k(16), top + k(12));
                line(&[(8, 12), (8, 15), (4, 15), (12, 15)]);
            }
            111 => {
                Rectangle(dc, left + k(1), top + k(2), left + k(16), top + k(14));
                line(&[(1, 6), (16, 6)]);
            }
            112 => {
                line(&[(1, 6), (1, 2), (5, 2)]);
                line(&[(12, 2), (16, 2), (16, 6)]);
                line(&[(1, 10), (1, 14), (5, 14)]);
                line(&[(12, 14), (16, 14), (16, 10)]);
            }
            113 => {
                RoundRect(
                    dc,
                    left + k(5),
                    top + k(1),
                    left + k(12),
                    top + k(11),
                    k(6),
                    k(6),
                );
                line(&[
                    (2, 8),
                    (2, 10),
                    (4, 13),
                    (8, 14),
                    (12, 13),
                    (14, 10),
                    (14, 8),
                ]);
                line(&[(8, 14), (8, 16)]);
            }
            114 => {
                Ellipse(dc, left + k(3), top + k(3), left + k(14), top + k(14));
                Ellipse(dc, left + k(7), top + k(7), left + k(10), top + k(10));
                line(&[(8, 0), (8, 3)]);
                line(&[(8, 14), (8, 17)]);
                line(&[(0, 8), (3, 8)]);
                line(&[(14, 8), (17, 8)]);
            }
            210 => {
                line(&[
                    (1, 4),
                    (1, 14),
                    (16, 14),
                    (16, 5),
                    (8, 5),
                    (6, 3),
                    (1, 3),
                    (1, 4),
                ]);
            }
            _ => {}
        }
        SelectObject(dc, oldbrush);
        SelectObject(dc, prev);
        DeleteObject(pen);
    }
}
fn draw_button(item: &DrawItem) {
    if crate::glass::enabled() && draw_glass(item) {
        return;
    }
    if [204, 208, 209, 211, 301, 302, 303, 304].contains(&item.id) {
        unsafe {
            BRUSH.with(|brush| FillRect(item.dc, &item.rect, brush.get()));
            SetBkMode(item.dc, 1);
            SetTextColor(
                item.dc,
                if high_contrast() {
                    fg_color()
                } else {
                    0x00bcb5b1
                },
            );
            let old = SelectObject(item.dc, DETAIL_FONT.with(|f| f.get()));
            let mut rect = item.rect;
            DrawTextW(
                item.dc,
                wide(&text(item.hwnd)).as_ptr(),
                -1,
                &mut rect,
                0x4 | 0x20 | 0x8000,
            );
            SelectObject(item.dc, old);
        }
        return;
    }
    if item.id == 210 {
        let dpi = unsafe { GetDpiForWindow(item.hwnd).max(96) } as i32;
        draw_glyph(item.dc, 210, item.rect.left, item.rect.top, dpi, fg_color());
        return;
    }
    let selected = STATE.with(|cell| {
        cell.try_borrow()
            .ok()
            .and_then(|state| {
                state.as_ref().map(|s| match item.id {
                    110 => s.mode == 0,
                    111 => s.mode == 1,
                    112 => s.mode == 2,
                    113 => unsafe { SendMessageW(s.microphone, 0x147, 0, 0) > 0 },
                    _ => false,
                })
            })
            .unwrap_or(false)
    });
    let high = high_contrast();
    let fill = if item.state & 4 != 0 && !high {
        0x00514b49
    } else if high {
        unsafe { GetSysColor(if selected || item.id == 101 { 13 } else { 15 }) }
    } else if item.id == 101 {
        ACCENT
    } else if selected {
        0x00534b49
    } else if (110..=112).contains(&item.id) {
        0x002d2a29
    } else {
        SURFACE
    };
    let text_color = if high {
        unsafe { GetSysColor(if selected || item.id == 101 { 14 } else { 18 }) }
    } else {
        TEXT
    };
    let border = if item.state & 0x10 != 0 {
        if high { text_color } else { 0x00ffffff }
    } else if high {
        text_color
    } else {
        if (110..=112).contains(&item.id) {
            fill
        } else {
            0x00544f4d
        }
    };
    unsafe {
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(0, if item.state & 0x10 != 0 { 2 } else { 1 }, border);
        let old_brush = SelectObject(item.dc, brush);
        let old_pen = SelectObject(item.dc, pen);
        let radius = (8 * GetDpiForWindow(item.hwnd).max(96) / 96) as i32;
        RoundRect(
            item.dc,
            item.rect.left,
            item.rect.top,
            item.rect.right,
            item.rect.bottom,
            radius,
            radius,
        );
        SelectObject(item.dc, old_pen);
        SelectObject(item.dc, old_brush);
        DeleteObject(pen);
        DeleteObject(brush);
        let label = text(item.hwnd);
        let folder = if item.id == 102 {
            STATE.with(|cell| {
                cell.try_borrow()
                    .ok()
                    .and_then(|state| state.as_ref().map(|s| text(s.folder)))
                    .unwrap_or_default()
            })
        } else {
            String::new()
        };
        let display_label = if item.id == 116 || item.id == 117 {
            "Change"
        } else if item.id == 102 {
            &folder
        } else {
            &label
        };
        let text_wide = wide(display_label);
        let mut rect = item.rect;
        let dpi = GetDpiForWindow(item.hwnd).max(96) as i32;
        let k = |n: i32| n * dpi / 96;
        let mid = (rect.top + rect.bottom) / 2;
        if (110..=112).contains(&item.id) {
            draw_glyph(
                item.dc,
                item.id,
                rect.left + k(17),
                mid - k(8),
                dpi,
                text_color,
            );
            rect.left += k(18);
        }
        if item.id == 113 || item.id == 114 {
            draw_glyph(
                item.dc,
                item.id,
                rect.left + k(12),
                mid - k(8),
                dpi,
                text_color,
            );
            rect.left += k(29);
        }
        if item.id == 113 {
            let track = if selected { ACCENT } else { 0x00605b59 };
            let brush = CreateSolidBrush(track);
            let oldb = SelectObject(item.dc, brush);
            let pen = CreatePen(0, 1, track);
            let oldp = SelectObject(item.dc, pen);
            let right = item.rect.right - k(12);
            let left = right - k(30);
            RoundRect(item.dc, left, mid - k(8), right, mid + k(8), k(16), k(16));
            SelectObject(item.dc, oldp);
            SelectObject(item.dc, oldb);
            DeleteObject(pen);
            DeleteObject(brush);
            let knob = CreateSolidBrush(TEXT);
            let oldb = SelectObject(item.dc, knob);
            let x = if selected { right - k(13) } else { left + k(3) };
            Ellipse(item.dc, x, mid - k(5), x + k(10), mid + k(5));
            SelectObject(item.dc, oldb);
            DeleteObject(knob);
            rect.right -= k(45);
        }
        if item.id == 101 && label.starts_with("Start") {
            let dot = CreateSolidBrush(TEXT);
            let old = SelectObject(item.dc, dot);
            let x = (rect.left + rect.right) / 2 - k(75);
            Ellipse(item.dc, x, mid - k(5), x + k(10), mid + k(5));
            SelectObject(item.dc, old);
            DeleteObject(dot);
            rect.left += k(14);
        }
        let font = font_for(item.id);
        let oldfont = SelectObject(item.dc, font);
        SetBkMode(item.dc, 1);
        SetTextColor(item.dc, text_color);
        let align = if item.id == 113 { 0 } else { 1 };
        DrawTextW(
            item.dc,
            text_wide.as_ptr(),
            -1,
            &mut rect,
            align | 0x4 | 0x20 | 0x8000,
        );
        SelectObject(item.dc, oldfont);
    }
}
/// Parent-surface layer in parent client coordinates: the dark wash over the
/// acrylic plus the segmented-control track on the panel.
fn paint_backdrop(c: &crate::glass::Canvas, width: f32, height: f32) {
    use crate::glass::*;
    c.fill(0.0, 0.0, width, height, WASH);
    let view = VIEW.with(|v| v.get());
    if view == View::Panel {
        let (x, y, w, h, r) = (c.k(24.0), c.k(16.0), c.k(496.0), c.k(44.0), c.k(12.0));
        c.glass_card(x, y, w, h, r, CARD, 1.0);
        // Output card: save folder, quality and click effect.
        let (y, h) = (c.k(220.0), c.k(60.0));
        c.glass_card(x, y, w, h, r, CARD, 1.0);
    } else if view == View::Settings {
        c.text(
            "Settings",
            c.k(124.0),
            c.k(16.0),
            c.k(300.0),
            c.k(36.0),
            18.0,
            600,
            Align::Left,
            TEXT,
        );
    }
}
/// Paints a child control: the parent backdrop beneath it, then `draw` in
/// the child's own coordinates. Children cannot be transparent on glass, so
/// each one reproduces the part of the parent surface it covers.
fn paint_child(child: Handle, dc: Handle, rect: Rect, draw: impl FnOnce(&crate::glass::Canvas)) {
    unsafe {
        let parent = GetParent(child);
        let mut origin = Point { x: 0, y: 0 };
        MapWindowPoints(child, parent, &mut origin, 1);
        let mut client = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetClientRect(parent, &mut client);
        let dpi = GetDpiForWindow(child).max(96);
        crate::glass::paint(dc, rect.right, rect.bottom, dpi, |c| {
            c.set_offset(-origin.x as f32, -origin.y as f32);
            paint_backdrop(c, client.right as f32, client.bottom as f32);
            c.set_offset(0.0, 0.0);
            draw(c);
        });
    }
}
/// Line icons on a 17-unit grid, stroked with antialiasing.
fn glass_glyph(c: &crate::glass::Canvas, id: u32, left: f32, top: f32, color: crate::glass::Rgba) {
    let k = |n: f32| c.k(n);
    let p = |x: f32, y: f32| (left + k(x), top + k(y));
    let w = k(1.4);
    match id {
        110 => {
            c.stroke_round(
                left + k(0.5),
                top + k(1.5),
                k(16.0),
                k(11.0),
                k(2.0),
                w,
                color,
            );
            c.line(&[p(8.5, 12.5), p(8.5, 15.0)], w, color);
            c.line(&[p(4.5, 15.5), p(12.5, 15.5)], w, color);
        }
        111 => {
            c.stroke_round(
                left + k(0.5),
                top + k(1.5),
                k(16.0),
                k(13.0),
                k(2.0),
                w,
                color,
            );
            c.line(&[p(1.0, 5.8), p(16.0, 5.8)], w, color);
        }
        112 => {
            c.line(&[p(1.0, 6.0), p(1.0, 2.0), p(5.0, 2.0)], w, color);
            c.line(&[p(12.0, 2.0), p(16.0, 2.0), p(16.0, 6.0)], w, color);
            c.line(&[p(1.0, 10.0), p(1.0, 14.0), p(5.0, 14.0)], w, color);
            c.line(&[p(12.0, 14.0), p(16.0, 14.0), p(16.0, 10.0)], w, color);
        }
        113 => {
            c.stroke_round(
                left + k(5.0),
                top + k(0.5),
                k(7.0),
                k(11.0),
                k(3.5),
                w,
                color,
            );
            c.line(
                &[
                    p(2.5, 7.5),
                    p(2.8, 9.8),
                    p(4.6, 12.4),
                    p(8.5, 13.8),
                    p(12.4, 12.4),
                    p(14.2, 9.8),
                    p(14.5, 7.5),
                ],
                w,
                color,
            );
            c.line(&[p(8.5, 13.8), p(8.5, 16.5)], w, color);
        }
        114 => {
            // Sliders: reads as "settings" rather than a crosshair target.
            c.line(&[p(1.0, 4.5), p(8.0, 4.5)], w, color);
            c.line(&[p(14.0, 4.5), p(16.0, 4.5)], w, color);
            c.ring(left + k(11.0), top + k(4.5), k(2.6), w, color);
            c.line(&[p(1.0, 12.5), p(3.0, 12.5)], w, color);
            c.line(&[p(9.0, 12.5), p(16.0, 12.5)], w, color);
            c.ring(left + k(6.0), top + k(12.5), k(2.6), w, color);
        }
        115 => c.line(&[p(10.5, 3.0), p(5.0, 8.5), p(10.5, 14.0)], k(1.6), color),
        210 => {
            c.line(
                &[
                    p(1.0, 4.0),
                    p(1.0, 14.0),
                    p(16.0, 14.0),
                    p(16.0, 5.5),
                    p(8.0, 5.5),
                    p(6.0, 3.0),
                    p(1.0, 3.0),
                    p(1.0, 4.0),
                ],
                w,
                color,
            );
        }
        _ => {}
    }
}
/// Owner-draw on glass. Returns false when the control has no glass design.
fn draw_glass(item: &DrawItem) -> bool {
    use crate::glass::*;
    let id = item.id;
    let label = text(item.hwnd);
    let (mode, mic_on, folder) = STATE.with(|cell| {
        cell.try_borrow()
            .ok()
            .and_then(|state| {
                state.as_ref().map(|s| {
                    (
                        s.mode,
                        unsafe { SendMessageW(s.microphone, 0x147, 0, 0) > 0 },
                        text(s.folder),
                    )
                })
            })
            .unwrap_or((0, false, String::new()))
    });
    let pressed = item.state & 0x1 != 0;
    let disabled = item.state & 0x4 != 0;
    let focus = item.state & 0x10 != 0 && item.state & 0x200 == 0;
    let toolbar = VIEW.with(|v| v.get()) == View::Toolbar;
    paint_child(item.hwnd, item.dc, item.rect, |c| {
        let (w, h) = (item.rect.right as f32, item.rect.bottom as f32);
        let mid = h / 2.0;
        let focus_ring = |r: f32| {
            if focus {
                c.stroke_round(0.0, 0.0, w, h, r, c.k(2.0), white(0.85));
            }
        };
        let card = |r: f32| {
            let fill = if pressed { CARD_PRESSED } else { CARD };
            c.glass_card(0.0, 0.0, w, h, r, fill, 1.0);
        };
        match id {
            110..=112 => {
                let selected = mode as u32 == id - 110;
                let r = c.k(9.0);
                if selected {
                    c.glass_card(0.0, 0.0, w, h, r, SELECTED, 1.0);
                } else if pressed {
                    c.fill_round(0.0, 0.0, w, h, r, CARD_HOVER);
                }
                let color = if selected {
                    ON_SELECTED
                } else {
                    TEXT_SECONDARY
                };
                let weight = if selected { 600 } else { 500 };
                let tw = c.measure(&label, 14.0, weight);
                let group = c.k(17.0) + c.k(10.0) + tw;
                let x = (w - group) / 2.0;
                glass_glyph(c, id, x, mid - c.k(8.5), color);
                c.text(
                    &label,
                    x + c.k(27.0),
                    0.0,
                    tw + c.k(4.0),
                    h,
                    14.0,
                    weight,
                    Align::Left,
                    color,
                );
                focus_ring(r);
            }
            113 => {
                let r = c.k(12.0);
                card(r);
                glass_glyph(c, 113, c.k(16.0), mid - c.k(8.5), TEXT);
                c.text(
                    "Microphone",
                    c.k(44.0),
                    0.0,
                    w - c.k(130.0),
                    h,
                    14.0,
                    500,
                    Align::Left,
                    TEXT,
                );
                let (tw, th) = (c.k(36.0), c.k(20.0));
                let tx = w - c.k(16.0) - tw;
                let track = if mic_on { LIVE } else { white(0.22) };
                c.fill_round(tx, mid - th / 2.0, tw, th, th / 2.0, track);
                let knob = if mic_on {
                    tx + tw - th / 2.0
                } else {
                    tx + th / 2.0
                };
                // Knob at 82 % of the track height.
                c.circle(knob, mid, th * 0.41, white(0.98));
                let state = if mic_on { "On" } else { "Off" };
                c.text(
                    state,
                    tx - c.k(52.0),
                    0.0,
                    c.k(44.0),
                    h,
                    13.0,
                    400,
                    Align::Right,
                    TEXT_SECONDARY,
                );
                focus_ring(r);
            }
            114 => {
                let r = c.k(12.0);
                card(r);
                let tw = c.measure(&label, 14.0, 500);
                let x = (w - (c.k(27.0) + tw)) / 2.0;
                glass_glyph(c, 114, x, mid - c.k(8.5), TEXT);
                c.text(
                    &label,
                    x + c.k(27.0),
                    0.0,
                    tw + c.k(4.0),
                    h,
                    14.0,
                    500,
                    Align::Left,
                    TEXT,
                );
                focus_ring(r);
            }
            101 => {
                let r = c.k(14.0);
                if label == "Cancel" {
                    // Cancelling the countdown is not a destructive or
                    // primary action; keep red for start/stop.
                    card(r);
                } else {
                    let fill = if disabled {
                        Rgba(RECORD.0, RECORD.1, RECORD.2, 0.45)
                    } else if pressed {
                        RECORD_PRESSED
                    } else {
                        RECORD
                    };
                    c.glass_card(0.0, 0.0, w, h, r, fill, 1.2);
                }
                let size = if toolbar { 14.0 } else { 16.0 };
                let tw = c.measure(&label, size, 600);
                let icon = label.starts_with("Start") || label.starts_with("Stop");
                let gap = if icon { c.k(22.0) } else { 0.0 };
                let x = (w - (gap + tw)) / 2.0;
                if label.starts_with("Start") {
                    c.circle(x + c.k(6.0), mid, c.k(6.0), white(0.98));
                } else if label.starts_with("Stop") {
                    c.fill_round(
                        x,
                        mid - c.k(6.0),
                        c.k(12.0),
                        c.k(12.0),
                        c.k(3.0),
                        white(0.98),
                    );
                }
                c.text(
                    &label,
                    x + gap,
                    0.0,
                    tw + c.k(4.0),
                    h,
                    size,
                    600,
                    Align::Left,
                    TEXT,
                );
                focus_ring(r);
            }
            103 | 104 | 116 | 117 => {
                let r = c.k(9.0);
                card(r);
                let shown = if id == 116 || id == 117 {
                    "Change"
                } else {
                    &label
                };
                let color = if disabled { white(0.35) } else { TEXT };
                c.text(shown, 0.0, 0.0, w, h, 13.0, 500, Align::Center, color);
                focus_ring(r);
            }
            115 => {
                let r = c.k(10.0);
                card(r);
                glass_glyph(c, 115, c.k(12.0), mid - c.k(8.5), TEXT);
                c.text(
                    "Back",
                    c.k(34.0),
                    0.0,
                    w - c.k(40.0),
                    h,
                    14.0,
                    500,
                    Align::Left,
                    TEXT,
                );
                focus_ring(r);
            }
            102 => {
                // Save-folder card: the whole card opens the folder picker.
                let r = c.k(12.0);
                card(r);
                glass_glyph(c, 210, c.k(16.0), mid - c.k(8.5), TEXT_SECONDARY);
                c.text(
                    &folder,
                    c.k(46.0),
                    0.0,
                    w - c.k(150.0),
                    h,
                    13.5,
                    500,
                    Align::Left,
                    TEXT,
                );
                let (pw, ph) = (c.k(84.0), c.k(30.0));
                let px = w - c.k(12.0) - pw;
                c.glass_card(px, mid - ph / 2.0, pw, ph, c.k(9.0), white(0.08), 0.8);
                c.text(
                    "Change",
                    px,
                    mid - ph / 2.0,
                    pw,
                    ph,
                    13.0,
                    500,
                    Align::Center,
                    TEXT,
                );
                focus_ring(r);
            }
            301..=304 => c.text(&label, 0.0, 0.0, w, h, 13.0, 600, Align::Left, white(0.78)),
            210 => glass_glyph(c, 210, 0.0, (h - c.k(17.0)) / 2.0, TEXT_SECONDARY),
            204 if toolbar => {
                // Live dot only once capture runs, not during the countdown.
                let recording = label.starts_with("Recording  ");
                let mut x = 0.0;
                if recording {
                    c.circle(c.k(6.0), mid, c.k(5.0), RECORD);
                    x = c.k(20.0);
                }
                c.text(&label, x, 0.0, w - x, h, 14.0, 600, Align::Left, TEXT);
            }
            209 => c.text(&label, 0.0, 0.0, w, h, 14.0, 500, Align::Left, TEXT),
            204 if label == "Ready" => c.text(
                "Stop anytime with Ctrl+Shift+F9",
                0.0,
                0.0,
                w,
                h,
                12.5,
                400,
                Align::Left,
                white(0.62),
            ),
            204 | 208 | 211 => {
                c.text(
                    &label,
                    0.0,
                    0.0,
                    w,
                    h,
                    13.0,
                    400,
                    Align::Left,
                    TEXT_SECONDARY,
                );
            }
            _ => {}
        }
    });
    true
}
/// Closed-state source picker on glass; the drop-down list is a separate
/// popup window and keeps its native rendering.
fn paint_glass_combo(hwnd: Handle) {
    use crate::glass::*;
    unsafe {
        let mut ps: PaintStruct = std::mem::zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut r = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetClientRect(hwnd, &mut r);
        let label = text(hwnd);
        let focused = GetFocus() == hwnd;
        let mode = STATE.with(|cell| {
            cell.try_borrow()
                .ok()
                .and_then(|state| state.as_ref().map(|s| s.mode))
                .unwrap_or(0)
        });
        paint_child(hwnd, dc, r, |c| {
            let (w, h) = (r.right as f32, r.bottom as f32);
            let radius = c.k(12.0);
            c.glass_card(0.0, 0.0, w, h, radius, CARD, 1.0);
            if focused {
                c.stroke_round(0.0, 0.0, w, h, radius, c.k(1.5), white(0.55));
            }
            let left = if GetDlgCtrlID(hwnd) == 207 {
                let glyph = if mode == 1 { 111 } else { 110 };
                glass_glyph(c, glyph, c.k(16.0), h / 2.0 - c.k(8.5), TEXT_SECONDARY);
                46.0
            } else {
                16.0
            };
            c.text(
                &label,
                c.k(left),
                0.0,
                w - c.k(left + 40.0),
                h,
                14.0,
                500,
                Align::Left,
                TEXT,
            );
            let (cx, cy) = (w - c.k(22.0), h / 2.0);
            c.line(
                &[
                    (cx - c.k(5.0), cy - c.k(2.5)),
                    (cx, cy + c.k(2.5)),
                    (cx + c.k(5.0), cy - c.k(2.5)),
                ],
                c.k(1.5),
                TEXT_SECONDARY,
            );
        });
        EndPaint(hwnd, &ps);
    }
}
fn position_controls(s: &State, dpi: i32) {
    let scale = |n: i32| n * dpi / 96;
    // Logical 96-DPI layout for a 560-wide window: 24 px gutters, 496 px
    // content column, 8 px rhythm.
    let controls = [
        (110, 28, 20, 162, 36),
        (111, 191, 20, 162, 36),
        (112, 354, 20, 162, 36),
        (207, 24, 72, 496, 220),
        (211, 26, 124, 380, 22),
        (117, 428, 121, 92, 28),
        (113, 24, 156, 244, 52),
        (114, 276, 156, 244, 52),
        (210, 40, 239, 17, 22),
        (209, 68, 229, 330, 22),
        (208, 68, 251, 330, 20),
        (116, 416, 236, 88, 28),
        (103, 384, 363, 136, 30),
        (104, 272, 363, 104, 30),
        (115, 24, 18, 88, 32),
        (301, 26, 68, 494, 20),
        (202, 24, 92, 496, 220),
        (302, 26, 152, 494, 20),
        (205, 24, 176, 496, 220),
        (303, 26, 236, 494, 20),
        (206, 24, 260, 496, 220),
        (304, 26, 320, 494, 20),
        (203, 24, 344, 386, 27),
        (102, 24, 344, 496, 52),
    ];
    unsafe {
        for (id, x, y, w, h) in controls {
            MoveWindow(
                GetDlgItem(s.window, id),
                scale(x),
                scale(y),
                scale(w),
                scale(h),
                1,
            );
        }
    }
}
fn resize_within_work_area(hwnd: Handle, width: i32, height: i32, active: bool) {
    unsafe {
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let monitor = MonitorFromWindow(hwnd, 2);
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            monitor: rect,
            work: rect,
            flags: 0,
        };
        let position =
            GetWindowRect(hwnd, &mut rect) != 0 && GetMonitorInfoW(monitor, &mut info) != 0;
        let x = if position {
            rect.left.clamp(
                info.work.left,
                (info.work.right - width).max(info.work.left),
            )
        } else {
            rect.left
        };
        let y = if position {
            rect.top.clamp(
                info.work.top,
                (info.work.bottom - height).max(info.work.top),
            )
        } else {
            rect.top
        };
        SetWindowPos(
            hwnd,
            if active { -1 } else { -2 },
            x,
            y,
            width,
            height,
            if position { 0x0010 } else { 0x0002 | 0x0010 },
        );
    }
}
unsafe extern "system" fn combo_subclass(
    hwnd: Handle,
    msg: u32,
    w: usize,
    l: isize,
    _id: usize,
    _data: usize,
) -> isize {
    let glass_combo = [207, 202, 205, 206].contains(&unsafe { GetDlgCtrlID(hwnd) });
    if glass_combo && matches!(msg, 0x0030 | 0x031a | 0x02e3) {
        // Font and theme changes re-measure the selection field; keep the
        // picker as tall as the cards around it.
        let result = unsafe { DefSubclassProc(hwnd, msg, w, l) };
        set_picker_height(hwnd);
        return result;
    }
    if crate::glass::enabled() && glass_combo {
        match msg {
            0x000f => {
                paint_glass_combo(hwnd);
                return 0;
            }
            0x0014 => return 1,
            _ => {}
        }
        let result = unsafe { DefSubclassProc(hwnd, msg, w, l) };
        // The combo paints its focus/selection state directly on some
        // input; repaint through the glass path afterwards.
        if matches!(
            msg,
            0x0007 | 0x0008 | 0x0100 | 0x0201 | 0x0202 | 0x014e | 0x000c | 0x0111
        ) {
            unsafe {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
        }
        if msg == 0x0082 {
            unsafe {
                RemoveWindowSubclass(hwnd, Some(combo_subclass), 1);
            }
        }
        return result;
    }
    let result = unsafe { DefSubclassProc(hwnd, msg, w, l) };
    if msg == 0x000f && !high_contrast() {
        unsafe {
            let mut r = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetClientRect(hwnd, &mut r);
            let dpi = GetDpiForWindow(hwnd).max(96) as i32;
            let scale = |n: i32| n * dpi / 96;
            let dc = GetDC(hwnd);
            let brush = CreateSolidBrush(SURFACE);
            let arrow = Rect {
                left: (r.right - scale(27)).max(0),
                top: r.top + 1,
                right: r.right - 1,
                bottom: r.bottom - 1,
            };
            FillRect(dc, &arrow, brush);
            DeleteObject(brush);
            let cx = (arrow.left + arrow.right) / 2;
            let cy = (arrow.top + arrow.bottom) / 2;
            let points = [
                Point {
                    x: cx - scale(4),
                    y: cy - scale(2),
                },
                Point {
                    x: cx + scale(4),
                    y: cy - scale(2),
                },
                Point {
                    x: cx,
                    y: cy + scale(2),
                },
            ];
            let color = CreateSolidBrush(TEXT);
            let old = SelectObject(dc, color);
            Polygon(dc, points.as_ptr(), 3);
            SelectObject(dc, old);
            DeleteObject(color);
            let border = CreatePen(0, scale(1).max(1), 0x00544f4d);
            let oldpen = SelectObject(dc, border);
            let oldbrush = SelectObject(dc, GetStockObject(5));
            let corner = CreateSolidBrush(bg_color());
            for corner_rect in [
                Rect {
                    left: r.left,
                    top: r.top,
                    right: r.left + scale(3),
                    bottom: r.top + scale(3),
                },
                Rect {
                    left: r.right - scale(3),
                    top: r.top,
                    right: r.right,
                    bottom: r.top + scale(3),
                },
                Rect {
                    left: r.left,
                    top: r.bottom - scale(3),
                    right: r.left + scale(3),
                    bottom: r.bottom,
                },
                Rect {
                    left: r.right - scale(3),
                    top: r.bottom - scale(3),
                    right: r.right,
                    bottom: r.bottom,
                },
            ] {
                FillRect(dc, &corner_rect, corner);
            }
            DeleteObject(corner);
            RoundRect(dc, r.left, r.top, r.right, r.bottom, scale(8), scale(8));
            SelectObject(dc, oldbrush);
            SelectObject(dc, oldpen);
            DeleteObject(border);
            ReleaseDC(hwnd, dc);
        }
    }
    if msg == 0x0082 {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(combo_subclass), 1);
        }
    }
    result
}
/// Closed source-picker height (selection field 38 px, about 44 px overall).
/// Resizing the combo recomputes this, so call it after moving the control.
fn set_picker_height(picker: Handle) {
    unsafe {
        let height = (38 * GetDpiForWindow(picker).max(96) / 96) as isize;
        if SendMessageW(picker, 0x154, usize::MAX, 0) != height {
            SendMessageW(picker, 0x153, usize::MAX, height);
        }
    }
}
fn settings_path() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
        .join("FrameLatch")
        .join("settings.txt")
}
struct State {
    window: Handle,
    source: Handle,
    quality: Handle,
    microphone: Handle,
    click_style: Handle,
    microphone_names: Vec<String>,
    folder: Handle,
    button: Handle,
    status: Handle,
    open: Handle,
    stop: Option<Arc<AtomicBool>>,
    receiver: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    started: Option<Instant>,
    ready: Option<Arc<AtomicBool>>,
    countdown: Option<Instant>,
    last: Option<PathBuf>,
    hotkey: bool,
    pending_close: bool,
    monitor_ids: Vec<String>,
    monitor_sizes: Vec<(u32, u32)>,
    windows: Vec<(usize, u32)>,
    area: Option<(usize, (u32, u32, u32, u32))>,
    picker: Handle,
    mode: u8,
    settings_open: bool,
    last_mic: usize,
}
thread_local! { static STATE:RefCell<Option<State>>=const {RefCell::new(None)}; }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Panel,
    Settings,
    Toolbar,
}
fn view(settings: bool, active: bool) -> View {
    if active {
        View::Toolbar
    } else if settings {
        View::Settings
    } else {
        View::Panel
    }
}
fn refresh_picker(s: &mut State) {
    unsafe {
        let old = SendMessageW(s.source, 0x147, 0, 0).max(0) as usize;
        SendMessageW(s.picker, 0x14b, 0, 0);
        let count = SendMessageW(s.source, 0x146, 0, 0).max(0) as usize;
        let (first, end) = if s.mode == 1 {
            (s.monitor_ids.len(), count)
        } else {
            (0, s.monitor_ids.len())
        };
        for i in first..end {
            let mut name = vec![0u16; 32768];
            if SendMessageW(s.source, 0x148, i, name.as_mut_ptr() as isize) >= 0 {
                SendMessageW(s.picker, 0x143, 0, name.as_ptr() as isize);
            }
        }
        if end > first {
            let selected = if old >= first && old < end {
                old - first
            } else {
                0
            };
            SendMessageW(s.picker, 0x14e, selected, 0);
            SendMessageW(s.source, 0x14e, selected + first, 0);
        } else {
            SendMessageW(s.source, 0x14e, usize::MAX, 0);
        }
    }
}
fn source_detail(
    mode: u8,
    area: Option<(u32, u32, u32, u32)>,
    screen: Option<(u32, u32)>,
) -> String {
    match mode {
        2 => area
            .map(|(_, _, w, h)| format!("Selected area · {w} × {h}"))
            .unwrap_or_else(|| "Select an area to record".into()),
        1 => "Records only this window".into(),
        _ => screen
            .map(|(w, h)| format!("Screen · {w} × {h}"))
            .unwrap_or_else(|| "Select a screen".into()),
    }
}
fn update_summary(s: &State) {
    let index = unsafe { SendMessageW(s.source, 0x147, 0, 0) };
    let area = s.area.map(|(_, rect)| rect);
    let screen = usize::try_from(index)
        .ok()
        .and_then(|i| s.monitor_sizes.get(i).copied());
    set(
        unsafe { GetDlgItem(s.window, 211) },
        &source_detail(s.mode, area, screen),
    );
    unsafe {
        ShowWindow(
            GetDlgItem(s.window, 117),
            if s.mode == 2 && !s.settings_open && s.stop.is_none() && s.countdown.is_none() {
                1
            } else {
                0
            },
        );
    }
    let mic = unsafe { SendMessageW(s.microphone, 0x147, 0, 0) };
    set(
        unsafe { GetDlgItem(s.window, 113) },
        if mic > 0 {
            "Microphone: on"
        } else {
            "Microphone: off"
        },
    );
    let selected_quality = unsafe { SendMessageW(s.quality, 0x147, 0, 0) };
    let quality = usize::try_from(selected_quality)
        .ok()
        .and_then(crate::quality::Quality::from_index)
        .unwrap_or(crate::quality::Quality::Standard)
        .summary();
    let clicks = match unsafe { SendMessageW(s.click_style, 0x147, 0, 0) } {
        1 => "Clicks: red ring",
        2 => "Clicks: red ripple",
        _ => "Clicks: not shown",
    };
    set(
        unsafe { GetDlgItem(s.window, 208) },
        &format!("{quality}  ·  {clicks}"),
    );
    let folder = PathBuf::from(text(s.folder));
    let display = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| folder.to_string_lossy().into_owned());
    set(
        unsafe { GetDlgItem(s.window, 209) },
        &format!("Saves to {display}"),
    );
    unsafe {
        for id in [110, 111, 112, 113] {
            InvalidateRect(GetDlgItem(s.window, id), std::ptr::null(), 1);
        }
    }
}
fn layout(s: &State) {
    let active = s.stop.is_some() || s.countdown.is_some();
    let target = view(s.settings_open, active);
    VIEW.with(|v| v.set(target));
    apply_material(s.window);
    let panel = [110, 111, 112, 207, 113, 114, 116, 208, 209, 210, 211];
    // The folder edit box (203) stays hidden: it holds the path, and the
    // folder card (102) shows it and opens the picker.
    let settings = [115, 202, 205, 206, 102, 301, 302, 303, 304];
    unsafe {
        for id in panel {
            ShowWindow(
                GetDlgItem(s.window, id),
                if target == View::Panel { 1 } else { 0 },
            );
        }
        for id in settings {
            ShowWindow(
                GetDlgItem(s.window, id),
                if target == View::Settings { 1 } else { 0 },
            );
        }
        ShowWindow(
            GetDlgItem(s.window, 117),
            if target == View::Panel && s.mode == 2 {
                1
            } else {
                0
            },
        );
        for id in [103, 104] {
            ShowWindow(
                GetDlgItem(s.window, id),
                if target == View::Panel && s.last.as_ref().is_some_and(|path| path.is_file()) {
                    1
                } else {
                    0
                },
            );
        }
        ShowWindow(GetDlgItem(s.window, 203), 0);
        ShowWindow(s.status, if target == View::Settings { 0 } else { 1 });
        ShowWindow(s.button, if target == View::Settings { 0 } else { 1 });
        let dpi = GetDpiForWindow(s.window).max(96) as i32;
        let scale = |n: i32| n * dpi / 96;
        position_controls(s, dpi);
        for combo in [s.picker, s.quality, s.microphone, s.click_style] {
            set_picker_height(combo);
        }
        if target == View::Toolbar {
            MoveWindow(s.status, scale(24), scale(17), scale(250), scale(28), 1);
            MoveWindow(s.button, scale(284), scale(10), scale(156), scale(42), 1);
        } else {
            MoveWindow(s.status, scale(26), scale(366), scale(240), scale(26), 1);
            MoveWindow(s.button, scale(24), scale(296), scale(496), scale(52), 1);
        }
        // Exact client sizes; the frame is added for the current DPI.
        let (w, h) = match target {
            View::Panel => (544, 408),
            View::Settings => (544, 416),
            View::Toolbar => (464, 62),
        };
        let mut frame = Rect {
            left: 0,
            top: 0,
            right: scale(w),
            bottom: scale(h),
        };
        AdjustWindowRectExForDpi(&mut frame, 0x00ca0000, 0, 0, dpi as u32);
        resize_within_work_area(
            s.window,
            frame.right - frame.left,
            frame.bottom - frame.top,
            active,
        );
        InvalidateRect(s.window, std::ptr::null(), 1);
    }
}
fn browse_folder(hwnd: Handle) -> Option<String> {
    use windows::Win32::{
        Foundation::HWND,
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree},
        UI::Shell::{
            FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
            SIGDN_FILESYSPATH,
        },
    };
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().ok()?;
        dialog
            .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)
            .ok()?;
        dialog.Show(Some(HWND(hwnd as *mut c_void))).ok()?;
        let path = dialog
            .GetResult()
            .ok()?
            .GetDisplayName(SIGDN_FILESYSPATH)
            .ok()?;
        let text = path.to_string().ok();
        CoTaskMemFree(Some(path.0.cast()));
        text
    }
}
fn reveal(path: &std::path::Path) -> Result<(), String> {
    use windows::{
        Win32::{
            System::Com::CoTaskMemFree,
            UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName},
        },
        core::HSTRING,
    };
    unsafe {
        let name = HSTRING::from(path.as_os_str());
        let mut item = std::ptr::null_mut();
        SHParseDisplayName(&name, None, &mut item, 0, None).map_err(|e| e.to_string())?;
        let result = SHOpenFolderAndSelectItems(item, None, 0).map_err(|e| e.to_string());
        CoTaskMemFree(Some(item.cast()));
        result
    }
}
fn default_folder() -> PathBuf {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_Videos, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath},
    };
    let folder = unsafe {
        SHGetKnownFolderPath(&FOLDERID_Videos, KNOWN_FOLDER_FLAG(0), None)
            .ok()
            .and_then(|path| {
                let name = path.to_string().ok();
                CoTaskMemFree(Some(path.0.cast()));
                name
            })
            .map(PathBuf::from)
    }
    .unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join("Videos")
    });
    let folder = folder.join("FrameLatch");
    let _ = std::fs::create_dir_all(&folder);
    folder
}
fn message(hwnd: Handle, s: &str) {
    unsafe {
        MessageBoxW(hwnd, wide(s).as_ptr(), wide("FrameLatch").as_ptr(), 0x10);
    }
}
fn save(state: &State) {
    let path = settings_path();
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let contents = format!(
        "FrameLatchSettings=1\n{}\n{}\n{}\n{}\n{}\n",
        text(state.folder),
        state
            .monitor_ids
            .get(unsafe { SendMessageW(state.source, 0x147, 0, 0) }.max(0) as usize)
            .map(String::as_str)
            .unwrap_or(""),
        unsafe { SendMessageW(state.quality, 0x147, 0, 0) },
        state
            .microphone_names
            .get(unsafe { SendMessageW(state.microphone, 0x147, 0, 0) }.saturating_sub(1) as usize)
            .map(String::as_str)
            .unwrap_or(""),
        unsafe { SendMessageW(state.click_style, 0x147, 0, 0) }
    );
    let temp = path.with_extension("tmp");
    if std::fs::write(&temp, contents).is_ok() {
        unsafe {
            MoveFileExW(
                wide(&temp.to_string_lossy()).as_ptr(),
                wide(&path.to_string_lossy()).as_ptr(),
                1 | 8,
            );
        }
    }
}
fn begin(s: &mut State) {
    set(s.button, "Start recording");
    set(s.status, "Ready");
    if !area_ready(s.mode, s.area.is_some()) {
        message(s.window, "Select an area before recording.");
        return;
    }
    let dir = PathBuf::from(text(s.folder));
    if !dir.is_dir() {
        message(s.window, "Choose an existing save folder.");
        return;
    }
    let mut available = 0u64;
    let free_ok = unsafe {
        GetDiskFreeSpaceExW(
            wide(&dir.to_string_lossy()).as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if free_ok == 0 || available < 100 * 1024 * 1024 {
        message(
            s.window,
            "Save folder needs at least 100 MB of free disk space.",
        );
        return;
    }
    let index = unsafe { SendMessageW(s.source, 0x147, 0, 0) };
    if index < 0 {
        message(s.window, "Choose a monitor.");
        return;
    }
    let quality = usize::try_from(unsafe { SendMessageW(s.quality, 0x147, 0, 0) })
        .ok()
        .and_then(crate::quality::Quality::from_index)
        .unwrap_or(crate::quality::Quality::Standard);
    let click_style = unsafe { SendMessageW(s.click_style, 0x147, 0, 0) }.clamp(0, 2) as u8;
    let selected_mic = unsafe { SendMessageW(s.microphone, 0x147, 0, 0) };
    let mic_index = if selected_mic > 0 {
        Some(selected_mic as usize - 1)
    } else {
        None
    };
    if let Some(mic_index) = mic_index {
        let current = match crate::audio::devices() {
            Ok(v) => v,
            Err(e) => {
                message(s.window, &e);
                return;
            }
        };
        if current.get(mic_index) != s.microphone_names.get(mic_index)
            || current
                .iter()
                .filter(|name| Some(*name) == current.get(mic_index))
                .count()
                != 1
        {
            message(
                s.window,
                "Microphone list changed. Restart FrameLatch and choose it again.",
            );
            return;
        }
    }
    let window_handle = if (index as usize) < s.monitor_ids.len() {
        None
    } else {
        let (handle, pid) = match s.windows.get(index as usize - s.monitor_ids.len()) {
            Some(value) => *value,
            None => {
                message(s.window, "Choose a recording source.");
                return;
            }
        };
        let target = windows_capture::window::Window::from_raw_hwnd(handle as *mut c_void);
        if !target.is_valid() || target.process_id().ok() != Some(pid) {
            message(
                s.window,
                "Selected window is no longer available. Restart FrameLatch and select again.",
            );
            return;
        }
        Some(handle)
    };
    let crop = s
        .area
        .filter(|(monitor, _)| *monitor == index as usize && window_handle.is_none())
        .map(|(_, area)| area);
    let current = match Monitor::enumerate() {
        Ok(monitors) => monitors,
        Err(e) => {
            message(s.window, &format!("Cannot list monitors: {e}"));
            return;
        }
    };
    let identity = current
        .get(index as usize)
        .and_then(|m| m.device_name().ok())
        .unwrap_or_default();
    if window_handle.is_none() && s.monitor_ids.get(index as usize) != Some(&identity) {
        message(
            s.window,
            "Displays changed. Restart FrameLatch and choose the display again.",
        );
        return;
    }
    let mut time: SystemTimeWin = unsafe { std::mem::zeroed() };
    unsafe {
        GetLocalTime(&mut time);
    }
    let mut path = dir.join(format!(
        "FrameLatch-{:04}-{:02}-{:02}-{:02}{:02}{:02}-{:03}.mp4",
        time.year, time.month, time.day, time.hour, time.minute, time.second, time.milliseconds
    ));
    let mut suffix = 1u32;
    while path.exists() || path.with_extension("partial.mp4").exists() {
        path = dir.join(format!(
            "FrameLatch-{:04}-{:02}-{:02}-{:02}{:02}{:02}-{:03}-{suffix}.mp4",
            time.year, time.month, time.day, time.hour, time.minute, time.second, time.milliseconds
        ));
        suffix += 1;
    }
    save(s);
    s.hotkey = unsafe { RegisterHotKey(s.window, 1, 0x0002 | 0x0004 | 0x4000, 0x78) != 0 };
    if !s.hotkey {
        message(
            s.window,
            "Ctrl+Shift+F9 is unavailable. Use Stop recording in FrameLatch.",
        );
    }
    unsafe {
        SetWindowDisplayAffinity(s.window, 0x11);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let ready = Arc::new(AtomicBool::new(false));
    let ready_worker = ready.clone();
    let (tx, rx) = mpsc::channel();
    let mic_name = mic_index.and_then(|i| s.microphone_names.get(i).cloned());
    std::thread::spawn(move || {
        let result = crate::record::record_control(
            index as usize,
            86_400,
            &path,
            signal,
            quality,
            mic_index,
            mic_name,
            window_handle,
            crop,
            click_style,
            ready_worker,
        )
        .map(|_| path)
        .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    s.stop = Some(stop);
    s.receiver = Some(rx);
    s.ready = Some(ready);
    s.started = None;
    set(s.button, "Stop & save");
    set(
        s.status,
        if mic_index.is_some() {
            "Starting…  ·  Mic on"
        } else {
            "Starting…  ·  Mic off"
        },
    );
    layout(s);
    unsafe {
        EnableWindow(s.source, 0);
        EnableWindow(s.quality, 0);
        EnableWindow(s.microphone, 0);
        EnableWindow(s.click_style, 0);
        EnableWindow(s.folder, 0);
        EnableWindow(GetDlgItem(s.window, 102), 0);
        EnableWindow(GetDlgItem(s.window, 105), 0);
        EnableWindow(GetDlgItem(s.window, 106), 0);
    }
}
fn choose_area(hwnd: Handle) {
    let index = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let s = state.as_mut()?;
        if s.mode == 1 {
            s.mode = 0;
            refresh_picker(s);
        }
        s.mode = 2;
        s.area = None;
        let index = unsafe { SendMessageW(s.source, 0x147, 0, 0) };
        update_summary(s);
        (index >= 0 && (index as usize) < s.monitor_ids.len()).then_some(index as usize)
    });
    let Some(index) = index else {
        return;
    };
    let monitor = Monitor::enumerate()
        .ok()
        .and_then(|list| list.get(index).copied());
    unsafe {
        EnableWindow(hwnd, 0);
    }
    let selected = monitor.as_ref().and_then(crate::area::select);
    let alive = unsafe { IsWindow(hwnd) != 0 };
    if !alive {
        unsafe {
            PostQuitMessage(0);
        }
        return;
    }
    unsafe {
        EnableWindow(hwnd, 1);
        SetForegroundWindow(hwnd);
        SetActiveWindow(hwnd);
        SetFocus(GetDlgItem(hwnd, 112));
    }
    STATE.with(|cell| {
        if let Some(s) = cell.borrow_mut().as_mut() {
            s.area = selected.map(|area| (index, area));
            if let Some(area) = selected {
                set(s.status, &format!("Area: {} × {}", area.2, area.3));
            } else {
                set(s.status, "Select an area to record");
            }
            update_summary(s);
        }
    });
}
fn area_ready(mode: u8, has_area: bool) -> bool {
    mode != 2 || has_area
}
fn click(s: &mut State) {
    if let Some(stop) = &s.stop {
        stop.store(true, Ordering::Relaxed);
        s.started = None;
        s.ready = None;
        set(
            s.status,
            if unsafe { SendMessageW(s.microphone, 0x147, 0, 0) } > 0 {
                "Saving…  ·  Mic on"
            } else {
                "Saving…  ·  Mic off"
            },
        );
        set(s.button, "Saving…");
        unsafe {
            EnableWindow(s.button, 0);
        }
    } else if s.countdown.is_some() {
        s.countdown = None;
        unsafe {
            KillTimer(s.window, 1);
        }
        set(s.button, "Start recording");
        set(s.status, "Ready");
        layout(s);
    } else {
        if !area_ready(s.mode, s.area.is_some()) {
            message(s.window, "Select an area before recording.");
            return;
        }
        if unsafe { SetTimer(s.window, 1, 200, std::ptr::null()) } == 0 {
            message(s.window, "Could not start the recording timer.");
            return;
        }
        s.countdown = Some(Instant::now());
        set(s.button, "Cancel");
        set(s.status, "Recording starts in 3…");
        layout(s);
    }
}
#[allow(clippy::collapsible_if)]
unsafe extern "system" fn wnd(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        // WM_NCACTIVATE: keep the frame drawn active so DWM keeps the acrylic
        // translucent while another app has focus. Activation itself still
        // changes normally; only the frame rendering is affected.
        0x0086 if ACRYLIC.with(|cell| cell.get()) => unsafe { DefWindowProcW(hwnd, msg, 1, l) },
        0x001a => {
            update_appearance(hwnd);
            0
        }
        0x02e0 => {
            if l != 0 {
                let rect = unsafe { &*(l as *const Rect) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        0,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        0x0004 | 0x0010,
                    );
                }
            }
            let initialized =
                STATE.with(|cell| cell.try_borrow().ok().is_some_and(|state| state.is_some()));
            if !initialized {
                return 0;
            }
            let dpi = ((w >> 16) as u32).max(96);
            reset_fonts(hwnd, dpi);
            STATE.with(|cell| {
                if let Ok(state) = cell.try_borrow()
                    && let Some(s) = state.as_ref()
                {
                    layout(s);
                }
            });
            0
        }
        0x002b => {
            if l != 0 {
                draw_button(unsafe { &*(l as *const DrawItem) });
            }
            1
        }
        0x0133 | 0x0134 | 0x0135 | 0x0138 => {
            let dc = w as Handle;
            unsafe {
                let id = GetDlgCtrlID(l as Handle);
                let color = if !high_contrast() && [204, 208, 209, 211].contains(&id) {
                    0x00bcb5b1
                } else {
                    fg_color()
                };
                SetTextColor(dc, color);
                SetBkColor(dc, bg_color());
            }
            BRUSH.with(|brush| brush.get())
        }
        // WM_ERASEBKGND
        0x0014 => {
            let dc = w as Handle;
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            unsafe {
                GetClientRect(hwnd, &mut rect);
                if crate::glass::enabled() {
                    let dpi = GetDpiForWindow(hwnd).max(96);
                    let (w, h) = (rect.right, rect.bottom);
                    if crate::glass::paint(dc, w, h, dpi, |c| paint_backdrop(c, w as f32, h as f32))
                    {
                        return 1;
                    }
                }
                BRUSH.with(|brush| FillRect(dc, &rect, brush.get()));
                if VIEW.with(|v| v.get()) == View::Panel {
                    let dpi = GetDpiForWindow(hwnd).max(96) as i32;
                    let k = |n: i32| n * dpi / 96;
                    let color = if high_contrast() {
                        GetSysColor(15)
                    } else {
                        0x002d2a29
                    };
                    let brush = CreateSolidBrush(color);
                    let pen = CreatePen(0, 1, color);
                    let oldb = SelectObject(dc, brush);
                    let oldp = SelectObject(dc, pen);
                    RoundRect(dc, k(24), k(16), k(520), k(60), k(12), k(12));
                    RoundRect(dc, k(24), k(220), k(520), k(280), k(12), k(12));
                    SelectObject(dc, oldp);
                    SelectObject(dc, oldb);
                    DeleteObject(pen);
                    DeleteObject(brush);
                }
            }
            1
        }
        0x111 => {
            let id = w & 0xffff;
            if id == 112 || id == 117 {
                choose_area(hwnd);
                return 0;
            }
            if id == 102 || id == 116 {
                if let Some(path) = browse_folder(hwnd) {
                    STATE.with(|cell| {
                        if let Ok(state) = cell.try_borrow()
                            && let Some(s) = state.as_ref()
                        {
                            set(s.folder, &path);
                            update_summary(s);
                            save(s);
                        }
                    });
                }
                return 0;
            }
            STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut()
                    && let Some(s) = state.as_mut()
                {
                    match id {
                        207 if (w >> 16) == 1 => {
                            let selected = unsafe { SendMessageW(s.picker,0x147,0,0) };
                            if selected >= 0 { unsafe { SendMessageW(s.source,0x14e, selected as usize + if s.mode == 1 { s.monitor_ids.len() } else { 0 },0); } }
                            s.area = None;
                            if s.mode == 2 { set(s.status, "Select Area again for this screen"); }
                            update_summary(s);
                        }
                        110 | 111 => { s.mode = if id == 110 { 0 } else { 1 }; s.area = None; refresh_picker(s); update_summary(s); set(s.status,"Ready"); }
                        113 => {
                            let selected = unsafe { SendMessageW(s.microphone,0x147,0,0) };
                            if selected > 0 { s.last_mic=selected as usize; unsafe { SendMessageW(s.microphone,0x14e,0,0); } }
                            else if s.microphone_names.is_empty() { message(hwnd,"No microphone was found. Check Windows microphone access and reconnect the device."); }
                            else { unsafe { SendMessageW(s.microphone,0x14e,s.last_mic.min(s.microphone_names.len()).max(1),0); } }
                            update_summary(s);
                        }
                        114 => { s.settings_open = true; layout(s); }
                        115 => { s.settings_open = false; update_summary(s); save(s); layout(s); }
                        202 | 205 | 206 if (w >> 16) == 1 => { update_summary(s); }
                        101 => click(s),
                        104 => {
                            if let Some(path) = &s.last {
                                if !path.is_file() {
                                    message(hwnd, "The saved video was moved or deleted.");
                                } else {
                                    let result = unsafe {
                                        ShellExecuteW(
                                            hwnd,
                                            wide("open").as_ptr(),
                                            wide(&path.to_string_lossy()).as_ptr(),
                                            std::ptr::null(),
                                            std::ptr::null(),
                                            1,
                                        )
                                    };
                                    if result <= 32 {
                                        message(hwnd, "Windows could not open the saved video. Check your default video player.");
                                    }
                                }
                            }
                        }
                        103 => {
                            if let Some(path) = &s.last
                                && let Err(error) = reveal(path)
                            {
                                message(hwnd, &error);
                            }
                        }
                        _ => {}
                    }
                }
            });
            0
        }
        0x113 => {
            STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut()
                    && let Some(s) = state.as_mut()
                {
                    if let Some(t) = s.countdown {
                        let n = t.elapsed().as_secs();
                        if n >= 3 {
                            s.countdown = None;
                            begin(s);
                            if s.stop.is_none() {
                                unsafe {
                                    KillTimer(hwnd, 1);
                                }
                                layout(s);
                            }
                        } else {
                            set(s.status, &format!("Recording starts in {}…", 3 - n));
                        }
                    }
                    if s.started.is_none()
                        && s.stop.is_some()
                        && s.ready
                            .as_ref()
                            .is_some_and(|ready| ready.load(Ordering::Relaxed))
                    {
                        s.started = Some(Instant::now());
                        s.ready = None;
                    }
                    if let Some(t) = s.started {
                        let elapsed = t.elapsed().as_secs();
                        if s.stop.as_ref().is_some_and(|v| !v.load(Ordering::Relaxed)) {
                            set(
                                s.status,
                                &format!(
                                    "Recording  {:02}:{:02}  ·  Mic {}",
                                    elapsed / 60,
                                    elapsed % 60,
                                    if unsafe { SendMessageW(s.microphone, 0x147, 0, 0) } > 0 {
                                        "on"
                                    } else {
                                        "off"
                                    }
                                ),
                            );
                        }
                    }
                    if let Some(rx) = &s.receiver {
                        if let Ok(result) = rx.try_recv() {
                            s.receiver = None;
                            unsafe {
                                KillTimer(hwnd, 1);
                            }
                            if s.hotkey {
                                unsafe {
                                    UnregisterHotKey(hwnd, 1);
                                }
                                s.hotkey = false;
                            }
                            unsafe {
                                SetWindowDisplayAffinity(hwnd, 0);
                            }
                            s.stop = None;
                            s.started = None;
                            s.ready = None;
                            set(s.button, "Start recording");
                            unsafe {
                                EnableWindow(s.button, 1);
                                EnableWindow(s.source, 1);
                                EnableWindow(s.quality, 1);
                                EnableWindow(s.microphone, 1);
                                EnableWindow(s.click_style, 1);
                                EnableWindow(s.folder, 1);
                                EnableWindow(GetDlgItem(hwnd, 102), 1);
                                EnableWindow(GetDlgItem(hwnd, 105), 1);
                                EnableWindow(GetDlgItem(hwnd, 106), 1);
                            }
                            if s.pending_close {
                                unsafe {
                                    PostMessageW(hwnd, 0x10, 0, 0);
                                }
                            }
                            match result {
                                Ok(path) => {
                                    s.last = Some(path);
                                    set(s.status, "Saved successfully");
                                    unsafe {
                                        EnableWindow(s.open, 1);
                                        EnableWindow(GetDlgItem(hwnd, 104), 1);
                                    }
                                }
                                Err(e) => {
                                    set(s.status, "Recording failed");
                                    message(hwnd, &e);
                                }
                            }
                            update_summary(s);
                            layout(s);
                        }
                    }
                }
            });
            0
        }
        0x312 => {
            STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut()
                    && let Some(s) = state.as_mut()
                    && s.stop.is_some()
                    && (s.ready.is_some() || s.started.is_some())
                {
                    click(s)
                }
            });
            0
        }
        0x10 => {
            let busy = STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut()
                    && let Some(s) = state.as_mut()
                    && s.stop.is_some()
                {
                    s.pending_close = true;
                    if s.started.is_some() || s.ready.is_some() {
                        click(s);
                    }
                    true
                } else {
                    false
                }
            });
            if !busy {
                unsafe {
                    DestroyWindow(hwnd);
                }
            }
            0
        }
        0x2 => {
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().take() {
                    save(&s);
                    if s.hotkey {
                        unsafe { UnregisterHotKey(hwnd, 1) };
                    }
                }
            });
            unsafe {
                KillTimer(hwnd, 1);
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        SetProcessDpiAwarenessContext(-4);
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("FrameLatchWindow");
        let brush = CreateSolidBrush(bg_color());
        BRUSH.with(|v| v.set(brush));
        let wc = WndClass {
            style: 0,
            proc: Some(wnd),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: LoadIconW(instance, std::ptr::without_provenance::<u16>(1)),
            cursor: LoadCursorW(0, 32512usize as *const u16),
            background: brush,
            menu: std::ptr::null(),
            class: class.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            return Err("Failed to register window".into());
        }
        let dpi = GetDpiForSystem().max(96) as i32;
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("FrameLatch").as_ptr(),
            0x00ca0000,
            0x80000000u32 as i32,
            0x80000000u32 as i32,
            510 * dpi / 96,
            550 * dpi / 96,
            0,
            0,
            instance,
            std::ptr::null(),
        );
        if hwnd == 0 {
            return Err("Failed to create window".into());
        }
        update_appearance(hwnd);
        let icon_dpi = GetDpiForWindow(hwnd).max(96);
        let small_icon = LoadImageW(
            instance,
            std::ptr::without_provenance::<u16>(1),
            1,
            (16 * icon_dpi / 96) as i32,
            (16 * icon_dpi / 96) as i32,
            0,
        );
        let large_icon = LoadImageW(
            instance,
            std::ptr::without_provenance::<u16>(1),
            1,
            (32 * icon_dpi / 96) as i32,
            (32 * icon_dpi / 96) as i32,
            0,
        );
        if small_icon != 0 {
            SendMessageW(hwnd, 0x80, 0, small_icon);
        }
        if large_icon != 0 {
            SendMessageW(hwnd, 0x80, 1, large_icon);
        }
        let corners: i32 = 2;
        DwmSetWindowAttribute(hwnd, 33, (&corners as *const i32).cast(), 4);
        let dpi = GetDpiForWindow(hwnd).max(96);
        reset_fonts(hwnd, dpi);
        let source = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            0,
            0,
            10,
            10,
            201,
        );
        ShowWindow(source, 0);
        let monitors = Monitor::enumerate()?;
        let monitor_ids: Vec<String> = monitors
            .iter()
            .map(|m| m.device_name().unwrap_or_default())
            .collect();
        let monitor_sizes: Vec<(u32, u32)> = monitors
            .iter()
            .map(|m| (m.width().unwrap_or(0), m.height().unwrap_or(0)))
            .collect();
        for (i, m) in monitors.iter().enumerate() {
            let name = m.name().unwrap_or_default();
            let name = if name.is_empty() {
                format!("Monitor {}", i + 1)
            } else {
                name
            };
            SendMessageW(source, 0x143, 0, wide(&name).as_ptr() as isize);
        }
        let windows: Vec<(usize, u32)> = windows_capture::window::Window::enumerate()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|window| {
                let title = window.title().ok()?;
                let pid = window.process_id().ok()?;
                if title.trim().is_empty() || pid == std::process::id() || !window.is_valid() {
                    return None;
                }
                SendMessageW(source, 0x143, 0, wide(&title).as_ptr() as isize);
                Some((window.as_raw_hwnd() as usize, pid))
            })
            .collect();
        control(hwnd, instance, "BUTTON", "Screen", 0, 20, 15, 122, 31, 110);
        control(hwnd, instance, "BUTTON", "Window", 0, 154, 15, 122, 31, 111);
        control(hwnd, instance, "BUTTON", "Area", 0, 288, 15, 122, 31, 112);
        let picker = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            20,
            57,
            390,
            200,
            207,
        );
        control(
            hwnd,
            instance,
            "BUTTON",
            "Microphone: off",
            0,
            20,
            101,
            190,
            32,
            113,
        );
        control(
            hwnd, instance, "BUTTON", "Settings", 0, 220, 101, 190, 32, 114,
        );
        control(hwnd, instance, "STATIC", "", 0x0d, 20, 84, 300, 20, 211);
        control(
            hwnd,
            instance,
            "BUTTON",
            "Change selected area",
            0,
            325,
            82,
            85,
            24,
            117,
        );
        control(hwnd, instance, "STATIC", "", 0x0d, 20, 179, 18, 18, 210);
        control(
            hwnd,
            instance,
            "BUTTON",
            "Change save folder",
            0,
            320,
            175,
            90,
            25,
            116,
        );
        control(hwnd, instance, "STATIC", "", 0x0d, 20, 143, 390, 20, 208);
        control(hwnd, instance, "STATIC", "", 0x0d, 20, 171, 390, 20, 209);
        let button = control(
            hwnd,
            instance,
            "BUTTON",
            "Start recording",
            0,
            20,
            203,
            390,
            40,
            101,
        );
        let status = control(
            hwnd, instance, "STATIC", "Ready", 0x0d, 20, 251, 390, 23, 204,
        );
        let open = control(
            hwnd,
            instance,
            "BUTTON",
            "Show in folder",
            0,
            20,
            278,
            190,
            28,
            103,
        );
        let open_video = control(
            hwnd,
            instance,
            "BUTTON",
            "Open video",
            0,
            220,
            278,
            190,
            28,
            104,
        );
        EnableWindow(open, 0);
        EnableWindow(open_video, 0);
        control(hwnd, instance, "BUTTON", "← Back", 0, 20, 15, 115, 30, 115);
        control(
            hwnd,
            instance,
            "STATIC",
            "Quality and orientation",
            0x0d,
            20,
            58,
            390,
            20,
            301,
        );
        let quality = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            20,
            80,
            390,
            200,
            202,
        );
        for label in [
            "Standard · 1080p",
            "Small file · 720p",
            "Portrait 1080 × 1920 · fit with bars",
            "Portrait 720 × 1280 · fit with bars",
            "High · up to 1440p",
            "Ultra · up to 4K",
            "Original · native resolution",
        ] {
            SendMessageW(quality, 0x143, 0, wide(label).as_ptr() as isize);
        }
        control(
            hwnd,
            instance,
            "STATIC",
            "Microphone device",
            0x0d,
            20,
            123,
            390,
            20,
            302,
        );
        let microphone = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            20,
            145,
            390,
            200,
            205,
        );
        SendMessageW(microphone, 0x143, 0, wide("Off").as_ptr() as isize);
        let microphone_names = crate::audio::devices().unwrap_or_default();
        for name in &microphone_names {
            SendMessageW(microphone, 0x143, 0, wide(name).as_ptr() as isize);
        }
        control(
            hwnd,
            instance,
            "STATIC",
            "Mouse clicks",
            0x0d,
            20,
            188,
            390,
            20,
            303,
        );
        let click_style = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            20,
            210,
            390,
            200,
            206,
        );
        for label in ["Off", "Red ring", "Red ripple"] {
            SendMessageW(click_style, 0x143, 0, wide(label).as_ptr() as isize);
        }
        control(
            hwnd,
            instance,
            "STATIC",
            "Save folder",
            0x0d,
            20,
            253,
            390,
            20,
            304,
        );
        let folder = control(
            hwnd,
            instance,
            "EDIT",
            "",
            0x00800000 | 0x80,
            20,
            275,
            290,
            27,
            203,
        );
        control(
            hwnd,
            instance,
            "BUTTON",
            "Browse…",
            0,
            320,
            275,
            90,
            27,
            102,
        );
        let mut saved = std::fs::read_to_string(settings_path())
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let versioned = saved
            .first()
            .is_some_and(|line| line == "FrameLatchSettings=1");
        let unknown_version = saved.first().is_some_and(|line| {
            line.starts_with("FrameLatchSettings=") && line != "FrameLatchSettings=1"
        });
        if versioned {
            saved.remove(0);
        }
        let invalid_settings = unknown_version
            || (!saved.is_empty() && !(3..=5).contains(&saved.len()))
            || (versioned && saved.len() != 5)
            || saved.get(2).is_some_and(|v| {
                v.parse::<usize>()
                    .ok()
                    .and_then(crate::quality::Quality::from_index)
                    .is_none()
            })
            || saved
                .get(4)
                .is_some_and(|v| v.parse::<usize>().map_or(true, |n| n > 2));
        if invalid_settings {
            saved.clear();
            message(hwnd, "Saved settings are invalid; defaults will be used.");
        }
        while saved.len() < 5 {
            saved.push(String::new())
        }
        let default = default_folder();
        set(
            folder,
            if saved[0].is_empty() {
                default.to_str().unwrap_or(".")
            } else {
                &saved[0]
            },
        );
        let count = SendMessageW(source, 0x146, 0, 0);
        let selected = monitor_ids
            .iter()
            .position(|id| !id.is_empty() && id == &saved[1])
            .unwrap_or(0) as isize;
        SendMessageW(
            source,
            0x14e,
            if selected >= 0 && selected < count {
                selected as usize
            } else {
                0
            },
            0,
        );
        SendMessageW(
            quality,
            0x14e,
            saved[2]
                .parse::<usize>()
                .ok()
                .filter(|n| crate::quality::Quality::from_index(*n).is_some())
                .unwrap_or(0),
            0,
        );
        let saved_mic = if !saved[3].is_empty()
            && microphone_names
                .iter()
                .filter(|name| *name == &saved[3])
                .count()
                == 1
        {
            microphone_names
                .iter()
                .position(|name| name == &saved[3])
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        };
        SendMessageW(microphone, 0x14e, saved_mic, 0);
        SendMessageW(
            click_style,
            0x14e,
            saved[4].parse::<usize>().unwrap_or(0).min(2),
            0,
        );
        let hotkey = false;
        STATE.with(|cell| {
            *cell.borrow_mut() = Some(State {
                window: hwnd,
                source,
                quality,
                microphone,
                click_style,
                microphone_names,
                folder,
                button,
                status,
                open,
                stop: None,
                receiver: None,
                started: None,
                ready: None,
                countdown: None,
                last: None,
                hotkey,
                pending_close: false,
                monitor_ids,
                monitor_sizes,
                windows,
                area: None,
                picker,
                mode: 0,
                settings_open: false,
                last_mic: saved_mic.max(1),
            })
        });
        STATE.with(|cell| {
            if let Some(s) = cell.borrow_mut().as_mut() {
                refresh_picker(s);
                update_summary(s);
                layout(s);
            }
        });
        ShowWindow(hwnd, 1);
        UpdateWindow(hwnd);
        let mut msg: Msg = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0, 0, 0);
            if r <= 0 {
                break;
            }
            if msg.message == 0x100 && msg.wparam == 27 {
                let cancelled = STATE.with(|cell| {
                    if let Ok(mut state) = cell.try_borrow_mut()
                        && let Some(s) = state.as_mut()
                        && s.countdown.is_some()
                    {
                        click(s);
                        true
                    } else {
                        false
                    }
                });
                if cancelled {
                    continue;
                }
            }
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        for font in [
            FONT.with(|f| f.replace(0)),
            DETAIL_FONT.with(|f| f.replace(0)),
            PRIMARY_FONT.with(|f| f.replace(0)),
        ] {
            if font != 0 {
                DeleteObject(font);
            }
        }
        if small_icon != 0 {
            DestroyIcon(small_icon);
        }
        if large_icon != 0 {
            DestroyIcon(large_icon);
        }
        BRUSH.with(|b| {
            let brush = b.replace(0);
            if brush != 0 {
                DeleteObject(brush);
            }
        });
        crate::glass::shutdown();
        CoUninitialize();
        Ok(())
    }
}

#[cfg(test)]
mod ui_tests {
    use super::{View, area_ready, glass_view, source_detail, view};
    #[test]
    fn active_session_always_uses_toolbar() {
        assert_eq!(view(false, false), View::Panel);
        assert_eq!(view(true, false), View::Settings);
        assert_eq!(view(false, true), View::Toolbar);
        assert_eq!(view(true, true), View::Toolbar);
    }
    #[test]
    fn every_view_is_owner_drawn_for_glass() {
        assert!(glass_view(View::Settings));
        assert!(glass_view(View::Panel));
        assert!(glass_view(View::Toolbar));
    }
    #[test]
    fn area_summary_never_uses_full_screen_dimensions() {
        assert_eq!(
            source_detail(2, Some((10, 20, 800, 500)), Some((3000, 1876))),
            "Selected area · 800 × 500"
        );
        assert_eq!(
            source_detail(2, None, Some((3000, 1876))),
            "Select an area to record"
        );
        assert_eq!(
            source_detail(0, None, Some((3000, 1876))),
            "Screen · 3000 × 1876"
        );
    }
    #[test]
    fn cancelled_area_cannot_fall_back_to_full_screen() {
        assert!(!area_ready(2, false));
        assert!(area_ready(2, true));
        assert!(area_ready(0, false));
        assert!(area_ready(1, false));
    }
}
