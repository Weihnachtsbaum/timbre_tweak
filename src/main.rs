use std::{env, f32::consts::TAU, fs, sync::Arc, thread};

use cpal::{
    Device, FromSample, I24, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use eframe::{
    App, NativeOptions,
    egui::{
        CentralPanel, Context, DragValue, Popup, PopupCloseBehavior, ScrollArea, Slider, Ui,
        mutex::Mutex,
    },
};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};

fn main() {
    let app = MyApp(Arc::new(Mutex::new(Playback {
        stream_config: None,
        stream: None,
        sample: 0,
        hz: 440.0,
        timbre: Timbre {
            amp: Curve(vec![0.5]),
            waves: vec![],
        },
    })));
    setup_audio(app.clone());
    eframe::run_native(
        "Timbre Tweak",
        NativeOptions::default(),
        Box::new(|_| Ok(Box::new(app))),
    )
    .expect("Error running eframe App");
}

#[derive(PartialEq, Serialize, Deserialize)]
enum Waveform {
    Sine,
    Triangle,
    Sawtooth,
    Square,
    WhiteNoise,
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
            Self::WhiteNoise => {
                let mut n = t.to_bits();
                // fmix32 (MurmurHash3)
                n = (n ^ n >> 16).wrapping_mul(0x85EBCA6B);
                n = (n ^ n >> 13).wrapping_mul(0xC2B2AE35);
                n ^= n >> 16;
                n as f32 / i32::MAX as f32 - 1.0
            }
        }
    }
}

/// A linearly-interpolated curve in range 0.0..1.0
#[derive(Serialize, Deserialize)]
struct Curve(Vec<f32>);

impl Curve {
    fn at(&self, t: f32) -> f32 {
        let i = t * (self.0.len() - 1) as f32;
        let (fract, i) = (i.fract(), i as usize);
        if i == self.0.len() - 1 {
            self.0[i]
        } else {
            (1.0 - fract) * self.0[i] + fract * self.0[i + 1]
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Wave {
    waveform: Waveform,
    freq: Curve,
    amp: Curve,
}

impl Wave {
    fn at(&self, sec: f32, hz: f32) -> f32 {
        self.waveform.at(sec * hz * self.freq.at(sec)) * self.amp.at(sec)
    }
}

#[derive(Serialize, Deserialize)]
struct Timbre {
    amp: Curve,
    waves: Vec<Wave>,
}

struct Playback {
    stream_config: Option<StreamConfig>,
    stream: Option<Stream>,
    sample: u32,
    hz: f32,
    timbre: Timbre,
}

#[derive(Clone)]
struct MyApp(Arc<Mutex<Playback>>);

impl App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // TODO: don't block audio thread
        let mut playback = self.0.lock();
        CentralPanel::default().show(ctx, |ui| {
            ui.add(Slider::new(&mut playback.hz, 20.0..=2000.0).text("hz"));
            if ui.button("Add wave").clicked() {
                playback.timbre.waves.push(Wave {
                    waveform: Waveform::Sine,
                    freq: Curve(vec![1.0]),
                    amp: Curve(vec![0.5]),
                });
            }
            ui.add_space(25.0);
            if ui.button("Save").clicked() {
                if let Some(path) = file_dialog().save_file()
                    && let Ok(str) = serde_json::to_string(&playback.timbre)
                    && let Ok(()) = fs::write(path, str)
                {
                } else {
                    eprintln!("File save failed");
                }
            }
            if ui.button("Load").clicked() {
                if let Some(path) = file_dialog().pick_file()
                    && let Ok(slice) = fs::read(path)
                    && let Ok(timbre) = serde_json::from_slice(&slice)
                {
                    playback.timbre = timbre;
                } else {
                    eprintln!("File load failed");
                }
            }
            ui.add_space(25.0);
            ui_curve(ui, &mut playback.timbre.amp, "Global volume");
            let mut i = 0;
            let len = playback.timbre.waves.len();
            let mut swap = vec![];
            ScrollArea::vertical().show(ui, |ui| {
                playback
                    .timbre
                    .waves
                    .retain_mut(|wave| wave_ui(wave, &mut i, len, &mut swap, ui));
            });
            for (i1, i2) in swap {
                playback.timbre.waves.swap(i1, i2);
            }
        });
    }
}

fn file_dialog() -> FileDialog {
    FileDialog::new()
        .set_directory(env::current_dir().expect("Could not get current dir"))
        .add_filter("JSON", &["json"])
}

fn ui_curve(ui: &mut Ui, curve: &mut Curve, label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.button("+").clicked() {
            curve.0.push(*curve.0.last().unwrap());
        }
        if ui.button("-").clicked() && curve.0.len() > 1 {
            curve.0.pop();
        }
        for v in curve.0.iter_mut() {
            ui.add(DragValue::new(v).range(0.0..=f32::INFINITY).speed(0.01));
        }
    });
}

