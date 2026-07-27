use std::sync::Arc;

use voxel_rendering::Renderer;
use voxel_world::World;
use winit::{
    application::ApplicationHandler, event_loop::{ActiveEventLoop, EventLoop}, window::{WindowAttributes}
};

pub mod schedule;

pub struct App {
    window_attributes: WindowAttributes,
    pub rtx: Renderer,
    pub world: World,
    pub schedule: schedule::Schedule,
}

impl App {
    pub fn new(event_loop: &EventLoop<()>) -> App {
        let renderer = Renderer::new(event_loop);
        let mut schedule = schedule::Schedule::new();
        schedule.add_task_fn(|_world, rtx| rtx.draw());
        App {
            window_attributes: WindowAttributes::default().with_title("Voxel engine"),
            rtx: renderer,
            world: World::new(),
            schedule,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(self.window_attributes.clone()).unwrap());
        self.rtx.set_window(window);
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
                self.rtx.recreate_swapchain();
            }
            _ => {}
        }        
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.schedule.run(&mut self.world, &mut self.rtx);
    }
}