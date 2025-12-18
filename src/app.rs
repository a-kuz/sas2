use std::sync::Arc;
use winit::{
    event_loop::EventLoop,
    window::Window,
};

use crate::render::WgpuRenderer;
use crate::input::InputState;
use crate::console::Console;
use crate::audio::events::AudioEventQueue;

pub struct App {
    pub window: Arc<Window>,
    pub renderer: WgpuRenderer,
    pub input: InputState,
    pub console: Console,
    pub audio_events: AudioEventQueue,
}

impl App {
    pub async fn new(event_loop: &EventLoop<()>) -> Result<Self, String> {
        let window_attributes = Window::default_attributes()
            .with_title("SAS2")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        
        let window = Arc::new(
            event_loop.create_window(window_attributes)
                .map_err(|e| format!("Failed to create window: {:?}", e))?
        );

        let renderer = WgpuRenderer::new(window.clone()).await?;
        let input = InputState::new();
        let console = Console::new();
        let audio_events = AudioEventQueue::new();

        Ok(Self {
            window,
            renderer,
            input,
            console,
            audio_events,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.resize(new_size);
    }

    pub fn handle_input(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::WindowEvent;
        use winit::keyboard::PhysicalKey;

        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    if event.state.is_pressed() {
                        self.input.handle_key_press(keycode);
                    } else {
                        self.input.handle_key_release(keycode);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Left {
                    if state.is_pressed() {
                        self.input.handle_mouse_button_press();
                    } else {
                        self.input.handle_mouse_button_release();
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.update_mouse_position(position.x as f32, position.y as f32);
            }
            _ => {}
        }
    }
}

