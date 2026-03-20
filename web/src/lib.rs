#![allow(unused)]

use std::sync::Arc;

use app::pixels::winit::dpi::PhysicalSize;
use app::pixels::winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use app::pixels::winit::window::Window;
use app::{Application, EmulatorEvent, EmulatorPlatform, GraphicsState};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
fn main() {
    console_error_panic_hook::set_once();
    Application::<Web>::new(false, None).run();
}

struct Web;

impl EmulatorPlatform for Web {
    fn create_window(proxy: &Arc<EventLoopProxy<EmulatorEvent>>, event_loop: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        {
            use app::pixels::winit::platform::web::WindowAttributesExtWebSys;

            let canvas = web_sys::window().unwrap_throw().document().unwrap_throw().get_element_by_id("gameboy-canvas").unwrap_throw().unchecked_into();

            let window = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_canvas(Some(canvas))
                        .with_title("Gameboy Emulator"),
                )
                .unwrap_or_else(|e| panic!("Could not create canvas with error {e}"));

            let proxy = proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                proxy
                    .send_event(EmulatorEvent::CreateGraphics(GraphicsState::new(window).await))
                    .unwrap_or_else(|e| panic!("Could not send graphics event with error {e}"));
            });
        }
    }
    
    fn run_async(future: impl std::future::Future<Output = ()> + Send + 'static) {
        wasm_bindgen_futures::spawn_local(future);
    }
}
