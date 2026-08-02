use std::sync::Arc;

use voxel_rendering::Renderer;
use voxel_world::World;
use winit::{
    application::ApplicationHandler, event, event_loop::{ActiveEventLoop, EventLoop}, raw_window_handle::{HasDisplayHandle, HasWindowHandle}, window::WindowAttributes
};

pub mod schedule;

pub struct App {
    window: Option<Arc<winit::window::Window>>,
    window_attributes: WindowAttributes,
    pub world: World,
    pub rtx: Renderer,
    pub schedule: schedule::Schedule,
}

impl App {
    pub fn new(event_loop: &EventLoop<()>) -> App {
        let renderer = Renderer::new(event_loop.display_handle().unwrap().as_raw());
        let mut schedule = schedule::Schedule::new();
        App {
            window: None,
            window_attributes: WindowAttributes::default().with_title("Voxel engine"),
            world: World::new(),
            rtx: renderer,
            schedule,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(self.window_attributes.clone()).unwrap());
        let window_size = window.inner_size();
        let window_handle = window.window_handle().unwrap();
        self.rtx.set_window(window_handle.as_raw(), window_size.width, window_size.height);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    )
    {
        match event {
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            winit::event::WindowEvent::Resized(_) => {
                let window_size = self.window.as_ref().unwrap().inner_size();
                // self.rtx.recreate_swapchain(window_size.width, window_size.height);
            }
            _ => {}
        }

        self.rtx.draw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.schedule.run(&mut self.world, &mut self.rtx);
    }
}