use std::{rc::Rc, sync::OnceLock, thread};

use noise::{NoiseFn, Perlin};

use crate::{REGION_SIZE, Region, chunk::{CHUNK_SIZE, Catalog, Chunk, Voxel}};

static PERLIN: OnceLock<Perlin> = OnceLock::new();

pub trait PerlinGenerator {
    fn generate_perlin(&mut self, origin: [isize; 3]);
}

impl PerlinGenerator for Chunk {
    fn generate_perlin(&mut self, origin: [isize; 3]) {
        // generate perlin noise for the chunk based on the origin
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let pos = [origin[0].saturating_add(x as isize), origin[1].saturating_add(y as isize), origin[2].saturating_add(z as isize)];
                    
                    let perlin = PERLIN.get_or_init(|| Perlin::new(1));
                    let noise_value = perlin.get([pos[0] as f64 / 10.0, pos[1] as f64 / 10.0, pos[2] as f64 / 10.0]);
                    let material = if noise_value > 0.0 { if noise_value > 0.1 { 1 } else { 2 } } else { 0 };
                    self.set_voxel([x, y, z], Voxel { material }).unwrap();
                }
            }
        }
    }
}

impl PerlinGenerator for Region {
    fn generate_perlin(&mut self, origin: [isize; 3]) {
        for x in 0..REGION_SIZE {
            for y in 0..REGION_SIZE {
                for z in 0..REGION_SIZE {
                    self.chunk_mut([x, y, z]).unwrap()
                        .generate_perlin([origin[0].saturating_add(x as isize) * CHUNK_SIZE as isize, origin[1].saturating_add(y as isize) * CHUNK_SIZE as isize, origin[2].saturating_add(z as isize) * CHUNK_SIZE as isize]);
                }
            }
        }
    }
}