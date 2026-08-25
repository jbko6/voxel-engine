pub mod brick;
pub mod perlin_gen;
pub mod BRICK;
pub mod palette;
pub mod octree;
pub mod chunk;

pub use brick::*;
pub use perlin_gen::*;
pub use palette::*;
pub use octree::*;
pub use chunk::*;

pub const VOXEL_SIZE: f32 = 0.2;

#[derive(Clone, Copy, Debug)]
pub struct Voxel {
    pub material: u8,
}