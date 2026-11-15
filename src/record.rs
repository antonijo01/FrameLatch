use std::{
    error::Error,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    },
    core::HSTRING,
};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn MoveFileW(existing: *const u16, new: *const u16) -> i32;
    fn GetDiskFreeSpaceExW(
        path: *const u16,
        available: *mut u64,
        total: *mut u64,
        free: *mut u64,
    ) -> i32;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn IsIconic(window: isize) -> i32;
}
fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn publish_no_replace(source: &Path, dest: &Path) -> std::io::Result<()> {
    if unsafe { MoveFileW(wide_path(source).as_ptr(), wide_path(dest).as_ptr()) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn has_disk_reserve(path: &Path) -> bool {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut free = 0u64;
    unsafe {
        GetDiskFreeSpaceExW(
            wide_path(directory).as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) != 0
            && free >= 100 * 1024 * 1024
    }
}
type CaptureError = Box<dyn Error + Send + Sync>;
type Shared = Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>;
pub struct Capture(Shared);
impl GraphicsCaptureApiHandler for Capture {
    type Flags = Shared;
    type Error = CaptureError;
    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self(ctx.flags))
    }
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let mut scratch = Vec::new();
        let width = frame.width();
        let height = frame.height();
        let buffer = frame.buffer()?;
        let data = buffer.as_nopadding_buffer(&mut scratch);
        let mut latest = self.0.lock().map_err(|_| "frame lock poisoned")?;
        match latest.as_mut() {
            Some((previous, w, h)) => {
                previous.clear();
                previous.extend_from_slice(data);
                *w = width;
                *h = height;
            }
            None => *latest = Some((data.to_vec(), width, height)),
        }
        Ok(())
    }
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
fn pair(a: u32, b: u32) -> u64 {
    (u64::from(a) << 32) | u64::from(b)
}
unsafe fn writer(
    path: &Path,
    width: u32,
    height: u32,
    small: bool,
    audio: Option<(u32, u16)>,
) -> windows::core::Result<(IMFSinkWriter, u32, Option<u32>)> {
    unsafe {
        let url = HSTRING::from(path.as_os_str());
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 2)?;
        let attributes = attributes.ok_or(windows::core::Error::from(windows::core::HRESULT(
            0x80004005u32 as i32,
        )))?;
        attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
        attributes.SetUINT32(&MF_LOW_LATENCY, 1)?;
        let sink = MFCreateSinkWriterFromURL(&url, None, &attributes)?;
        let output = MFCreateMediaType()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        output.SetUINT32(
            &MF_MT_AVG_BITRATE,
            if small { 3_000_000 } else { 8_000_000 },
        )?;
        output.SetUINT32(&MF_MT_INTERLACE_MODE, 2)?;
        output.SetUINT64(&MF_MT_FRAME_SIZE, pair(width, height))?;
        output.SetUINT64(&MF_MT_FRAME_RATE, pair(30, 1))?;
        output.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pair(1, 1))?;
        let stream = sink.AddStream(&output)?;
        let input = MFCreateMediaType()?;
        input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        input.SetUINT32(&MF_MT_INTERLACE_MODE, 2)?;
        input.SetUINT64(&MF_MT_FRAME_SIZE, pair(width, height))?;
        input.SetUINT64(&MF_MT_FRAME_RATE, pair(30, 1))?;
        input.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pair(1, 1))?;
        sink.SetInputMediaType(stream, &input, None)?;
        let audio_stream = if let Some((rate, channels)) = audio {
            let output = MFCreateMediaType()?;
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
            output.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
            output.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(channels))?;
            output.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            output.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 16000)?;
            let audio_stream = sink.AddStream(&output)?;
            let input = MFCreateMediaType()?;
            input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
            input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(channels))?;
            input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, u32::from(channels) * 2)?;
            input.SetUINT32(
                &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
                rate * u32::from(channels) * 2,
            )?;
            sink.SetInputMediaType(audio_stream, &input, None)?;
            Some(audio_stream)
        } else {
            None
        };
        sink.BeginWriting()?;
        Ok((sink, stream, audio_stream))
    }
}
#[allow(clippy::too_many_arguments)]
unsafe fn write(
    sink: &IMFSinkWriter,
    stream: u32,
    bytes: &[u8],
    n: u64,
    source_size: (u32, u32),
    output_size: (u32, u32),
    crop: (u32, u32, u32, u32),
    click: Option<(i32, i32, u32)>,
    content: (u32, u32, u32, u32),
) -> windows::core::Result<()> {
    let (source_width, source_height) = source_size;
    let (width, height) = output_size;
    let (left, top, crop_width, crop_height) = crop;
    let _ = source_height;
    unsafe {
        let buffer = MFCreateMemoryBuffer(width * height * 4)?;
        let mut dst = std::ptr::null_mut();
        buffer.Lock(&mut dst, None, None)?;
        // RGB32 supplied to Media Foundation has bottom-up row order.
        let stride = width as usize * 4;
        std::ptr::write_bytes(dst, 0, width as usize * height as usize * 4);
        let (offset_x, offset_y, content_width, content_height) = content;
        for y in 0..content_height as usize {
            let sy = top as usize + y * crop_height as usize / content_height as usize;
            for x in 0..content_width as usize {
                let sx = left as usize + x * crop_width as usize / content_width as usize;
                let src_offset = (sy * source_width as usize + sx) * 4;
                let dst_offset = (height as usize - 1 - y - offset_y as usize) * stride
                    + (x + offset_x as usize) * 4;
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr().add(src_offset),
                    dst.add(dst_offset),
                    4,
                );
            }
        }
        if let Some((cx, cy, radius)) = click {
            let r = radius as i32;
            for dy in -r - 2..=r + 2 {
                for dx in -r - 2..=r + 2 {
                    let d = dx * dx + dy * dy;
                    if d >= (r - 2) * (r - 2) && d <= (r + 2) * (r + 2) {
                        let x = cx + dx;
                        let y = cy + dy;
                        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                            let offset =
                                (height as usize - 1 - y as usize) * stride + x as usize * 4;
                            *dst.add(offset) = 0;
                            *dst.add(offset + 1) = 0;
                            *dst.add(offset + 2) = 255;
                        }
                    }
                }
            }
        }
        buffer.Unlock()?;
        buffer.SetCurrentLength(width * height * 4)?;
        let sample = MFCreateSample()?;
        sample.AddBuffer(&buffer)?;
        let start = (n * 10_000_000 / 30) as i64;
        let end = ((n + 1) * 10_000_000 / 30) as i64;
        sample.SetSampleTime(start)?;
        sample.SetSampleDuration(end - start)?;
        sink.WriteSample(stream, &sample)
    }
}
unsafe fn write_audio(
    sink: &IMFSinkWriter,
    stream: u32,
    data: &[i16],
    sample_start: u64,
    rate: u32,
    channels: u16,
) -> windows::core::Result<()> {
    unsafe {
        let bytes = (data.len() * 2) as u32;
        let buffer = MFCreateMemoryBuffer(bytes)?;
        let mut dst = std::ptr::null_mut();
        buffer.Lock(&mut dst, None, None)?;
        std::ptr::copy_nonoverlapping(data.as_ptr().cast::<u8>(), dst, bytes as usize);
        buffer.Unlock()?;
        buffer.SetCurrentLength(bytes)?;
        let sample = MFCreateSample()?;
        sample.AddBuffer(&buffer)?;
        let frames = (data.len() / channels as usize) as u64;
        let start = (sample_start * 10_000_000 / u64::from(rate)) as i64;
        let end = ((sample_start + frames) * 10_000_000 / u64::from(rate)) as i64;
        sample.SetSampleTime(start)?;
        sample.SetSampleDuration(end - start)?;
        sink.WriteSample(stream, &sample)
    }
}
// Returns silence to insert, leading packet frames to discard, and frames to write.
fn audio_span(
    cursor: u64,
    timestamp: u64,
    count: usize,
    rate: u32,
    limit: Option<u64>,
) -> Result<(usize, usize, usize), &'static str> {
    let end = limit.unwrap_or(u64::MAX);
    if cursor >= end {
        return Ok((0, count, 0));
    }
    let gap = timestamp.saturating_sub(cursor);
    if gap > u64::from(rate) / 4 {
        return Err("Microphone timestamp discontinuity exceeds 250 ms");
    }
    let silence = gap.min(end - cursor) as usize;
    let skip = cursor.saturating_sub(timestamp).min(count as u64) as usize;
    let available = count - skip;
    let frames = available.min(end.saturating_sub(cursor + silence as u64) as usize);
    Ok((silence, skip, frames))
}
fn drain_audio(
    mic: &crate::audio::Microphone,
    sink: &IMFSinkWriter,
    stream: u32,
    audio_frames: &mut u64,
    limit: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    while let Ok(packet) = mic.receiver.try_recv() {
        let channels = usize::from(mic.channels);
        let count = packet.samples.len() / channels;
        mic.queued
            .fetch_sub(count, std::sync::atomic::Ordering::Relaxed);
        let timestamp = (packet.start.as_nanos() * u128::from(mic.rate) / 1_000_000_000) as u64;
        let (silence, skip, frames) = audio_span(*audio_frames, timestamp, count, mic.rate, limit)?;
        if silence > 0 {
            let zeros = vec![0i16; silence * channels];
            unsafe {
                write_audio(sink, stream, &zeros, *audio_frames, mic.rate, mic.channels)?;
            }
            *audio_frames += silence as u64;
        }
        if frames > 0 {
            unsafe {
                write_audio(
                    sink,
                    stream,
                    &packet.samples[skip * channels..(skip + frames) * channels],
                    *audio_frames,
                    mic.rate,
                    mic.channels,
                )?;
            }
            *audio_frames += frames as u64;
        }
    }
    Ok(())
}
pub fn parse_crop(value: &str) -> Result<(u32, u32, u32, u32), Box<dyn Error>> {
    let parts: Vec<u32> = value.split(',').map(str::parse).collect::<Result<_, _>>()?;
    if parts.len() != 4 {
        return Err("Invalid area selection".into());
    }
    Ok((parts[0], parts[1], parts[2], parts[3]))
}
fn output_size(source_width: u32, source_height: u32, small: bool) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 {
        return None;
    }
    let (max_width, max_height) = if small {
        (1280.0, 720.0)
    } else {
        (1920.0, 1080.0)
    };
    let scale = (max_width / f64::from(source_width))
        .min(max_height / f64::from(source_height))
        .min(1.0);
    let width = ((f64::from(source_width) * scale) as u32) & !1;
    let height = ((f64::from(source_height) * scale) as u32) & !1;
    (width >= 2 && height >= 2).then_some((width, height))
}
fn fit_content(crop_width: u32, crop_height: u32, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let scale = (width as f64 / crop_width as f64).min(height as f64 / crop_height as f64);
    let content_width = ((crop_width as f64 * scale) as u32).max(1).min(width);
    let content_height = ((crop_height as f64 * scale) as u32).max(1).min(height);
    (
        (width - content_width) / 2,
        (height - content_height) / 2,
        content_width,
        content_height,
    )
}
#[allow(clippy::too_many_arguments)]
pub fn record_internal(
    index: usize,
    seconds: u64,
    path: &Path,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    small: bool,
    mic_index: Option<usize>,
    mic_name: Option<&str>,
    window_handle: Option<usize>,
    crop: Option<(u32, u32, u32, u32)>,
    click_style: u8,
    vertical: bool,
) -> Result<(), Box<dyn Error>> {
    if !(1..=86_400).contains(&seconds) || path.exists() {
        return Err("Duration must be positive and output must not already exist".into());
    }
    let tmp = path.with_extension("partial.mp4");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|error| format!("Cannot reserve temporary output {}: {error}", tmp.display()))?;
    let shared: Shared = Arc::new(Mutex::new(None));
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        if let Err(error) = MFStartup(MF_VERSION, 0) {
            CoUninitialize();
            return Err(error.into());
        }
    }
    let result = (|| -> Result<(), Box<dyn Error>> {
        let mic = mic_index
            .map(|index| crate::audio::open(index, mic_name))
            .transpose()?;
        let audio_format = mic.as_ref().map(|m| (m.rate, m.channels));
        let origin = if let Some(handle) = window_handle {
            let window =
                windows_capture::window::Window::from_raw_hwnd(handle as *mut std::ffi::c_void);
            let rect = window.rect()?;
            (rect.left, rect.top)
        } else {
            let monitor = Monitor::enumerate()?
                .into_iter()
                .nth(index)
                .ok_or("Display index unavailable")?;
            crate::area::origin(&monitor).ok_or("Display position unavailable")?
        };
        let _click_listener = if click_style != 0 {
            Some(crate::click::Listener::start(window_handle)?)
        } else {
            None
        };
        let control = if let Some(handle) = window_handle {
            let window =
                windows_capture::window::Window::from_raw_hwnd(handle as *mut std::ffi::c_void);
            if !window.is_valid() {
                return Err("Selected window is no longer available".into());
            }
            Capture::start_free_threaded(Settings::new(
                window,
                CursorCaptureSettings::WithCursor,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                shared.clone(),
            ))?
        } else {
            let monitor = Monitor::enumerate()?
                .into_iter()
                .nth(index)
                .ok_or("Display index unavailable")?;
            Capture::start_free_threaded(Settings::new(
                monitor,
                CursorCaptureSettings::WithCursor,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                shared.clone(),
            ))?
        };
        let wait = Instant::now();
        while shared.lock().map_err(|_| "frame lock poisoned")?.is_none() {
            if control.is_finished() {
                return Err("Capture ended before the first frame".into());
            }
            if wait.elapsed() > Duration::from_secs(10) {
                return Err("No screen frame received".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        let (source_width, source_height) = {
            let lock = shared.lock().map_err(|_| "frame lock poisoned")?;
            let (_, w, h) = lock.as_ref().ok_or("Missing frame")?;
            (*w, *h)
        };
        let crop = crop.unwrap_or((0, 0, source_width, source_height));
        if crop.2 < 2
            || crop.3 < 2
            || crop.0.checked_add(crop.2).is_none_or(|x| x > source_width)
            || crop.1.checked_add(crop.3).is_none_or(|y| y > source_height)
        {
            return Err("Selected area is outside the display".into());
        }
        let (width, height) = if vertical {
            if small { (720, 1280) } else { (1080, 1920) }
        } else {
            output_size(crop.2, crop.3, small).ok_or("Capture dimensions too small")?
        };
        let content = fit_content(crop.2, crop.3, width, height);
        let (sink, stream, audio_stream) =
            unsafe { writer(&tmp, width, height, small, audio_format)? };
        let capture_result = (|| -> Result<(), Box<dyn Error>> {
            let start = Instant::now();
            if let Some(mic) = &mic {
                crate::audio::start(mic, start)?;
            }
            use std::io::Write;
            std::io::stdout().write_all(&[1])?;
            std::io::stdout().flush()?;
            let mut n = 0u64;
            let mut audio_frames = 0u64;
            let mut disk_check = Instant::now();
            while n < seconds * 30 && !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let target = Duration::from_nanos(n * 1_000_000_000 / 30);
                if let Some(delay) = target.checked_sub(start.elapsed()) {
                    thread::sleep(delay);
                }
                if disk_check.elapsed() >= Duration::from_secs(5) {
                    if !has_disk_reserve(path) {
                        return Err(
                            "Disk space is low or unavailable; partial output retained".into()
                        );
                    }
                    disk_check = Instant::now();
                }
                if window_handle.is_some_and(|handle| unsafe { IsIconic(handle as isize) != 0 }) {
                    return Err("The recorded window was minimized; partial output retained".into());
                }
                if control.is_finished() {
                    return Err("Capture ended unexpectedly".into());
                }
                if start.elapsed() > target + Duration::from_secs(1) {
                    return Err("Encoder cannot keep up with real-time recording".into());
                }
                let (bytes, frame_width, frame_height) = shared
                    .lock()
                    .map_err(|_| "frame lock poisoned")?
                    .as_ref()
                    .ok_or("Missing frame")?
                    .clone();
                if (frame_width, frame_height) != (source_width, source_height) {
                    return Err("Capture dimensions changed during recording".into());
                }
                if bytes.len() != (source_width as usize) * (source_height as usize) * 4 {
                    return Err("Display dimensions changed during recording".into());
                }
                let click = crate::click::current()
                    .filter(|_| click_style != 0)
                    .and_then(|(x, y, elapsed)| {
                        let (base_x, base_y) = if window_handle.is_some() {
                            (0, 0)
                        } else {
                            origin
                        };
                        let sx = x - base_x - crop.0 as i32;
                        let sy = y - base_y - crop.1 as i32;
                        if sx < 0 || sy < 0 || sx >= crop.2 as i32 || sy >= crop.3 as i32 {
                            return None;
                        }
                        let radius = if click_style == 2 {
                            8 + (elapsed.as_millis() as u32 * 18 / 450)
                        } else {
                            16
                        };
                        Some((
                            content.0 as i32 + sx * content.2 as i32 / crop.2 as i32,
                            content.1 as i32 + sy * content.3 as i32 / crop.3 as i32,
                            radius,
                        ))
                    });
                unsafe {
                    write(
                        &sink,
                        stream,
                        &bytes,
                        n,
                        (source_width, source_height),
                        (width, height),
                        crop,
                        click,
                        content,
                    )?;
                }
                n += 1;
                if let (Some(mic), Some(audio_stream)) = (&mic, audio_stream) {
                    if mic.failed.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err("Microphone disconnected or could not keep up".into());
                    }
                    drain_audio(mic, &sink, audio_stream, &mut audio_frames, None)?;
                }
            }
            if let (Some(mic), Some(audio_stream)) = (&mic, audio_stream) {
                use cpal::traits::StreamTrait;
                mic.stream
                    .pause()
                    .map_err(|e| format!("Cannot stop microphone: {e}"))?;
                let end = n * u64::from(mic.rate) / 30;
                drain_audio(mic, &sink, audio_stream, &mut audio_frames, Some(end))?;
                if mic.failed.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err("Microphone disconnected or could not keep up".into());
                }
            }
            Ok(())
        })();
        let stop_result = control.stop();
        let finalize_result = unsafe { sink.Finalize() };
        capture_result?;
        stop_result?;
        finalize_result?;
        drop(sink);
        publish_no_replace(&tmp, path)?;
        Ok(())
    })();
    unsafe {
        let _ = MFShutdown();
        CoUninitialize();
    }
    result
}

