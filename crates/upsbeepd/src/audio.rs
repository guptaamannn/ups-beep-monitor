use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::sync::mpsc::SyncSender;
use std::time::Duration;

pub const BLOCK: Duration = Duration::from_millis(50);

pub enum Msg {
    /// One block of mono samples, `BLOCK` long.
    Block(Vec<f32>),
    StreamError(String),
}

pub struct Capture {
    pub sample_rate: u32,
    _stream: Stream,
}

fn describe(device: &Device) -> String {
    device
        .description()
        .map(|d| d.to_string())
        .unwrap_or_else(|_| "unknown".into())
}

pub fn list_devices() -> Result<(), String> {
    let host = cpal::default_host();
    let default = host.default_input_device().map(|d| describe(&d));
    for device in host.input_devices().map_err(|e| e.to_string())? {
        let name = describe(&device);
        let mark = if Some(&name) == default.as_ref() {
            " (default)"
        } else {
            ""
        };
        println!("{name}{mark}");
    }
    Ok(())
}

fn find_device(wanted: Option<&str>) -> Result<Device, String> {
    let host = cpal::default_host();
    match wanted {
        None => host
            .default_input_device()
            .ok_or_else(|| "no default input device".to_string()),
        Some(w) => {
            let w = w.to_lowercase();
            host.input_devices()
                .map_err(|e| e.to_string())?
                .find(|d| describe(d).to_lowercase().contains(&w))
                .ok_or_else(|| format!("no input device matching {w:?}"))
        }
    }
}

pub fn open(wanted: Option<&str>, tx: SyncSender<Msg>) -> Result<Capture, String> {
    let device = find_device(wanted)?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    eprintln!(
        "capturing from {} ({} Hz, {} ch, {format:?})",
        describe(&device),
        config.sample_rate,
        config.channels
    );
    let sample_rate = config.sample_rate;

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, tx),
        SampleFormat::I16 => build::<i16>(&device, config, tx),
        SampleFormat::U16 => build::<u16>(&device, config, tx),
        SampleFormat::I32 => build::<i32>(&device, config, tx),
        other => Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Capture {
        sample_rate,
        _stream: stream,
    })
}

fn build<T>(device: &Device, config: StreamConfig, tx: SyncSender<Msg>) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let block_len = (config.sample_rate as f32 * BLOCK.as_secs_f32()) as usize;
    let mut buf: Vec<f32> = Vec::with_capacity(block_len);
    let err_tx = tx.clone();

    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                // Downmix interleaved frames to mono.
                for frame in data.chunks(channels) {
                    let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
                    buf.push(sum / channels as f32);
                    if buf.len() >= block_len {
                        let block = std::mem::replace(&mut buf, Vec::with_capacity(block_len));
                        // Drop the block if the worker is behind rather than block the audio thread.
                        let _ = tx.try_send(Msg::Block(block));
                    }
                }
            },
            move |e| {
                let _ = err_tx.try_send(Msg::StreamError(e.to_string()));
            },
            None,
        )
        .map_err(|e| e.to_string())
}
