pub extern crate pixels;

use std::sync::Arc;
use std::time::Duration;

use egui_winit::EventResponse;
use gameboy_core::util::Controls;

use pixels::winit::event_loop::EventLoopProxy;
#[cfg(target_family = "wasm")]
use web_time::Instant;

#[cfg(not(target_family = "wasm"))]
use std::time::Instant;

use pixels::winit::keyboard::{Key, NamedKey};
use tracing::error;

use pixels::winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
    {event::WindowEvent, event_loop::ControlFlow},
};

use pixels::wgpu;

use crate::debugger::Debugger;
use gameboy_core::{Cartridge, Cycles, GameboyColor};

mod debugger;

pub struct Application<W: CreateWindow> {
    pub emulator: Emulator,
    pub cartridge: Option<Box<dyn Cartridge + 'static>>,
    graphics: Option<GraphicsState>,
    proxy: Arc<EventLoopProxy<GraphicsState>>,
    event_loop: Option<EventLoop<GraphicsState>>,
    _w: std::marker::PhantomData<W>,
}

impl<W: CreateWindow> Application<W> {
    pub fn new(debugger: bool) -> Self {
        let event_loop = EventLoop::<GraphicsState>::with_user_event().build().unwrap_or_else(|e| panic!("Could not create event loop with error {e}"));
        event_loop.set_control_flow(ControlFlow::Poll);
        Self {
            emulator: Emulator::new(debugger),
            cartridge: None,
            graphics: None,
            proxy: Arc::new(event_loop.create_proxy()),
            event_loop: Some(event_loop),
            _w: std::marker::PhantomData,
        }
    }

    pub fn run(mut self) {
        let el = self.event_loop.take();
        el.unwrap().run_app(&mut self).unwrap_or_else(|e| panic!("Could not run event loop with error {e}"));
    }
}

pub struct Emulator {
    gameboy: GameboyColor,
    debugger: Option<Debugger>,
}

pub enum ApplicationUpdate {
    Continue,
    Render,
    WaitUntil(Instant),
    Exit,
}

impl Emulator {
    pub fn new(debugger: bool) -> Self {
        match debugger {
            true => {
                let mut debugger = Debugger::new();
                Self {
                    gameboy: GameboyColor::with_serial_callback(debugger.create_serial_callback()),
                    debugger: Some(debugger),
                }
            },
            false => {
                Self {
                    gameboy: GameboyColor::default(),
                    debugger: None,
                }
            },
        }
    }

    pub fn update<const LOG: bool>(
        &mut self,
        cart: &mut dyn Cartridge,
        next: Option<&mut Instant>,
    ) -> ApplicationUpdate {
        let mut update = ApplicationUpdate::Continue;

        let max_cycles = next.map(|next| {
            let new = Instant::now();
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
            match self.gameboy.cycle(cart) {
                Ok(result) => {
                    cycles += result.cpu.cycles;
                    if let Some(debugger) = self.debugger.as_mut() {
                        debugger.on_cycle(result.cpu);
                    }
                    if result.render {
                        return ApplicationUpdate::Render;
                    }
                }
                Err(err) => {
                    error!("Error during frame: {err}");
                    if let Some(debugger) = self.debugger.as_mut() {
                        debugger.error(err);
                    } else {
                        return ApplicationUpdate::Exit;
                    }
                }
            }
        }
        update
    }
}

pub struct GraphicsState {
    window: Arc<Window>,
    pixels: pixels::Pixels,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    egui_shapes: Vec<egui::epaint::ClippedPrimitive>,
    next: Instant,
}

impl GraphicsState {
    const CYCLE_TIME: Duration = Duration::new(0, 16600000);
    const CLOCK_SPEED: usize = 4194304;
    // const FRAME_RATE: usize = 60;
    // const CYCLES_PER_FRAME: usize = Self::CLOCK_SPEED / Self::FRAME_RATE;

