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
    fn SetProcessDpiAwarenessContext(context: isize) -> i32;
    fn GetDpiForWindow(hwnd: Handle) -> u32;
    fn GetDpiForSystem() -> u32;
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
}
thread_local! { static FONT: std::cell::Cell<Handle> = const { std::cell::Cell::new(0) }; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn set(hwnd: Handle, s: &str) {
    unsafe {
        SetWindowTextW(hwnd, wide(s).as_ptr());
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
            0x50000000 | style | if cls == "STATIC" { 0 } else { 0x00010000 },
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
    FONT.with(|font| unsafe {
        SendMessageW(hwnd, 0x30, font.get() as usize, 1);
    });
    hwnd
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
    windows: Vec<(usize, u32)>,
    area: Option<(usize, (u32, u32, u32, u32))>,
}
thread_local! { static STATE:RefCell<Option<State>>=const {RefCell::new(None)}; }
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
    set(s.button, "Record");
    set(s.status, "Ready");
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
    let quality = unsafe { SendMessageW(s.quality, 0x147, 0, 0) };
    let small = quality == 1 || quality == 3;
    let vertical = quality == 2 || quality == 3;
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
            small,
            mic_index,
            mic_name,
            window_handle,
            crop,
            click_style,
            vertical,
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
    set(s.button, "Stop recording");
    set(s.status, "Starting capture…");
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
fn click(s: &mut State) {
    if let Some(stop) = &s.stop {
        stop.store(true, Ordering::Relaxed);
        s.started = None;
        s.ready = None;
        set(s.status, "Saving recording…");
        set(s.button, "Saving…");
        unsafe {
            EnableWindow(s.button, 0);
        }
    } else if s.countdown.is_some() {
        s.countdown = None;
        set(s.button, "Record");
        set(s.status, "Ready");
    } else {
        s.countdown = Some(Instant::now());
        set(s.button, "Cancel");
        set(s.status, "Recording starts in 3…");
    }
}
#[allow(clippy::collapsible_if)]
unsafe extern "system" fn wnd(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        0x111 => {
            let id = w & 0xffff;
            STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut()
                    && let Some(s) = state.as_mut()
                {
                    match id {
                        201 if (w >> 16) == 1 => {
                            s.area = None;
                            set(s.status, "Full source selected");
                        }
                        101 => click(s),
                        105 => {
                            let index = unsafe { SendMessageW(s.source, 0x147, 0, 0) };
                            if index < 0 || index as usize >= s.monitor_ids.len() {
                                message(hwnd, "Select a screen before choosing an area.");
                            } else if let Ok(monitors) = Monitor::enumerate()
                                && let Some(monitor) = monitors.get(index as usize)
                            {
                                if let Some(area) = crate::area::select(monitor) {
                                    s.area = Some((index as usize, area));
                                    set(
                                        s.status,
                                        &format!("Area selected: {} × {}", area.2, area.3),
                                    );
                                }
                            }
                        }
                        106 => {
                            s.area = None;
                            set(s.status, "Full source selected");
                        }
                        102 => {
                            if let Some(path) = browse_folder(hwnd) {
                                set(s.folder, &path);
                            }
                        }
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
                                &format!("Recording  {:02}:{:02}", elapsed / 60, elapsed % 60),
                            );
                        }
                    }
                    if let Some(rx) = &s.receiver {
                        if let Ok(result) = rx.try_recv() {
                            s.receiver = None;
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
                            set(s.button, "Record");
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
        let wc = WndClass {
            style: 0,
            proc: Some(wnd),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: LoadCursorW(0, 32512usize as *const u16),
            background: 6,
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
        let dpi = GetDpiForWindow(hwnd).max(96);
        let font = CreateFontW(
            -((14 * dpi / 96) as i32),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe UI").as_ptr(),
        );
        FONT.with(|f| f.set(font));
        control(
            hwnd,
            instance,
            "STATIC",
            "Screen or window",
            0,
            22,
            20,
            220,
            20,
            0,
        );
        let source = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            22,
            42,
            450,
            200,
            201,
        );
        let monitors = Monitor::enumerate()?;
        let monitor_ids: Vec<String> = monitors
            .iter()
            .map(|m| m.device_name().unwrap_or_default())
            .collect();
        for (i, m) in monitors.iter().enumerate() {
            let name = m.name().unwrap_or_default();
            let name = if name.is_empty() {
                format!("Monitor {}", i + 1)
            } else {
                name
            };
            let label = format!(
                "{}  ({} × {})",
                name,
                m.width().unwrap_or(0),
                m.height().unwrap_or(0)
            );
            SendMessageW(source, 0x143, 0, wide(&label).as_ptr() as isize);
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
                let label = format!("Window - {title}");
                SendMessageW(source, 0x143, 0, wide(&label).as_ptr() as isize);
                Some((window.as_raw_hwnd() as usize, pid))
            })
            .collect();
        control(hwnd, instance, "STATIC", "Quality", 0, 22, 84, 100, 20, 0);
        let quality = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            22,
            106,
            450,
            200,
            202,
        );
        for label in [
            "Standard · 1080p",
            "Small file · 720p",
            "Portrait 1080 × 1920 · fit with bars",
            "Portrait 720 × 1280 · fit with bars",
        ] {
            SendMessageW(quality, 0x143, 0, wide(label).as_ptr() as isize);
        }
        control(
            hwnd,
            instance,
            "STATIC",
            "Microphone",
            0,
            22,
            148,
            150,
            20,
            0,
        );
        let microphone = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            22,
            170,
            450,
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
            "Save folder",
            0,
            22,
            212,
            100,
            20,
            0,
        );
        let folder = control(
            hwnd,
            instance,
            "EDIT",
            "",
            0x00800000 | 0x80,
            22,
            234,
            345,
            26,
            203,
        );
        control(
            hwnd,
            instance,
            "BUTTON",
            "Browse…",
            0,
            377,
            234,
            95,
            27,
            102,
        );
        let button = control(hwnd, instance, "BUTTON", "Record", 0, 22, 284, 215, 40, 101);
        let open = control(
            hwnd,
            instance,
            "BUTTON",
            "Show in folder",
            0,
            247,
            284,
            105,
            40,
            103,
        );
        let open_video = control(
            hwnd,
            instance,
            "BUTTON",
            "Open video",
            0,
            362,
            284,
            110,
            40,
            104,
        );
        EnableWindow(open_video, 0);
        EnableWindow(open, 0);
        control(
            hwnd,
            instance,
            "BUTTON",
            "Select area…",
            0,
            22,
            341,
            180,
            30,
            105,
        );
        control(
            hwnd,
            instance,
            "BUTTON",
            "Full source",
            0,
            212,
            341,
            140,
            30,
            106,
        );
        control(
            hwnd,
            instance,
            "STATIC",
            "Mouse clicks",
            0,
            22,
            385,
            120,
            20,
            0,
        );
        let click_style = control(
            hwnd,
            instance,
            "COMBOBOX",
            "",
            0x3 | 0x00200000,
            152,
            382,
            250,
            160,
            206,
        );
        for label in ["Off", "Red ring", "Red ripple"] {
            SendMessageW(click_style, 0x143, 0, wide(label).as_ptr() as isize);
        }
        let status = control(hwnd, instance, "STATIC", "Ready", 0, 22, 430, 440, 24, 204);
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
            || saved
                .get(2)
                .is_some_and(|v| v.parse::<usize>().map_or(true, |n| n > 3))
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
            saved[2].parse::<usize>().unwrap_or(0).min(3),
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
                windows,
                area: None,
            })
        });
        SetTimer(hwnd, 1, 200, std::ptr::null());
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
        if font != 0 {
            DeleteObject(font);
        }
        CoUninitialize();
        Ok(())
    }
}
