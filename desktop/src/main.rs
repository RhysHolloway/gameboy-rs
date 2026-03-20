use std::sync::Arc;

use app::pixels::winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use app::pixels::winit::window::Window;
use app::{Application, EmulatorEvent, EmulatorPlatform, GraphicsState};

fn main(){
    tracing_subscriber::fmt().with_target(false).init();
    let rom = std::env::args().nth(1).map(|s| std::fs::read(&s).unwrap_or_else(|e| panic!("Could not read ROM file {s} with error: {e}")));
    Application::<Desktop>::new(true, rom).run()
}

struct Desktop;

impl EmulatorPlatform for Desktop {
    fn create_window(proxy: &Arc<EventLoopProxy<EmulatorEvent>>, event_loop: &ActiveEventLoop) {
    let window = event_loop.create_window(Window::default_attributes().with_title("Gameboy Emulator")).unwrap_or_else(|e| panic!("Could not create window with error: {e}"));
    pollster::block_on(async move {
        proxy.send_event(EmulatorEvent::CreateGraphics(GraphicsState::new(window).await)).unwrap_or_else(|e| panic!("Could not send graphics event with error {e}"));
    });
    }
    
    fn run_async(future: impl std::future::Future<Output = ()> + Send + 'static) {
        std::thread::spawn(move || {
            pollster::block_on(future);
        });
    }
    // fn create_window(&self) -> Result<Window, EventLoopError> {
    // }
}