use std::{f32::consts::TAU, sync::Arc};

use cpal::{
    Device, FromSample, I24, SizedSample, StreamConfig, StreamError,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use eframe::{
    App, NativeOptions,
    egui::{CentralPanel, Context, Popup, PopupCloseBehavior, ScrollArea, Slider, mutex::Mutex},
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

#[derive(PartialEq)]
enum Waveform {
    Sine,
    Triangle,
    Sawtooth,
    Square,
}

impl Waveform {
    fn at(&self, t: f32) -> f32 {
        match *self {
            Self::Sine => (t * TAU).sin(),
            Self::Triangle => {
                if t.fract() < 0.5 {
                    t.fract() * 4.0 - 1.0
                } else {
                    3.0 - t.fract() * 4.0
                }
            }
            Self::Sawtooth => t.fract() * 2.0 - 1.0,
            Self::Square => {
                if t.fract() < 0.5 {
                    -1.0
                } else {
                    1.0
                }
            }
        }
    }
}

struct Wave {
    waveform: Waveform,
    hz: f32,
    amp: f32,
}

impl Wave {
    fn at(&self, sec: f32) -> f32 {
        self.waveform.at(sec * self.hz) * self.amp
    }
}

struct Playback {
    sample_rate: u32,
    channels: u16,
    sample: u32,
    waves: Vec<Wave>,
}

#[derive(Clone)]
struct MyApp(Arc<Mutex<Playback>>);

impl App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // TODO: don't block audio thread
        let mut playback = self.0.lock();
        CentralPanel::default().show(ctx, |ui| {
            if ui.button("Add wave").clicked() {
                playback.waves.push(Wave {
                    waveform: Waveform::Sine,
                    hz: 440.0,
                    amp: 0.5,
                });
            }
            ScrollArea::vertical().show(ui, |ui| {
                playback.waves.retain_mut(|wave| {
                    ui.add_space(25.0);
                    ui.add(Slider::new(&mut wave.amp, 0.0..=1.0).text("volume"));
                    ui.add(Slider::new(&mut wave.hz, 20.0..=2000.0).text("hz"));

                    let response = ui.button("Waveform");
                    Popup::menu(&response)
                        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| {
                            ui.selectable_value(&mut wave.waveform, Waveform::Sine, "Sine");
                            ui.selectable_value(&mut wave.waveform, Waveform::Triangle, "Triangle");
                            ui.selectable_value(&mut wave.waveform, Waveform::Sawtooth, "Sawtooth");
                            ui.selectable_value(&mut wave.waveform, Waveform::Square, "Square");
                        });

                    !ui.button("Remove wave").clicked()
                });
            });
        });
    }
}

fn run<T: SizedSample + FromSample<f32> + 'static>(device: &Device, config: &StreamConfig) {
    let app = MyApp(Arc::new(Mutex::new(Playback {
        sample: 0,
        sample_rate: config.sample_rate.0,
        channels: config.channels,
        waves: vec![],
    })));
    let clone = app.clone();
    let stream = device
        .build_output_stream(
            config,
            move |data, _| write_data::<T>(data, &clone),
            err,
            None,
        )
        .expect("Could not build output stream");
    stream.play().expect("Could not play stream");

    eframe::run_native(
        "Timbre Tweak",
        NativeOptions::default(),
        Box::new(|_| Ok(Box::new(app))),
    )
    .expect("Error running eframe App");
}

fn write_data<T: SizedSample + FromSample<f32>>(data: &mut [T], app: &MyApp) {
    let mut playback = app.0.lock();
    for frame in data.chunks_mut(playback.channels as usize) {
        let sec = playback.sample as f32 / playback.sample_rate as f32;
        let value = playback.waves.iter().map(|wave| wave.at(sec)).sum();
        let value = T::from_sample(value);
        playback.sample += 1;
        for sample in frame {
            *sample = value;
        }
    }
}

fn err(err: StreamError) {
    eprintln!("Error: {err}");
}
