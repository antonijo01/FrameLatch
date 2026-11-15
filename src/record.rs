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

type CaptureError = Box<dyn Error + Send + Sync>;
type Shared = Arc<Mutex<Option<Vec<u8>>>>;
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
        let buffer = frame.buffer()?;
        let data = buffer.as_nopadding_buffer(&mut scratch);
        let mut latest = self.0.lock().map_err(|_| "frame lock poisoned")?;
        match latest.as_mut() {
            Some(previous) => {
                previous.clear();
                previous.extend_from_slice(data);
            }
            None => *latest = Some(data.to_vec()),
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
) -> windows::core::Result<(IMFSinkWriter, u32)> {
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
        output.SetUINT32(&MF_MT_AVG_BITRATE, 8_000_000)?;
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
        sink.BeginWriting()?;
        Ok((sink, stream))
    }
}
unsafe fn write(
    sink: &IMFSinkWriter,
    stream: u32,
    bytes: &[u8],
    n: u64,
    source_size: (u32, u32),
    output_size: (u32, u32),
) -> windows::core::Result<()> {
    let (source_width, source_height) = source_size;
    let (width, height) = output_size;
    unsafe {
        let buffer = MFCreateMemoryBuffer(width * height * 4)?;
        let mut dst = std::ptr::null_mut();
        buffer.Lock(&mut dst, None, None)?;
        // RGB32 supplied to Media Foundation has bottom-up row order.
        let stride = width as usize * 4;
        for y in 0..height as usize {
            let sy = y * source_height as usize / height as usize;
            for x in 0..width as usize {
                let sx = x * source_width as usize / width as usize;
                let src_offset = (sy * source_width as usize + sx) * 4;
                let dst_offset = (height as usize - 1 - y) * stride + x * 4;
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr().add(src_offset),
                    dst.add(dst_offset),
                    4,
                );
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
pub fn record_internal(index: usize, seconds: u64, path: &Path) -> Result<(), Box<dyn Error>> {
    if !(1..=86_400).contains(&seconds) || path.exists() {
        return Err("Duration must be positive and output must not already exist".into());
    }
    let monitor = Monitor::enumerate()?
        .into_iter()
        .nth(index)
        .ok_or("Display index unavailable")?;
    let (source_width, source_height) = (monitor.width()?, monitor.height()?);
    let scale = (1920.0 / f64::from(source_width))
        .min(1080.0 / f64::from(source_height))
        .min(1.0);
    let width = ((f64::from(source_width) * scale) as u32) & !1;
    let height = ((f64::from(source_height) * scale) as u32) & !1;
    if width < 2 || height < 2 {
        return Err("Monitor dimensions too small".into());
    }
    let tmp = path.with_extension("partial.mp4");
    if tmp.exists() {
        return Err("Temporary output already exists".into());
    }
    let shared: Shared = Arc::new(Mutex::new(None));
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::Default,
        DrawBorderSettings::Default,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        shared.clone(),
    );
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        if let Err(error) = MFStartup(MF_VERSION, 0) {
            CoUninitialize();
            return Err(error.into());
        }
    }
    let result = (|| -> Result<(), Box<dyn Error>> {
        let (sink, stream) = unsafe { writer(&tmp, width, height)? };
        let control = match Capture::start_free_threaded(settings) {
            Ok(control) => control,
            Err(error) => {
                let _ = unsafe { sink.Finalize() };
                return Err(error.into());
            }
        };
        let capture_result = (|| -> Result<(), Box<dyn Error>> {
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
            let start = Instant::now();
            let mut n = 0u64;
            while n < seconds * 30 {
                let target = Duration::from_nanos(n * 1_000_000_000 / 30);
                if let Some(delay) = target.checked_sub(start.elapsed()) {
                    thread::sleep(delay);
                }
                if control.is_finished() {
                    return Err("Capture ended unexpectedly".into());
                }
                if start.elapsed() > target + Duration::from_secs(1) {
                    return Err("Encoder cannot keep up with real-time recording".into());
                }
                let bytes = shared
                    .lock()
                    .map_err(|_| "frame lock poisoned")?
                    .as_ref()
                    .ok_or("Missing frame")?
                    .clone();
                if bytes.len() != (source_width as usize) * (source_height as usize) * 4 {
                    return Err("Display dimensions changed during recording".into());
                }
                unsafe {
                    write(
                        &sink,
                        stream,
                        &bytes,
                        n,
                        (source_width, source_height),
                        (width, height),
                    )?;
                }
                n += 1;
            }
            Ok(())
        })();
        let stop_result = control.stop();
        let finalize_result = unsafe { sink.Finalize() };
        capture_result?;
        stop_result?;
        finalize_result?;
        drop(sink);
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    unsafe {
        let _ = MFShutdown();
        CoUninitialize();
    }
    result
}

pub fn record(index: usize, seconds: u64, path: &Path) -> Result<(), Box<dyn Error>> {
    use std::{
        io::Write,
        os::windows::{io::AsRawHandle, process::CommandExt},
        process::Stdio,
    };
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::*,
    };
    let executable = std::env::current_exe()?;
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
        .creation_flags(0x08000000)
        .stdin(Stdio::piped())
        .spawn();
    let result = match child {
        Ok(mut child) => {
            let handle = HANDLE(child.as_raw_handle());
            match unsafe { AssignProcessToJobObject(job, handle) } {
                Ok(()) => {
                    let start = child
                        .stdin
                        .take()
                        .ok_or_else(|| std::io::Error::other("Recording worker input unavailable"))
                        .and_then(|mut input| input.write_all(&[1]));
                    if let Err(error) = start {
                        let _ = child.kill();
                        let _ = child.wait();
                        Err(Box::new(error) as Box<dyn Error>)
                    } else {
                        child.wait().map_err(|e| e.into())
                    }
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(Box::new(e) as Box<dyn Error>)
                }
            }
        }
        Err(e) => Err(Box::new(e) as Box<dyn Error>),
    };
    unsafe {
        let _ = CloseHandle(job);
    }
    let status = result?;
    if !status.success() {
        return Err(format!(
            "Recording worker failed with {status}. Check the partial output file."
        )
        .into());
    }
    Ok(())
}
