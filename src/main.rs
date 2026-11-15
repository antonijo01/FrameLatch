mod record;
use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
type CaptureError = Box<dyn Error + Send + Sync>;
struct Probe(Arc<AtomicU64>);
impl GraphicsCaptureApiHandler for Probe {
    type Flags = Arc<AtomicU64>;
    type Error = CaptureError;
    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self(ctx.flags))
    }
    fn on_frame_arrived(
        &mut self,
        _: &mut Frame,
        _: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" {
        println!(
            "FrameLatch capture qualification\n\n  framelatch list\n  framelatch idle SECONDS\n  framelatch record MONITOR_INDEX SECONDS OUTPUT.mp4\n  framelatch record-cycles MONITOR_INDEX SECONDS OUTPUT_FOLDER CYCLES\n  framelatch probe MONITOR_INDEX SECONDS [CYCLES]\n\nRecord captures a silent H.264 MP4, scaled to fit 1920x1080."
        );
        return Ok(());
    }
    let monitors = || Monitor::enumerate().map_err(|e| format!("Cannot enumerate displays: {e}"));
    match args[0].as_str() {
        "internal-record" if args.len() == 4 => {
            use std::io::Read;
            let mut start = [0u8; 1];
            std::io::stdin().read_exact(&mut start)?;
            if start != [1] {
                return Err("Recording worker was not started by its controller".into());
            }
            record::record_internal(
                args[1].parse()?,
                args[2].parse()?,
                std::path::Path::new(&args[3]),
            )?;
        }
        "record" if args.len() == 4 => record::record(
            args[1].parse()?,
            args[2].parse()?,
            std::path::Path::new(&args[3]),
        )?,
        "record-cycles" if args.len() == 5 => {
            let index = args[1].parse()?;
            let duration = args[2].parse()?;
            let count: u32 = args[4].parse()?;
            if count == 0 || count > 100 {
                return Err("Cycles must be between 1 and 100".into());
            }
            let folder = std::path::Path::new(&args[3]);
            for cycle in 1..=count {
                record::record(index, duration, &folder.join(format!("cycle-{cycle}.mp4")))?;
                println!("Completed cycle {cycle}. Waiting 5 seconds for memory sampling.");
                thread::sleep(Duration::from_secs(5));
            }
        }
        "list" if args.len() == 1 => {
            for (index, monitor) in monitors()?.iter().enumerate() {
                let name = monitor.name().unwrap_or_default();
                let name = if name.is_empty() {
                    format!("Monitor {}", index + 1)
                } else {
                    name
                };
                println!(
                    "{index}: {} ({} x {})",
                    name,
                    monitor.width()?,
                    monitor.height()?
                );
            }
        }
        "idle" if args.len() == 2 => thread::sleep(Duration::from_secs(args[1].parse()?)),
        "probe" if (3..=4).contains(&args.len()) => {
            let index: usize = args[1].parse()?;
            let seconds: u64 = args[2].parse()?;
            let cycles: usize = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(1);
            if seconds == 0 || cycles == 0 {
                return Err("Duration and cycles must be positive".into());
            }
            for cycle in 0..cycles {
                let monitor = monitors()?
                    .into_iter()
                    .nth(index)
                    .ok_or("Display index is unavailable")?;
                let frames = Arc::new(AtomicU64::new(0));
                let settings = Settings::new(
                    monitor,
                    CursorCaptureSettings::Default,
                    DrawBorderSettings::Default,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Default,
                    DirtyRegionSettings::Default,
                    ColorFormat::Bgra8,
                    frames.clone(),
                );
                let control = Probe::start_free_threaded(settings)?;
                thread::sleep(Duration::from_secs(seconds));
                control.stop()?;
                println!(
                    "cycle {}: {} frames in {} seconds",
                    cycle + 1,
                    frames.load(Ordering::Relaxed),
                    seconds
                );
                thread::sleep(Duration::from_secs(2));
            }
        }
        _ => return Err("Invalid arguments. Run framelatch --help.".into()),
    }
    Ok(())
}
