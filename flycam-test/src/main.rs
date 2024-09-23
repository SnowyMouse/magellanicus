use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalSize, Size};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};
use magellanicus::renderer::{Renderer, RendererParameters, Resolution};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut handler = FlycamTestHandler {
        renderer: None,
        window: None
    };
    event_loop.run_app(&mut handler).unwrap()
}

pub struct FlycamTestHandler {
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>
}

impl ApplicationHandler for FlycamTestHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes();
        attributes.inner_size = Some(Size::Physical(PhysicalSize::new(640, 480)));

        let window = Arc::new(event_loop.create_window(attributes).unwrap());
        self.window = Some(window.clone());

        let renderer = Renderer::new(RendererParameters {
            resolution: Resolution { width: window.inner_size().width, height: window.inner_size().height },
            number_of_viewports: 1
        }, window.clone());

        match renderer {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                eprintln!("Failed to initialize renderer: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            _ => ()
        }
    }
}
