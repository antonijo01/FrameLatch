use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
type Handle = isize;
#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct MouseHook {
    point: Point,
    data: u32,
    flags: u32,
    time: u32,
    extra: usize,
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
#[link(name = "user32")]
unsafe extern "system" {
    fn SetWindowsHookExW(
        kind: i32,
        proc: Option<unsafe extern "system" fn(i32, usize, isize) -> isize>,
        module: Handle,
        tid: u32,
    ) -> Handle;
    fn UnhookWindowsHookEx(hook: Handle) -> i32;
    fn CallNextHookEx(hook: Handle, code: i32, w: usize, l: isize) -> isize;
    fn GetMessageW(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
    fn PeekMessageW(msg: *mut Msg, hwnd: Handle, min: u32, max: u32, remove: u32) -> i32;
    fn WindowFromPoint(point: Point) -> Handle;
    fn GetAncestor(hwnd: Handle, flags: u32) -> Handle;
    fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn PostThreadMessageW(tid: u32, msg: u32, w: usize, l: isize) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
    fn GetModuleHandleW(name: *const u16) -> Handle;
}
type Event = (i32, i32, Instant);
static CLICK: OnceLock<Mutex<Option<Event>>> = OnceLock::new();
static TARGET: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
unsafe extern "system" fn callback(code: i32, w: usize, l: isize) -> isize {
    if code >= 0 && (w == 0x201 || w == 0x204 || w == 0x207) {
        let event = unsafe { &*(l as *const MouseHook) };
        let target = TARGET.load(std::sync::atomic::Ordering::Relaxed);
        let mut x = event.point.x;
        let mut y = event.point.y;
        if target != 0 {
            let hit = unsafe { WindowFromPoint(event.point) };
            let root = unsafe { GetAncestor(hit, 2) };
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if root != target || unsafe { GetWindowRect(target, &mut rect) } == 0 {
                return unsafe { CallNextHookEx(0, code, w, l) };
            }
            x -= rect.left;
            y -= rect.top;
        }
        if let Ok(mut state) = CLICK.get_or_init(|| Mutex::new(None)).lock() {
            *state = Some((x, y, Instant::now()));
        }
    }
    unsafe { CallNextHookEx(0, code, w, l) }
}
pub struct Listener {
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Listener {
    pub fn start(window: Option<usize>) -> Result<Self, String> {
        TARGET.store(
            window.unwrap_or(0) as isize,
            std::sync::atomic::Ordering::Relaxed,
        );
        if let Ok(mut state) = CLICK.get_or_init(|| Mutex::new(None)).lock() {
            *state = None;
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || unsafe {
            let hook = SetWindowsHookExW(14, Some(callback), GetModuleHandleW(std::ptr::null()), 0);
            let tid = GetCurrentThreadId();
            if hook == 0 {
                let _ = tx.send(None);
                return;
            }
            let mut msg: Msg = std::mem::zeroed();
            PeekMessageW(&mut msg, 0, 0, 0, 0);
            let _ = tx.send(Some(tid));
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {}
            UnhookWindowsHookEx(hook);
        });
        let thread_id = rx
            .recv()
            .map_err(|_| "Cannot start mouse listener")?
            .ok_or("Cannot install mouse listener")?;
        Ok(Self {
            thread_id,
            thread: Some(thread),
        })
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        unsafe {
            PostThreadMessageW(self.thread_id, 0x12, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn current() -> Option<(i32, i32, Duration)> {
    let event = *CLICK.get_or_init(|| Mutex::new(None)).lock().ok()?;
    let (x, y, time) = event?;
    let elapsed = time.elapsed();
    (elapsed < Duration::from_millis(450)).then_some((x, y, elapsed))
}
