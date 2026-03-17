use std::sync::Arc;
use std::time::Duration;

use egui_winit::EventResponse;
use instant::Instant;
use tracing::{error};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};
use winit::{
    event::{WindowEvent},
    event_loop::ControlFlow,
};

use crate::gb::{Debugger, GameboyColor};

pub mod gb;

mod pixels;

fn main() -> Result<(), winit::error::EventLoopError> {
    tracing_subscriber::fmt()
        // .with_max_level(tracing::Level::TRACE)
        .with_target(false)
        // .with_thread_names(true)
        // .with_thread_ids(true)
        .init();
    let rom = include_bytes!("../blargg.bin").to_vec();

    // let rom = std::fs::read("test.bin").expect("Could not open test ROM!");
    let emulator = gb::GameboyColor::new(rom);

    // emulator.set_cartridge(&rom).unwrap();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = Application::new();
    app.emulator = Some(emulator);
    app.debugger = Some(gb::Debugger::new());
    event_loop.run_app(&mut app)
}

pub struct Application {
    emulator: Option<GameboyColor>,
    debugger: Option<Debugger>,
    graphics: Option<GraphicsState>,
}

impl Application {
    pub fn new() -> Self {
        Self {
            emulator: None,
            debugger: None,
            graphics: None,
        }
    }
}

struct GraphicsState {
    window: Arc<winit::window::Window>,
    pixels: pixels::Pixels<'static>,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    egui_shapes: Vec<egui::epaint::ClippedPrimitive>,
    next: Instant,
}

impl GraphicsState {
    const CYCLE_TIME: Duration = Duration::new(0, 16600000);
    const CLOCK_SPEED: usize = 4194304;
    const FRAME_RATE: usize = 60;
    const CYCLES_PER_FRAME: usize = Self::CLOCK_SPEED / Self::FRAME_RATE;
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title(
            self.emulator
                .as_ref()
                .map(|gb| format!("Gameboy Emulator - {}", gb.title()))
                .unwrap_or_else(|| "Unknown".to_string()),
        );
        match event_loop.create_window(window_attributes) {
            Ok(window) => {
                let window = Arc::new(window);
                let mut pixels = pollster::block_on(pixels::Pixels::new_async(
                    160,
                    144,
                    pixels::SurfaceTexture::new(&window),
                ))
                .expect("Could not create window!");

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

                self.graphics = Some(GraphicsState {
                    window: window.clone(),
                    pixels,
                    egui_state,
                    egui_renderer,
                    egui_shapes: Vec::new(),
                    next: Instant::now(),
                });
            }
            Err(e) => panic!("Could not create window: {e}"),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(graphics) = self.graphics.as_mut() {
            let EventResponse { consumed, repaint } = graphics
                .egui_state
                .on_window_event(&graphics.window, &event);
            if repaint {
                graphics.window.request_redraw();
            }
            if consumed {
                return;
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
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match key.as_ref() {
                // WARNING: Consider using `key_without_modifiers()` if available on your platform.
                // See the `key_binding` example

                // Key::Character("1") => {
                //     self.state.mode = Mode::Wait;
                //     warn!("mode: {:?}", self.state.mode);
                // },
                // Key::Character("2") => {
                //     self.state.mode = Mode::WaitUntil;
                //     warn!("mode: {:?}", self.state.mode);
                // },
                // Key::Character("3") => {
                //     self.state.mode = Mode::Poll;
                //     warn!("mode: {:?}", self.mode);
                // },
                // Key::Character("r") => {
                //     self.state.request_redraw = !self.state.request_redraw;
                //     warn!("request_redraw: {}", self.request_redraw);
                // },
                // Key::Named(NamedKey::Escape) => {
                //     self.close_requested = true;
                // },
                _ => (),
            },
            WindowEvent::RedrawRequested => {
                let mut step = None;
                if let Some(graphics) = self.graphics.as_mut() {
                    if let Some(emulator) = self.emulator.as_mut() {
                        let new = Instant::now();
                        if new >= graphics.next {
                            let between = new - graphics.next;
                            graphics.next += GraphicsState::CYCLE_TIME;
                            event_loop.set_control_flow(ControlFlow::WaitUntil(graphics.next));
                            step = Some(between);
                        }

                        if let Some(between) = step {
                            while emulator.cycles()
                                <= (GraphicsState::CLOCK_SPEED as f64 * between.as_secs_f64())
                                    as usize
                                && self
                                    .debugger
                                    .as_mut()
                                    .map(|d| d.should_step(emulator))
                                    .unwrap_or(true)
                            {
                                match emulator.cycle() {
                                    Ok(result) => {
                                        if let Some(debugger) = self.debugger.as_mut() {
                                            debugger.on_cycle(result.cpu);
                                        }
                                        if result.render {
                                            emulator.frame_to_rgba(graphics.pixels.frame_mut());
                                        }
                                    }
                                    Err(err) => {
                                        error!("Error during frame: {err}");
                                        if let Some(debugger) = self.debugger.as_mut() {
                                            debugger.error(err);
                                        } else {
                                            event_loop.exit();
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(debugger) = self.debugger.as_mut() {
                            let raw_input = graphics.egui_state.take_egui_input(&graphics.window);

                            let egui_output =
                                graphics.egui_state.egui_ctx().run(raw_input, |ctx| {
                                    debugger.window(emulator, ctx);
                                });

                            graphics.egui_state.handle_platform_output(
                                &graphics.window,
                                egui_output.platform_output,
                            );

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

                            let pixels_per_point =
                                graphics.egui_state.egui_ctx().pixels_per_point();
                            graphics.egui_shapes = graphics
                                .egui_state
                                .egui_ctx()
                                .tessellate(egui_output.shapes, pixels_per_point);
                        }
                    }

                    let window = graphics.window.as_ref();
                    window.pre_present_notify();

                    graphics
                        .pixels
                        .render_with(|encoder, output, ctx| {
                            ctx.scaling_renderer.render(encoder, output);

                            if self.debugger.is_none() {
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
            _ => (),
        }
    }
}
