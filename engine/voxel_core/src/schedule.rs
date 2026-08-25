use voxel_world::World;
use voxel_world::System;

// pub trait Task {
//     fn run(&mut self, world: &mut World, rtx: &mut Renderer);
// }

// impl Task for System {
//     fn run(&mut self, world: &mut World, _rtx: &mut Renderer) {
//         self.run(world);
//     }
// }

// impl Task for Renderer {
//     fn run(&mut self, _world: &mut World, _rtx: &mut Renderer) {
//         // rtx.draw();
//     }
// }

// pub struct Schedule {
//     tasks: Vec<Box<dyn Task>>,
// }

// impl Schedule {
//     pub fn new() -> Self {
//         Self { tasks: Vec::new() }
//     }

//     pub fn add_task(&mut self, task: Box<dyn Task>) {
//         self.tasks.push(task);
//     }

//     pub fn run(&mut self, world: &mut World, rtx: &mut Renderer) {
//         for task in &mut self.tasks {
//             task.run(world, rtx);
//         }
//     }
// }