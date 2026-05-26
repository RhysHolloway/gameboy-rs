use cpal::SizedSample;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gameboy_core::GameboyColor;
use gameboy_core::bus::AudioCallback;
use tracing::{error, info};

use std::sync::mpsc::{self, Receiver, Sender};

pub struct Audio {
    sample_rate: usize,
    sender: Sender<[f32; 2]>,
    _stream: cpal::Stream,
}

impl Audio {
    pub fn callback(&self) -> AudioCallback {
        let mut sample_counter = 0_u64;
        const CLOCK_SPEED: u64 = GameboyColor::CLOCK_SPEED as u64;

        let sender_clone = self.sender.clone();
        let sample_rate = self.sample_rate as u64;

        Some(Box::new(move |(sample, cycles)| {
            sample_counter += cycles as u64 * sample_rate;
            let frames = sample_counter / CLOCK_SPEED;
            sample_counter %= CLOCK_SPEED;

            for _ in 0..frames {
                if let Err(_) = sender_clone.send(sample.map(|s| s as f32 / 60.0)) {
                    break;
                }
            }
        }))
    }

    pub fn new() -> Option<Self> {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(device) => device,
            None => {
                error!("No output audio device found");
                return None;
            }
        };

        let supported_config = match device.default_output_config() {
            Ok(config) => config,
            Err(err) => {
                error!("No default output audio config found: {err}");
                return None;
            }
        };

        let sample_format = supported_config.sample_format();
        let config = supported_config.config();
        let channels = config.channels.max(1) as usize;
        let sample_rate = config.sample_rate as usize;
        let (sender, receiver) = mpsc::channel();

        info!(
            "Using audio device: {}, config: {:?}, sample format: {:?}",
            device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "Unknown".to_string()),
            config,
            sample_format
        );

        fn write_output<T: SizedSample>(
            data: &mut [T],
            channels: usize,
            receiver: &Receiver<[f32; 2]>,
            convert: impl Fn(f32) -> T,
        ) {
            for frame in data.chunks_mut(channels) {
                let sample = receiver.try_recv().unwrap_or([0.0, 0.0]);
                for (channel, output) in frame.iter_mut().enumerate() {
                    *output = convert(sample[channel.min(1)]);
                }
            }
        }

        let err_callback = |err| error!("Audio stream error: {err}");

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data, _| write_output(data, channels, &receiver, |sample| sample),
                err_callback,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data, _| {
                    write_output(data, channels, &receiver, |sample| {
                        (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                    })
                },
                err_callback,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _| {
                    write_output(data, channels, &receiver, |sample| {
                        ((sample.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16
                    })
                },
                err_callback,
                None,
            ),
            _ => {
                error!("Unsupported audio sample format: {:?}", sample_format);
                return None;
            }
        };

        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                error!("Failed to build output audio stream: {err}");
                return None;
            }
        };

        if let Err(err) = stream.play() {
            error!("Failed to play output audio stream: {err}");
            return None;
        }

        Some(Self {
            sample_rate,
            sender,
            _stream: stream,
        })
    }
}