pub fn record(index: usize, seconds: u64, path: &Path) -> Result<(), Box<dyn Error>> {
    record_control(
        index,
        seconds,
        path,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        false,
        None,
        None,
        None,
        None,
        0,
        false,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
}

#[allow(clippy::collapsible_if, clippy::too_many_arguments)]
pub fn record_control(
    index: usize,
    seconds: u64,
    path: &Path,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    small: bool,
    mic_index: Option<usize>,
    expected_mic_name: Option<String>,
    window_handle: Option<usize>,
    crop: Option<(u32, u32, u32, u32)>,
    click_style: u8,
    vertical: bool,
    ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    use std::{
        io::{Read, Write},
        os::windows::{io::AsRawHandle, process::CommandExt},
        process::Stdio,
    };
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::*,
    };
    let executable = std::env::current_exe()?;
    let mic_name = if let Some(index) = mic_index {
        let names = crate::audio::devices()?;
        let name = names.get(index).ok_or("Microphone unavailable")?.clone();
        if expected_mic_name
            .as_ref()
            .is_some_and(|expected| expected != &name)
            || names.iter().filter(|other| **other == name).count() != 1
        {
            return Err("Microphone name is ambiguous; choose another device".into());
        }
        name
    } else {
        String::new()
    };
    let job = unsafe { CreateJobObjectW(None, windows::core::PCWSTR::null())? };
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    };
    if let Err(error) = configured {
        unsafe {
            let _ = CloseHandle(job);
        }
        return Err(error.into());
    }
    let child = std::process::Command::new(executable)
        .arg("internal-record")
        .arg(index.to_string())
        .arg(seconds.to_string())
        .arg(path)
        .arg(if small { "small" } else { "standard" })
        .arg(
            mic_index
                .map(|n| n.to_string())
                .unwrap_or_else(|| "off".into()),
        )
        .arg(mic_name)
        .arg(window_handle.map(|h| h.to_string()).unwrap_or_default())
        .arg(
            crop.map(|(x, y, w, h)| format!("{x},{y},{w},{h}"))
                .unwrap_or_default(),
        )
        .arg(click_style.to_string())
        .arg(if vertical { "vertical" } else { "landscape" })
        .creation_flags(0x08000000)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut worker_error = String::new();
    let result = match child {
        Ok(mut child) => {
            let mut error_output = child.stderr.take();
            let handle = HANDLE(child.as_raw_handle());
            let status = match unsafe { AssignProcessToJobObject(job, handle) } {
                Ok(()) => {
                    if let Some(mut output) = child.stdout.take() {
                        std::thread::spawn(move || {
                            let mut byte = [0u8; 1];
                            if output.read_exact(&mut byte).is_ok() && byte == [1] {
                                ready.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        });
                    }
                    let mut input = child.stdin.take();
                    let start = input
                        .as_mut()
                        .ok_or_else(|| std::io::Error::other("Recording worker input unavailable"))
                        .and_then(|pipe| pipe.write_all(&[1]));
                    if let Err(error) = start {
                        let _ = child.kill();
                        let _ = child.wait();
                        Err(Box::new(error) as Box<dyn Error>)
                    } else {
                        loop {
                            match child.try_wait() {
                                Ok(Some(status)) => break Ok(status),
                                Err(error) => {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    break Err(error.into());
                                }
                                Ok(None) => {}
                            }
                            if stop.swap(false, std::sync::atomic::Ordering::Relaxed) {
                                if let Some(mut pipe) = input.take() {
                                    let _ = pipe.write_all(&[2]);
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(30));
                        }
                    }
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(Box::new(e) as Box<dyn Error>)
                }
            };
            if let Some(ref mut stderr) = error_output {
                let _ = stderr.read_to_string(&mut worker_error);
            }
            status
        }
        Err(e) => Err(Box::new(e) as Box<dyn Error>),
    };
    unsafe {
        let _ = CloseHandle(job);
    }
    let status = result?;
    if !status.success() {
        return Err(format!(
            "Recording worker failed with {status}. {} Partial output: {}",
            worker_error.trim(),
            path.with_extension("partial.mp4").display()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{audio_span, fit_content, output_size, publish_no_replace};
    #[test]
    fn portrait_fits_entire_landscape_source_with_bars() {
        assert_eq!(fit_content(1920, 1080, 1080, 1920), (0, 656, 1080, 607));
        assert_eq!(fit_content(1080, 1920, 1080, 1920), (0, 0, 1080, 1920));
    }
    #[test]
    fn publish_does_not_replace_existing_destination() {
        let dir =
            std::env::temp_dir().join(format!("framelatch-publish-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source");
        let dest = dir.join("dest");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&dest, b"existing").unwrap();
        assert!(publish_no_replace(&source, &dest).is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"existing");
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(dest).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn audio_timeline_clips_overlaps_gaps_and_stop() {
        assert_eq!(audio_span(100, 90, 30, 48_000, None).unwrap(), (0, 10, 20));
        assert_eq!(audio_span(100, 110, 30, 48_000, None).unwrap(), (10, 0, 30));
        assert_eq!(
            audio_span(100, 110, 30, 48_000, Some(115)).unwrap(),
            (10, 0, 5)
        );
        assert!(audio_span(100, 13_000, 30, 48_000, None).is_err());
    }
    #[test]
    fn dimensions_fit_preset_and_preserve_aspect() {
        assert_eq!(output_size(3840, 2160, false), Some((1920, 1080)));
        assert_eq!(output_size(3840, 2160, true), Some((1280, 720)));
        assert_eq!(output_size(3000, 1876, true), Some((1150, 720)));
        assert_eq!(output_size(1, 0, false), None);
    }
}