    pub async fn new(window: Window) -> GraphicsState {
        let window = Arc::new(window);

        let mut pixels = pixels::Pixels::new(160, 144, pixels::SurfaceTexture::new(&window)).await
            .unwrap_or_else(|e| panic!("Could not initialize graphics with error: {e}"));

        pixels.clear_color(wgpu::Color::GREEN);

        let egui_state = egui_winit::State::new(
            Default::default(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            pixels.device(),
            pixels.render_texture_format(),
            Default::default(),
        );

        GraphicsState {
            window: window.clone(),
            pixels,
            egui_state,
            egui_renderer,
            egui_shapes: Vec::new(),
            next: Instant::now(),
        }
    }

}

pub trait CreateWindow {
    fn create_window(proxy: &Arc<EventLoopProxy<GraphicsState>>, event_loop: &ActiveEventLoop);
}

impl<W: CreateWindow> ApplicationHandler<GraphicsState> for Application<W> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        W::create_window(&self.proxy, event_loop);
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: GraphicsState) {
        self.graphics = Some(event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(graphics) = self.graphics.as_mut() {
            let EventResponse { repaint, .. } = graphics
                .egui_state
                .on_window_event(&graphics.window, &event);
            if repaint {
                graphics.window.request_redraw();
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(window) = self.graphics.as_ref().map(|g| &g.window) {
                    if window.id() == window_id {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(graphics) = self.graphics.as_mut() {
                    if let Err(e) = graphics.pixels.resize(new_size) {
                        error!("Failed to resize pixels: {}", e);
                    }
                }
            }
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
                                self.emulator
                                    .gameboy
                                    .frame_to_rgba(graphics.pixels.frame_mut());
                            }
                        }
                    }
                }

                if let Some(graphics) = self.graphics.as_mut() {
                    if let Some((debugger, cart)) =
                        self.emulator.debugger.as_mut().zip(self.cartridge.as_deref())
                    {
                        let raw_input = graphics.egui_state.take_egui_input(&graphics.window);

                        let egui_output = graphics.egui_state.egui_ctx().run(raw_input, |ctx| {
                            debugger.window(
                                cart,
                                &mut self.emulator.gameboy,
                                ctx,
                                graphics.window.inner_size(),
                            );
                        });

                        graphics
                            .egui_state
                            .handle_platform_output(&graphics.window, egui_output.platform_output);

                        for (id, image_delta) in egui_output.textures_delta.set {
                            graphics.egui_renderer.update_texture(
                                graphics.pixels.device(),
                                graphics.pixels.queue(),
                                id,
                                &image_delta,
                            );
                        }

                        for id in egui_output.textures_delta.free {
                            graphics.egui_renderer.free_texture(&id);
                        }

                        let pixels_per_point = graphics.egui_state.egui_ctx().pixels_per_point();
                        graphics.egui_shapes = graphics
                            .egui_state
                            .egui_ctx()
                            .tessellate(egui_output.shapes, pixels_per_point);
                    }

                    let window = graphics.window.as_ref();
                    window.pre_present_notify();

                    graphics
                        .pixels
                        .render_with(|encoder, output, ctx| {
                            ctx.scaling_renderer.render(encoder, output);

                            if self.emulator.debugger.is_none() {
                                return Ok(());
                            }

                            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                pixels_per_point: window.scale_factor() as f32,
                                size_in_pixels: window.inner_size().into(),
                            };

                            let cmd_buffers = graphics.egui_renderer.update_buffers(
                                &ctx.device,
                                &ctx.queue,
                                encoder,
                                &graphics.egui_shapes,
                                &screen_descriptor,
                            );

                            ctx.queue.submit(cmd_buffers);

                            let mut egui_pass = encoder
                                .begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("egui"),
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &output,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Load,
                                            store: wgpu::StoreOp::Store,
                                        },
                                        depth_slice: None,
                                    })],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                })
                                .forget_lifetime();

                            graphics.egui_renderer.render(
                                &mut egui_pass,
                                &graphics.egui_shapes,
                                &screen_descriptor,
                            );

                            Ok(())
                        })
                        .unwrap();
                }
            }
            WindowEvent::DroppedFile(path) => match std::fs::read(path) {
                Ok(rom) => {
                    let cart = GameboyColor::load(rom);
                    self.emulator.gameboy.reset(&*cart);
                    self.cartridge = Some(cart);
                    if let Some(graphics) = self.graphics.as_mut() {
                        graphics.window.set_title(&format!(
                            "Gameboy Emulator - {}",
                            self.cartridge.as_ref().unwrap().title()
                        ));
                    }
                }
                Err(err) => error!("Could not open ROM with error: {err}"),
            },
            _ => (),
        }
    }
}
