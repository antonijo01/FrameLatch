use std::cell::RefCell;
use std::ffi::c_void;
use windows_capture::monitor::Monitor;

type Handle = isize;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
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
    fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    fn SetLayeredWindowAttributes(hwnd: Handle, key: u32, alpha: u8, flags: u32) -> i32;
    fn ShowWindow(hwnd: Handle, how: i32) -> i32;
    fn UpdateWindow(hwnd: Handle) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn DestroyWindow(hwnd: Handle) -> i32;
    fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    fn SetCapture(hwnd: Handle) -> Handle;
    fn ReleaseCapture() -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn InvalidateRect(hwnd: Handle, rect: *const Rect, erase: i32) -> i32;
    fn GetDC(hwnd: Handle) -> Handle;
    fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreatePen(style: i32, width: i32, color: u32) -> Handle;
    fn SelectObject(dc: Handle, obj: Handle) -> Handle;
    fn GetStockObject(index: i32) -> Handle;
    fn Rectangle(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn DeleteObject(obj: Handle) -> i32;
}
struct Selection {
    origin: Point,
    width: i32,
    height: i32,
    start: Option<Point>,
    current: Option<Point>,
    result: Option<(u32, u32, u32, u32)>,
    done: bool,
}
thread_local! { static SELECT: RefCell<Option<Selection>> = const { RefCell::new(None) }; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn clamp_position(position: i32, origin: i32, extent: i32) -> i32 {
    position.saturating_sub(origin).clamp(0, extent.max(0))
}
#[allow(clippy::collapsible_if)]
unsafe extern "system" fn window(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        0x201 => {
            unsafe {
                SetCapture(hwnd);
            }
            SELECT.with(|s| {
                if let Some(s) = s.borrow_mut().as_mut() {
                    let mut p = Point::default();
                    unsafe {
                        GetCursorPos(&mut p);
                    }
                    p.x = clamp_position(p.x, s.origin.x, s.width);
                    p.y = clamp_position(p.y, s.origin.y, s.height);
                    s.start = Some(p);
                    s.current = Some(p);
                }
            });
            0
        }
        0x200 => {
            SELECT.with(|s| {
                if let Some(s) = s.borrow_mut().as_mut() {
                    if s.start.is_some() {
                        let mut p = Point::default();
                        unsafe {
                            GetCursorPos(&mut p);
                        }
                        p.x = clamp_position(p.x, s.origin.x, s.width);
                        p.y = clamp_position(p.y, s.origin.y, s.height);
                        s.current = Some(p);
                        unsafe {
                            InvalidateRect(hwnd, std::ptr::null(), 1);
                        }
                    }
                }
            });
            0
        }
        0x202 => {
            unsafe {
                ReleaseCapture();
            }
            SELECT.with(|s| {
                if let Some(s) = s.borrow_mut().as_mut() {
                    if let (Some(a), Some(b)) = (s.start, s.current) {
                        let x = a.x.min(b.x).max(0);
                        let y = a.y.min(b.y).max(0);
                        let width = (a.x.max(b.x) - x).max(0) as u32;
                        let height = (a.y.max(b.y) - y).max(0) as u32;
                        if width >= 16 && height >= 16 {
                            s.result = Some((x as u32, y as u32, width, height));
                        }
                    }
                    s.done = true;
                }
            });
            0
        }
        0x100 if w == 27 => {
            SELECT.with(|s| {
                if let Some(s) = s.borrow_mut().as_mut() {
                    s.done = true;
                }
            });
            0
        }
        0x0f => {
            let result = unsafe { DefWindowProcW(hwnd, msg, w, l) };
            SELECT.with(|s| {
                if let Some(s) = s.borrow().as_ref() {
                    if let (Some(a), Some(b)) = (s.start, s.current) {
                        unsafe {
                            let dc = GetDC(hwnd);
                            let pen = CreatePen(0, 3, 0x000000ff);
                            let old = SelectObject(dc, pen);
                            let brush = SelectObject(dc, GetStockObject(5));
                            Rectangle(dc, a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                            SelectObject(dc, brush);
                            SelectObject(dc, old);
                            DeleteObject(pen);
                            ReleaseDC(hwnd, dc);
                        }
                    }
                }
            });
            result
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}
pub fn origin(monitor: &Monitor) -> Option<(i32, i32)> {
    let mut info = MonitorInfo {
        size: std::mem::size_of::<MonitorInfo>() as u32,
        monitor: Rect::default(),
        work: Rect::default(),
        flags: 0,
    };
    (unsafe { GetMonitorInfoW(monitor.as_raw_hmonitor(), &mut info) } != 0)
        .then_some((info.monitor.left, info.monitor.top))
}
pub fn select(monitor: &Monitor) -> Option<(u32, u32, u32, u32)> {
    unsafe {
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            monitor: Rect::default(),
            work: Rect::default(),
            flags: 0,
        };
        if GetMonitorInfoW(monitor.as_raw_hmonitor(), &mut info) == 0 {
            return None;
        }
        let r = info.monitor;
        SELECT.with(|s| {
            *s.borrow_mut() = Some(Selection {
                origin: Point {
                    x: r.left,
                    y: r.top,
                },
                width: r.right - r.left,
                height: r.bottom - r.top,
                start: None,
                current: None,
                result: None,
                done: false,
            })
        });
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("FrameLatchAreaSelection");
        let wc = WndClass {
            style: 0,
            proc: Some(window),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: LoadCursorW(0, 32515usize as *const u16),
            background: 6,
            menu: std::ptr::null(),
            class: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            0x00080000 | 0x00000008 | 0x00000080,
            class.as_ptr(),
            wide("Drag to select; Escape cancels").as_ptr(),
            0x80000000,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            0,
            0,
            instance,
            std::ptr::null(),
        );
        if hwnd == 0 {
            return None;
        }
        SetLayeredWindowAttributes(hwnd, 0, 90, 2);
        ShowWindow(hwnd, 1);
        UpdateWindow(hwnd);
        let mut msg: Msg = std::mem::zeroed();
        while !SELECT.with(|s| s.borrow().as_ref().is_none_or(|s| s.done)) {
            if GetMessageW(&mut msg, 0, 0, 0) <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        DestroyWindow(hwnd);
        SELECT.with(|s| s.borrow_mut().take().and_then(|s| s.result))
    }
}

#[cfg(test)]
mod tests {
    use super::clamp_position;
    #[test]
    fn coordinates_are_clamped_to_selected_monitor() {
        assert_eq!(clamp_position(-2000, -1920, 1920), 0);
        assert_eq!(clamp_position(-1000, -1920, 1920), 920);
        assert_eq!(clamp_position(200, -1920, 1920), 1920);
    }
}
