use cpal::{
    InputCallbackInfo, SampleFormat, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};
pub struct Packet {
    pub samples: Vec<i16>,
    pub start: Duration,
}
pub struct Microphone {
    pub stream: cpal::Stream,
    pub receiver: Receiver<Packet>,
    pub failed: Arc<AtomicBool>,
    pub queued: Arc<AtomicUsize>,
    pub epoch: Arc<std::sync::Mutex<Option<Instant>>>,
    pub rate: u32,
    pub channels: u16,
}
pub fn devices() -> Result<Vec<String>, String> {
    Ok(cpal::default_host()
        .input_devices()
        .map_err(|e| e.to_string())?
        .map(|d| d.name().unwrap_or_else(|_| "Microphone".into()))
        .collect())
}
struct Delivery {
    channels: usize,
    rate: u32,
    tx: SyncSender<Packet>,
    failed: Arc<AtomicBool>,
    queued: Arc<AtomicUsize>,
    epoch: Arc<std::sync::Mutex<Option<Instant>>>,
}
fn deliver<T: Copy>(
    samples: &[T],
    info: &InputCallbackInfo,
    d: &Delivery,
    convert: impl Fn(T) -> i16,
) {
    let epoch = match d.epoch.lock().ok().and_then(|v| *v) {
        Some(v) => v,
        None => return,
    };
    let ts = info.timestamp();
    let lag = ts.callback.duration_since(&ts.capture).unwrap_or_default();
    let start = Instant::now()
        .checked_sub(lag)
        .unwrap_or_else(Instant::now)
        .saturating_duration_since(epoch);
    let count = samples.len() / d.channels;
    let previous = d.queued.fetch_add(count, Ordering::Relaxed);
    if previous + count > d.rate as usize / 4 {
        d.queued.fetch_sub(count, Ordering::Relaxed);
        d.failed.store(true, Ordering::Relaxed);
        return;
    }
    let out_channels = d.channels.min(2);
    let mut data = Vec::with_capacity(count * out_channels);
    for frame in samples.chunks_exact(d.channels) {
        for sample in frame.iter().take(out_channels) {
            data.push(convert(*sample));
        }
    }
    if d.tx
        .try_send(Packet {
            samples: data,
            start,
        })
        .is_err()
    {
        d.queued.fetch_sub(count, Ordering::Relaxed);
        d.failed.store(true, Ordering::Relaxed)
    }
}
pub fn open(index: usize, expected_name: Option<&str>) -> Result<Microphone, String> {
    let device = cpal::default_host()
        .input_devices()
        .map_err(|e| e.to_string())?
        .nth(index)
        .ok_or("Microphone no longer available")?;
    if let Some(expected) = expected_name {
        let names = devices()?;
        if names.get(index).map(String::as_str) != Some(expected)
            || names
                .iter()
                .filter(|name| name.as_str() == expected)
                .count()
                != 1
        {
            return Err("Microphone list changed; select it again".into());
        }
    }
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let channels = config.channels as usize;
    if channels == 0 {
        return Err("Invalid microphone channels".into());
    }
    let rate = config.sample_rate.0;
    if ![44100, 48000].contains(&rate) {
        return Err(format!(
            "Microphone sample rate {rate} is not supported yet"
        ));
    }
    let (tx, receiver) = mpsc::sync_channel(32);
    let failed = Arc::new(AtomicBool::new(false));
    let queued = Arc::new(AtomicUsize::new(0));
    let epoch = Arc::new(std::sync::Mutex::new(None));
    let err = {
        let failed = failed.clone();
        move |_e| failed.store(true, Ordering::Relaxed)
    };
    let delivery = Delivery {
        channels,
        rate,
        tx,
        failed: failed.clone(),
        queued: queued.clone(),
        epoch: epoch.clone(),
    };
    let stream = match format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], info| {
                deliver(data, info, &delivery, |x| {
                    (x.clamp(-1.0, 1.0) * 32767.0) as i16
                })
            },
            err,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], info| deliver(data, info, &delivery, |x| x),
            err,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], info| {
                deliver(data, info, &delivery, |x| (i32::from(x) - 32768) as i16)
            },
            err,
            None,
        ),
        _ => return Err(format!("Unsupported microphone sample format: {format:?}")),
    }
    .map_err(|e| e.to_string())?;
    Ok(Microphone {
        stream,
        receiver,
        failed,
        queued,
        epoch,
        rate,
        channels: channels.min(2) as u16,
    })
}
pub fn start(mic: &Microphone, epoch: Instant) -> Result<(), String> {
    *mic.epoch.lock().map_err(|_| "Audio clock unavailable")? = Some(epoch);
    mic.stream.play().map_err(|e| e.to_string())
}
