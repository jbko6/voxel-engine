use voxel_core::*;
// use voxel_rendering::Mesh;
use voxel_world::*;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(&event_loop);

    let world = &mut app.world;
    let schedule = &mut app.schedule;
    let rtx = &mut app.rtx;

    let teapot = world.spawn();
    let teapot_mesh = rtx.load_obj("assets/teapot.obj");
    world.emplace(teapot, "mesh", teapot_mesh);

    // schedule.add_task_fn(|world, rtx| {
    //     let meshes = world.iter::<Mesh>("mesh").cloned().collect::<Vec<Mesh>>();
    //     rtx.set_meshes(meshes);
    // });

    let _ = event_loop.run_app(&mut app);
}