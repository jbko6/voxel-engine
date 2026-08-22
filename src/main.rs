use std::rc::Rc;

use glam::Mat4;
use voxel_core::*;
use voxel_rendering::ObjectData;
use voxel_rendering::Mesh;
use voxel_world::chunk::VOXEL_SIZE;
use voxel_world::{chunk::{Catalog, Chunk, Voxel}, *};
use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    let mut app = voxel_core::App::new();

    app.run();
}