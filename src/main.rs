use std::rc::Rc;

use glam::Mat4;
use voxel_world::voxel::*;

fn main() {
    let mut app = voxel_core::App::new();

    let mut palette = Palette::new();
    palette.material_to_color[2] = [1.0, 0.0, 0.0]; // Red block

    let palette = Rc::new(palette);
    let mut chunk = Brick::new([0, 0, 0], palette.clone());
    chunk.generate_perlin([0, 0, 0]);

    let mesh = chunk.get_mesh();


    let mesh_handle = app.renderer.build_mesh(app.scenario, &mesh);
    app.renderer.create_instance(app.scenario, Mat4::IDENTITY, mesh_handle);


    app.run();
}