use std::time::Duration;

use egui::ahash::HashMap;
use egui_winit::EventResponse;
use instant::Instant;
use winit::application::ApplicationHandler;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::State as EguiState;

use crate::gb::{Debugger, GameboyColor};
use crate::pixels::{Pixels, SurfaceTexture};

pub struct Application {
    window: Option<GameWindow>,
    game: Option<GameboyColor>,
    debugger: Debugger,
    next: Instant,
}

struct GameWindow {
    window: Window,
    pixels: Pixels<'static>,
}

impl Application {

    const CYCLE_TIME: Duration = Duration::new(0, 16600000);
    const FRAME_RATE: usize = 60;
    const CYCLES_PER_FRAME: usize = GameboyColor::CLOCK_SPEED / Self::FRAME_RATE;

    pub fn new() -> Self {

        Self {
            window: None,
            next: Instant::now(),
        }
    }
}

impl ApplicationHandler for Application {
    

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.take() {
            panic!("todo use old window!");
        }
        self.window = Some(GameWindow::new(&mut self, event_loop));
        self.next = Instant::now();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let EventResponse { consumed, repaint } = egui_state.on_window_event(&window, &event);
        if repaint {
            self.window.request_redraw();
        }
        if consumed {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                if window_id == self.window.id() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                self.pixels.resize_surface(size.width, size.height).unwrap();
            }
            WindowEvent::RedrawRequested => {
                if window_id == window.id() {
                    let raw_input = egui_state.take_egui_input(&window);

                    let egui_output = egui_state.egui_ctx().run(raw_input, |ctx| {
                        debugger.window(&mut emulator, ctx);
                    });

                    egui_state.handle_platform_output(&window, egui_output.platform_output);

                    for (id, image_delta) in egui_output.textures_delta.set {
                        egui_renderer.update_texture(
                            pixels.device(),
                            pixels.queue(),
                            id,
                            &image_delta,
                        );
                    }

                    for id in egui_output.textures_delta.free {
                        egui_renderer.free_texture(&id);
                    }

                    let pixels_per_point = egui_state.egui_ctx().pixels_per_point();
                    egui_shapes = egui_state
                        .egui_ctx()
                        .tessellate(egui_output.shapes, pixels_per_point);

                    pixels
                        .render_with(|encoder, output, ctx| {
                            ctx.scaling_renderer.render(encoder, output);

                            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                pixels_per_point: window.scale_factor() as f32,
                                size_in_pixels: window.inner_size().into(),
                            };

                            let cmd_buffers = egui_renderer.update_buffers(
                                &ctx.device,
                                &ctx.queue,
                                encoder,
                                &egui_shapes,
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
                                    })],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                })
                                .forget_lifetime();

                            self.egui_renderer.render(&mut egui_pass, &self.egui_shapes, &screen_descriptor);

                            Ok(())
                        })
                        .unwrap();
                }
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let new = Instant::now();
        if &new >= &self.next {
            let between = new - self.next;
            self.next += CYCLE_TIME;
            event_loop.set_control_flow(ControlFlow::WaitUntil(next));
            if let Err(err) = self.emulator.frame(|gb| self.debugger.should_step(gb)) {
                self.debugger.error(err);
            }
            let fb = self.pixels.frame_mut();
            for (i, color) in bytemuck::cast_slice(&self.emulator.memory.ppu.frame_buffer)
                .iter()
                .enumerate()
            {
                fb[i] = *color;
            }
            // println!("{fb:?}");
            self.window.request_redraw();
        }
    }
}
