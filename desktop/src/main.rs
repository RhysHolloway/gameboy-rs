use std::sync::Arc;

use app::pixels::winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use app::pixels::winit::window::Window;
use app::{Application, CreateWindow, GraphicsState};

fn main(){
    tracing_subscriber::fmt().with_target(false).init();
    Application::<DesktopApp>::new(true).run()
}

struct DesktopApp;

impl CreateWindow for DesktopApp {
    fn create_window(proxy: &Arc<EventLoopProxy<GraphicsState>>, event_loop: &ActiveEventLoop) {
    let window = event_loop.create_window(Window::default_attributes().with_title("Gameboy Emulator")).unwrap_or_else(|e| panic!("Could not create window with error: {e}"));
    pollster::block_on(async move {
        proxy.send_event(GraphicsState::new(window).await).unwrap_or_else(|e| panic!("Could not send graphics event with error {e}"));
    });
    }
    // fn create_window(&self) -> Result<Window, EventLoopError> {
    // }
}