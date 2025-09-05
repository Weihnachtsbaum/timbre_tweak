use std::{f32::consts::TAU, thread, time::Duration};

use cpal::{
    Device, FromSample, I24, SizedSample, StreamConfig, StreamError,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

fn main() {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("No output device available");
    let default_config = device
        .default_output_config()
        .expect("Could not get default output config");
    println!("{default_config:?}");
    let config = default_config.config();
    match default_config.sample_format() {
        cpal::SampleFormat::I8 => run::<i8>(&device, &config),
        cpal::SampleFormat::I16 => run::<i16>(&device, &config),
        cpal::SampleFormat::I24 => run::<I24>(&device, &config),
        cpal::SampleFormat::I32 => run::<i32>(&device, &config),
        cpal::SampleFormat::I64 => run::<i64>(&device, &config),
        cpal::SampleFormat::U8 => run::<u8>(&device, &config),
        cpal::SampleFormat::U16 => run::<u16>(&device, &config),
        cpal::SampleFormat::U32 => run::<u32>(&device, &config),
        cpal::SampleFormat::U64 => run::<u64>(&device, &config),
        cpal::SampleFormat::F32 => run::<f32>(&device, &config),
        cpal::SampleFormat::F64 => run::<f64>(&device, &config),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }
}

struct Playback {
    sample_rate: u32,
    channels: u16,
    hz: f32,
    sample: u32,
}

fn run<T: SizedSample + FromSample<f32> + 'static>(device: &Device, config: &StreamConfig) {
    let mut playback = Playback {
        sample: 0,
        sample_rate: config.sample_rate.0,
        channels: config.channels,
        hz: 440.0,
    };
    let stream = device
        .build_output_stream(
            config,
            move |data, _| write_data::<T>(data, &mut playback),
            err,
            None,
        )
        .expect("Could not build output stream");
    stream.play().expect("Could not play stream");
    thread::sleep(Duration::from_secs(5));
}

fn write_data<T: SizedSample + FromSample<f32>>(data: &mut [T], playback: &mut Playback) {
    for frame in data.chunks_mut(playback.channels as usize) {
        let sec = playback.sample as f32 / playback.sample_rate as f32;
        let value = T::from_sample((sec * playback.hz * TAU).sin());
        playback.sample += 1;
        for sample in frame {
            *sample = value;
        }
    }
}

fn err(err: StreamError) {
    eprintln!("Error: {err}");
}
