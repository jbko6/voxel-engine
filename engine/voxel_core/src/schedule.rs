use voxel_rendering::Renderer;
use voxel_world::World;
use voxel_world::System;

pub trait Task {
    fn run(&mut self, world: &mut World, rtx: &mut Renderer);
}

impl Task for System {
    fn run(&mut self, world: &mut World, _rtx: &mut Renderer) {
        self.run(world);
    }
}

impl Task for Renderer {
    fn run(&mut self, _world: &mut World, rtx: &mut Renderer) {
        rtx.draw();
    }
}

pub struct Schedule {
    tasks: Vec<Box<dyn Task>>,
}

struct BaseTask {
    fun: Box<dyn FnMut(&mut World, &mut Renderer)>,
}

impl BaseTask {
    fn new(fun: impl 'static + FnMut(&mut World, &mut Renderer)) -> Self {
        Self {
            fun: Box::new(fun),
        }
    }
}

impl Task for BaseTask {
    fn run(&mut self, world: &mut World, rtx: &mut Renderer) {
        (self.fun)(world, rtx);
    }
}

impl Schedule {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn add_task<T: 'static + Task>(&mut self, task: T) {
        self.tasks.push(Box::new(task));
    }

    pub fn add_task_fn<F>(&mut self, fun: F)
    where
        F: 'static + FnMut(&mut World, &mut Renderer),
    {
        let system = BaseTask::new(fun);
        self.tasks.push(Box::new(system));
    }

    pub fn run(&mut self, world: &mut World, rtx: &mut Renderer) {
        for task in &mut self.tasks {
            task.run(world, rtx);
        }
    }
}