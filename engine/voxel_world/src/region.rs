use std::rc::Rc;

use voxel_rendering::{Mesh, v2::MeshBuilder};

use crate::chunk::{Catalog, Chunk, Voxel};

pub const REGION_SIZE: usize = 1;

pub struct Region {
    chunks: Vec<Chunk>,
    origin: [usize; 3],
}

impl Region {
    pub fn new(voxel_catalog: Rc<Catalog>) -> Self {
        let chunks = (0..REGION_SIZE * REGION_SIZE * REGION_SIZE)
            .map(|_| Chunk::new(voxel_catalog.clone()))
            .collect();
        Region {
            chunks,
            origin: [0, 0, 0],
        }
    }

    #[inline]
    pub fn chunk(&self, pos: [usize; 3]) -> Option<&Chunk> {
        let [x, y, z] = [pos[0] - self.origin[0], pos[1] - self.origin[1], pos[2] - self.origin[2]];
        if x < REGION_SIZE && y < REGION_SIZE && z < REGION_SIZE {
            let index = x * REGION_SIZE * REGION_SIZE + y * REGION_SIZE + z;
            return self.chunks.get(index);
        }
        None
    }

    #[inline]
    pub fn chunk_mut(&mut self, pos: [usize; 3]) -> Option<&mut Chunk> {
        let [x, y, z] = [pos[0] - self.origin[0], pos[1] - self.origin[1], pos[2] - self.origin[2]];
        if x < REGION_SIZE && y < REGION_SIZE && z < REGION_SIZE {
            let index = x * REGION_SIZE * REGION_SIZE + y * REGION_SIZE + z;
            return self.chunks.get_mut(index);
        }
        None
    }

    pub fn get_voxel(&self, pos: [usize; 3]) -> Option<&Voxel> {
        if pos[0] < self.origin[0] || pos[1] < self.origin[1] || pos[2] < self.origin[2] {
            return None;
        }
        let [x, y, z] = [pos[0] - self.origin[0], pos[1] - self.origin[1], pos[2] - self.origin[2]];
        if x < REGION_SIZE && y < REGION_SIZE && z < REGION_SIZE {
            if let Some(chunk) = self.chunk([x, y, z]) {
                return chunk.get_voxel(pos);
            }
        }
        None
    }

    pub fn get_meshes_around(&mut self, pos: [usize; 3]) -> Vec<&MeshBuilder> {
        let [x, y, z] = [pos[0] - self.origin[0], pos[1] - self.origin[1], pos[2] - self.origin[2]];

        let mut coords = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    let nz = z as isize + dz;
                    if nx >= 0 && ny >= 0 && nz >= 0
                        && nx < REGION_SIZE as isize
                        && ny < REGION_SIZE as isize
                        && nz < REGION_SIZE as isize
                    {
                        coords.push((nx as usize, ny as usize, nz as usize));
                    }
                }
            }
        }

        for &(nx, ny, nz) in &coords {
            if let Some(chunk) = self.chunk_mut([nx, ny, nz]) {
                chunk.ensure_mesh();
            }
        }

        let mut meshes = Vec::new();
        for &(nx, ny, nz) in &coords {
            if let Some(chunk) = self.chunk([nx, ny, nz]) {
                if let Some(mesh) = chunk.get_mesh_ref() {
                    meshes.push(mesh);
                }
            }
        }

        meshes
    }
}