use egui_winit::EventResponse;
use gameboy_core::Cartridge;
use gameboy_core::bus::Pixel;
use pixels::wgpu;
use pixels::winit::event::WindowEvent;
use pixels::winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use pixels::winit::window::{Window, WindowId};
use tracing::error;

use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_family = "wasm"))]
pub use std::time::Instant;
#[cfg(target_family = "wasm")]
pub use web_time::Instant;

use crate::EmulatorEvent;

pub struct GraphicsState {
    window: Arc<Window>,
    pixels: pixels::Pixels,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    egui_shapes: Vec<egui::epaint::ClippedPrimitive>,
    pub next: Instant,
}

impl GraphicsState {
    pub const CYCLE_TIME: Duration = Duration::new(0, 16600000);
    pub const CLOCK_SPEED: usize = 4194304;
    // const FRAME_RATE: usize = 60;
    // const CYCLES_PER_FRAME: usize = Self::CLOCK_SPEED / Self::FRAME_RATE;

    pub async fn new(window: Window) -> GraphicsState {
        let window = Arc::new(window);

        let mut pixels = pixels::Pixels::new(160, 144, pixels::SurfaceTexture::new(&window))
            .await
            .unwrap_or_else(|e| panic!("Could not initialize graphics with error: {e}"));

        pixels.clear_color(pixels::wgpu::Color::GREEN);

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

    pub(crate) fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: &WindowEvent,
    ) -> bool {
        let EventResponse { repaint, .. } = self.egui_state.on_window_event(&self.window, &event);
        if repaint {
            self.window.request_redraw();
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.window.id() == window_id {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Err(e) = self.pixels.resize(new_size) {
                    error!("Failed to resize pixels: {}", e);
                }
            }
            _ => {}
        }
        true
    }

    pub(crate) fn redraw<P: crate::EmulatorPlatform>(
        &mut self,
        proxy: &Arc<EventLoopProxy<EmulatorEvent>>,
        emulator: &mut crate::Emulator,
        cartridge: Option<&dyn Cartridge>,
    ) {
        let raw_input = self.egui_state.take_egui_input(&self.window);

        let egui_output = self.egui_state.egui_ctx().run(raw_input, |ctx| {
            if let Some((debugger, cart)) = emulator.debugger.as_mut().zip(cartridge) {
                debugger.window(cart, &mut emulator.gameboy, ctx, self.window.inner_size());
            }
            super::Application::<P>::menu(proxy, cartridge, ctx);
        });

        self.egui_state
            .handle_platform_output(&self.window, egui_output.platform_output);

        for (id, image_delta) in egui_output.textures_delta.set {
            self.egui_renderer.update_texture(
                self.pixels.device(),
                self.pixels.queue(),
                id,
                &image_delta,
            );
        }

        for id in egui_output.textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }

        let pixels_per_point = self.egui_state.egui_ctx().pixels_per_point();
        self.egui_shapes = self
            .egui_state
            .egui_ctx()
            .tessellate(egui_output.shapes, pixels_per_point);

        self.window.pre_present_notify();

        self.pixels
            .render_with(|encoder, output, ctx| {
                ctx.scaling_renderer.render(encoder, output);

                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                    pixels_per_point: self.window.scale_factor() as f32,
                    size_in_pixels: self.window.inner_size().into(),
                };

                let cmd_buffers = self.egui_renderer.update_buffers(
                    &ctx.device,
                    &ctx.queue,
                    encoder,
                    &self.egui_shapes,
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

                self.egui_renderer
                    .render(&mut egui_pass, &self.egui_shapes, &screen_descriptor);

                Ok(())
            })
            .unwrap();
    }

    pub(crate) fn load(&self, cart: &dyn Cartridge) {
        self.window
            .set_title(&format!("Gameboy Emulator - {}", cart.title()));
    }

    pub(crate) fn update_frame(&mut self, gameboy: &gameboy_core::GameboyColor) {
        self.pixels.frame_mut()
            .as_chunks_mut::<4>()
            .0
            .into_iter()
            .zip(gameboy.framebuffer())
            .for_each(convert_pixel);
    }
}

const fn convert_pixel((data, pixel): (&mut [u8; 4], &Pixel)) {
    *data = match *pixel {
        Pixel::Monochrome(shade) => match shade {
            0 => [0xE0, 0xF8, 0xD0, 0xFF],
            1 => [0x88, 0xC0, 0x70, 0xFF],
            2 => [0x34, 0x68, 0x56, 0xFF],
            3 => [0x08, 0x18, 0x20, 0xFF],
            _ => unreachable!(),
        },
        Pixel::Rgb([r, g, b]) => [
            ((r * 13 + g * 2 + b) >> 1) as u8,
            ((g * 3 + b) << 1) as u8,
            ((r * 3 + g * 2 + b * 11) >> 1) as u8,
            0xFF,
        ],
    };
}
