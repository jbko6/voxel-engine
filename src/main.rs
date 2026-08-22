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

    let mut catalog = Catalog::new();
    catalog.material_to_color[1] = [1.0, 1.0, 1.0]; // Solid block
    catalog.material_to_color[2] = [1.0, 0.0, 0.0]; // Red block

    let catalog = Rc::new(catalog);
    let mut chunk = Chunk::new(catalog);
    chunk.generate_perlin([0, 0, 0]);

    let mesh = chunk.get_mesh();


    let mesh_handle = app.renderer.build_mesh(app.scenario, &mesh);
    app.renderer.create_instance(app.scenario, Mat4::IDENTITY, mesh_handle);


    app.run();
}