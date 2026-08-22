pub mod world;
pub mod chunk;
pub mod entity_table;
pub mod component_column;
pub mod system;
pub mod region;
pub mod perlin_gen;

pub use world::*;
pub use entity_table::*;
pub use component_column::*;
pub use system::*;
pub use region::*;
pub use perlin_gen::*;

pub fn test() {
    println!("Hello from voxel_world!");
}