use crate::World;

pub struct System {
    fun: Box<dyn FnMut(&mut World)>,
}

impl System {
    pub fn new<F>(fun: F) -> Self
    where
        F: 'static + FnMut(&mut World),
    {
        Self {
            fun: Box::new(fun),
        }
    }

    pub fn run(&mut self, world: &mut World) {
        (self.fun)(world);
    }
}