fn wave_ui(
    wave: &mut Wave,
    i: &mut usize,
    len: usize,
    swap: &mut Vec<(usize, usize)>,
    ui: &mut Ui,
) -> bool {
    ui.add_space(25.0);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            if ui.button("^").clicked() && *i != 0 {
                swap.push((*i, *i - 1));
            }
            if ui.button("v").clicked() && *i != len - 1 {
                swap.push((*i, *i + 1));
            }
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui_curve(ui, &mut wave.amp, "Volume");
            ui_curve(ui, &mut wave.freq, "Relative frequency");

            let response = ui.button("Waveform");
            Popup::menu(&response)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.selectable_value(&mut wave.waveform, Waveform::Sine, "Sine");
                    ui.selectable_value(&mut wave.waveform, Waveform::Triangle, "Triangle");
                    ui.selectable_value(&mut wave.waveform, Waveform::Sawtooth, "Sawtooth");
                    ui.selectable_value(&mut wave.waveform, Waveform::Square, "Square");
                    ui.selectable_value(&mut wave.waveform, Waveform::WhiteNoise, "White noise");
                });

            let retain = !ui.button("Remove wave").clicked();
            if retain {
                *i += 1;
            }
            retain
        })
    })
    .inner
    .inner
}

fn setup_audio(app: MyApp) {
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
        cpal::SampleFormat::I8 => setup_stream::<i8>(device, config, app),
        cpal::SampleFormat::I16 => setup_stream::<i16>(device, config, app),
        cpal::SampleFormat::I24 => setup_stream::<I24>(device, config, app),
        cpal::SampleFormat::I32 => setup_stream::<i32>(device, config, app),
        cpal::SampleFormat::I64 => setup_stream::<i64>(device, config, app),
        cpal::SampleFormat::U8 => setup_stream::<u8>(device, config, app),
        cpal::SampleFormat::U16 => setup_stream::<u16>(device, config, app),
        cpal::SampleFormat::U32 => setup_stream::<u32>(device, config, app),
        cpal::SampleFormat::U64 => setup_stream::<u64>(device, config, app),
        cpal::SampleFormat::F32 => setup_stream::<f32>(device, config, app),
        cpal::SampleFormat::F64 => setup_stream::<f64>(device, config, app),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }
}

fn setup_stream<T: SizedSample + FromSample<f32> + 'static>(
    device: Device,
    config: StreamConfig,
    app: MyApp,
) {
    let app1 = app.clone();
    let app2 = app.clone();
    let stream = device
        .build_output_stream(
            &config,
            move |data, _| write_data::<T>(data, app1.clone()),
            move |err| {
                eprintln!("Error: {err}\nRetrying...");
                let app = app2.clone();
                thread::spawn(move || setup_audio(app));
            },
            None,
        )
        .expect("Could not build output stream");
    stream.play().expect("Could not play stream");
    let mut lock = app.0.lock();
    lock.stream_config = Some(config.clone());
    lock.stream = Some(stream);
}

fn write_data<T: SizedSample + FromSample<f32>>(data: &mut [T], app: MyApp) {
    let mut playback = app.0.lock();
    let Some(stream_config) = playback.stream_config.clone() else {
        eprintln!("Error: No stream config");
        return;
    };
    for frame in data.chunks_mut(stream_config.channels as usize) {
        let sec = playback.sample as f32 / stream_config.sample_rate.0 as f32;
        let value = playback
            .timbre
            .waves
            .iter()
            .map(|wave| wave.at(sec, playback.hz))
            .sum::<f32>()
            * playback.timbre.amp.at(sec);
        let value = T::from_sample(value);
        playback.sample = (playback.sample + 1) % stream_config.sample_rate.0;
        for sample in frame {
            *sample = value;
        }
    }
}
