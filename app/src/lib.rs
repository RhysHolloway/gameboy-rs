pub extern crate pixels;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gameboy_core::util::Controls;

use pixels::winit::event_loop::EventLoopProxy;

use pixels::winit::keyboard::{Key, NamedKey};
use tracing::{error, info};

use pixels::winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
    {event::WindowEvent, event_loop::ControlFlow},
};

use crate::debugger::Debugger;
use gameboy_core::{Cartridge, Cycles, GameboyColor};

mod debugger;
mod graphics;

pub use graphics::GraphicsState;

pub struct Application<P: EmulatorPlatform> {
    pub emulator: Emulator,
    pub cartridge: Option<Box<dyn Cartridge + 'static>>,
    graphics: Option<GraphicsState>,
    audio: Option<Audio>,
    proxy: Arc<EventLoopProxy<EmulatorEvent>>,
    event_loop: Option<EventLoop<EmulatorEvent>>,
    _p: std::marker::PhantomData<P>,
}

struct Audio {
    sample_rate: usize,
    buffer: Arc<Mutex<VecDeque<[f32; 2]>>>,
    _stream: cpal::Stream,
}

impl Audio {
    const MAX_BUFFERED_MS: usize = 100;

    fn callback(&self) -> Box<dyn FnMut([f32; 2], usize) + 'static> {
        let buffer = Arc::clone(&self.buffer);
        let max_buffered_frames = (self.sample_rate * Self::MAX_BUFFERED_MS / 1000).max(1);
        let sample_rate = self.sample_rate as u64;
        let mut sample_counter = 0_u64;
        const CLOCK_SPEED: u64 = GameboyColor::CLOCK_SPEED as u64;

        Box::new(move |sample, cycles| {
            sample_counter += cycles as u64 * sample_rate;
            let frames = sample_counter / CLOCK_SPEED;
            sample_counter %= CLOCK_SPEED;

            if frames == 0 {
                return;
            }

            if let Ok(mut buffer) = buffer.lock() {
                for _ in 0..frames {
                    while buffer.len() >= max_buffered_frames {
                        buffer.pop_front();
                    }
                    buffer.push_back(sample);
                }
            }
        })
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
        let buffer = Arc::new(Mutex::new(VecDeque::new()));

        info!(
            "Using audio device: {}, config: {:?}, sample format: {:?}",
            device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "Unknown".to_string()),
            config,
            sample_format
        );

        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                Self::build_stream_f32(&device, &config, channels, Arc::clone(&buffer))
            }
            cpal::SampleFormat::I16 => {
                Self::build_stream_i16(&device, &config, channels, Arc::clone(&buffer))
            }
            cpal::SampleFormat::U16 => {
                Self::build_stream_u16(&device, &config, channels, Arc::clone(&buffer))
            }
            format => {
                error!("Unsupported output audio sample format: {format:?}");
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
            buffer,
            _stream: stream,
        })
    }

    fn build_stream_f32(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        channels: usize,
        buffer: Arc<Mutex<VecDeque<[f32; 2]>>>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError> {
        device.build_output_stream(
            config,
            move |data: &mut [f32], _| Self::write_output(data, channels, &buffer, |sample| sample),
            |err| error!("Audio stream error: {err}"),
            None,
        )
    }

    fn build_stream_i16(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        channels: usize,
        buffer: Arc<Mutex<VecDeque<[f32; 2]>>>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError> {
        device.build_output_stream(
            config,
            move |data: &mut [i16], _| {
                Self::write_output(data, channels, &buffer, |sample| {
                    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                })
            },
            |err| error!("Audio stream error: {err}"),
            None,
        )
    }

    fn build_stream_u16(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        channels: usize,
        buffer: Arc<Mutex<VecDeque<[f32; 2]>>>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError> {
        device.build_output_stream(
            config,
            move |data: &mut [u16], _| {
                Self::write_output(data, channels, &buffer, |sample| {
                    ((sample.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16
                })
            },
            |err| error!("Audio stream error: {err}"),
            None,
        )
    }

    fn write_output<T>(
        data: &mut [T],
        channels: usize,
        buffer: &Arc<Mutex<VecDeque<[f32; 2]>>>,
        convert: impl Fn(f32) -> T,
    ) {
        let mut buffer = buffer.try_lock().ok();

        for frame in data.chunks_mut(channels) {
            let sample = buffer
                .as_mut()
                .and_then(|buffer| buffer.pop_front())
                .unwrap_or([0.0, 0.0]);

            for (channel, output) in frame.iter_mut().enumerate() {
                *output = convert(sample[channel.min(1)]);
            }
        }
    }
}

impl<P: EmulatorPlatform> Application<P> {
    pub fn new(debugger: bool, rom: Option<Vec<u8>>) -> Self {
        let event_loop = EventLoop::<EmulatorEvent>::with_user_event()
            .build()
            .unwrap_or_else(|e| panic!("Could not create event loop with error {e}"));
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut this = Self {
            emulator: Emulator::new(debugger),
            cartridge: None,
            graphics: None,
            proxy: Arc::new(event_loop.create_proxy()),
            event_loop: Some(event_loop),
            _p: std::marker::PhantomData,
            audio: None,
        };
        if let Some(rom) = rom {
            this.open_rom(rom);
        }
        this
    }

    pub fn run(mut self) {
        let el = self.event_loop.take();
        el.unwrap()
            .run_app(&mut self)
            .unwrap_or_else(|e| panic!("Could not run event loop with error {e}"));
    }

    fn open_rom(&mut self, rom: Vec<u8>) {
        match GameboyColor::load(rom) {
            Ok(cart) => {
                self.emulator.gameboy.reset(&*cart);
                if let Some(graphics) = self.graphics.as_mut() {
                    graphics.load(&*cart);
                }
                self.cartridge = Some(cart);
            }
            Err(err) => error!("Could not load cartridge with error: {err}"),
        }
    }

    fn open_ram(&mut self, ram: Vec<u8>) {
        if let Some(cart) = self.cartridge.as_mut() {
            if ram.len() != cart.ram().len() {
                error!(
                    "Could not load RAM with size {}, expected {}",
                    ram.len(),
                    cart.ram().len()
                );
                return;
            }
            cart.ram_mut().copy_from_slice(&ram);
            self.emulator.gameboy.reset(&**cart);
        }
    }
    fn open(proxy: &Arc<EventLoopProxy<EmulatorEvent>>, kind: DataType) {
        let proxy = proxy.clone();
        P::run_async(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter(
                    match kind {
                        DataType::Rom => "Gameboy ROM",
                        DataType::Ram => "Gameboy RAM Save",
                    },
                    match kind {
                        DataType::Rom => &["gb", "gbc"],
                        DataType::Ram => &["rsav"],
                    },
                )
                .pick_file()
                .await
            {
                if let Err(err) = proxy.send_event(EmulatorEvent::OpenFile(file.read().await, kind))
                {
                    error!("Could not send open file event with error: {err}");
                }
            }
        });
    }

    fn save(name: String, data: Vec<u8>) {
        P::run_async(async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_file_name(name)
                .add_filter("Gameboy Save", &["rsav"])
                .save_file()
                .await
            {
                if let Err(err) = handle.write(&data).await {
                    error!("Could not save file with error: {err}");
                }
            }
        });
    }

    fn try_create_audio(&mut self) {
        if self.audio.is_some() {
            return;
        }

        if let Some(audio) = Audio::new() {
            info!("Audio output created");
            self.emulator
                .gameboy
                .bus
                .set_audio_callback(Some(audio.callback()));
            self.audio = Some(audio);
        }
    }

    fn menu(
        proxy: &Arc<EventLoopProxy<EmulatorEvent>>,
        cartridge: Option<&dyn Cartridge>,
        ctx: &egui::Context,
    ) {
        egui::Window::new("Menu").show(ctx, |ui| {
            if ui.button("Open ROM").clicked() {
                Self::open(proxy, DataType::Rom);
            }
            if let Some(cart) = cartridge {
                ui.separator();
                if ui.button("Save RAM").clicked() {
                    Self::save(format!("{}.rsav", cart.title()), cart.ram().to_vec());
                }
                if ui.button("Load RAM").clicked() {
                    Self::open(proxy, DataType::Ram);
                }
            }
        });
    }
}

pub struct Emulator {
    gameboy: GameboyColor,
    debugger: Option<Debugger>,
}

pub enum ApplicationUpdate {
    Continue,
    Render,
    WaitUntil(graphics::Instant),
    Exit,
}

impl Emulator {
    pub fn new(debugger: bool) -> Self {
        let mut gameboy = GameboyColor::default();
        Self {
            debugger: debugger.then(|| {
                let mut debugger = Debugger::default();
                gameboy
                    .bus
                    .set_serial_callback(Some(debugger.create_serial_callback()));
                debugger
            }),
            gameboy,
        }
    }

    pub fn update<const LOG: bool>(
        &mut self,
        cart: &mut dyn Cartridge,
        next: Option<&mut graphics::Instant>,
    ) -> ApplicationUpdate {
        let mut update = ApplicationUpdate::Continue;

        let max_cycles = next.map(|next| {
            let new = graphics::Instant::now();
            let between = new - *next;
            *next += GraphicsState::CYCLE_TIME;
            update = ApplicationUpdate::WaitUntil(*next);
            (GraphicsState::CLOCK_SPEED as f64
                * between.as_secs_f64()
                * self.debugger.as_mut().map(|d| d.speed()).unwrap_or(1.0)) as usize
        });

        let mut cycles = Cycles::new(0);

        while self
            .debugger
            .as_mut()
            .map(|d| d.should_step(&self.gameboy))
            .unwrap_or(true)
            && max_cycles.map(|max| cycles <= max).unwrap_or(true)
        {
            if LOG {
                if let Some(debugger) = self.debugger.as_mut() {
                    debugger.log(cart, &self.gameboy);
                }
            }
            let result = self.gameboy.cycle(cart);
            cycles += result.cpu.cycles;
            if let Some(debugger) = self.debugger.as_mut() {
                debugger.on_cycle(result.cpu);
            }
            if result.render {
                return ApplicationUpdate::Render;
            }
        }
        update
    }
}

pub enum DataType {
    Rom,
    Ram,
}

pub enum EmulatorEvent {
    CreateGraphics(GraphicsState),
    OpenFile(Vec<u8>, DataType),
}

pub trait EmulatorPlatform {
    fn run_async(future: impl std::future::Future<Output = ()> + Send + 'static);

    fn create_window(proxy: &Arc<EventLoopProxy<EmulatorEvent>>, event_loop: &ActiveEventLoop);
}

impl<E: EmulatorPlatform> ApplicationHandler<EmulatorEvent> for Application<E> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        E::create_window(&self.proxy, event_loop);
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: EmulatorEvent) {
        match event {
            EmulatorEvent::CreateGraphics(graphics) => {
                self.graphics = Some(graphics);
                self.try_create_audio();
                // self.audio = Some(crate::audio::AudioState::new());
            }
            EmulatorEvent::OpenFile(data, kind) => match kind {
                DataType::Rom => self.open_rom(data),
                DataType::Ram => self.open_ram(data),
            },
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(graphics) = self.graphics.as_mut() {
            if !graphics.window_event(event_loop, window_id, &event) {
                return;
            }
        }
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: key,
                        state,
                        ..
                    },
                ..
            } => {
                // tracing::info!("Key event: {key:?} is {:?}\r", state);
                if let Some(control) = self
                    .cartridge
                    .is_some()
                    .then(|| {
                        Some(match key.as_ref() {
                            Key::Named(NamedKey::ArrowUp) => Controls::Up,
                            Key::Named(NamedKey::ArrowDown) => Controls::Down,
                            Key::Named(NamedKey::ArrowLeft) => Controls::Left,
                            Key::Named(NamedKey::ArrowRight) => Controls::Right,
                            Key::Character("a") => Controls::Start,
                            Key::Character("s") => Controls::Select,
                            Key::Character("z") => Controls::A,
                            Key::Character("x") => Controls::B,
                            _ => return None,
                        })
                    })
                    .flatten()
                {
                    self.emulator
                        .gameboy
                        .update_input(control, state == ElementState::Pressed);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(cart) = self.cartridge.as_deref_mut() {
                    match self
                        .emulator
                        .update::<false>(cart, self.graphics.as_mut().map(|g| &mut g.next))
                    {
                        ApplicationUpdate::Continue => (),
                        ApplicationUpdate::Exit => event_loop.exit(),
                        ApplicationUpdate::WaitUntil(instant) => {
                            event_loop.set_control_flow(ControlFlow::WaitUntil(instant))
                        }
                        ApplicationUpdate::Render => {
                            if let Some(graphics) = self.graphics.as_mut() {
                                graphics.update_frame(&self.emulator.gameboy);
                            }
                        }
                    }
                }

                if let Some(graphics) = self.graphics.as_mut() {
                    graphics.redraw::<E>(
                        &self.proxy,
                        &mut self.emulator,
                        self.cartridge.as_deref(),
                    );
                }
            }
            WindowEvent::DroppedFile(path) => match std::fs::read(path) {
                Ok(rom) => self.open_rom(rom),
                Err(err) => error!("Could not open ROM with error: {err}"),
            },
            _ => (),
        }
    }
}
