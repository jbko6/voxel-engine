use std::rc::Rc;

use glam::Vec3;
use voxel_rendering::{Mesh, Vertex};

pub const VOXEL_SIZE: f32 = 0.2;

#[derive(Clone, Copy, Debug)]
pub struct Voxel {
    pub material: u8,
}

pub struct Catalog {
    pub material_to_color: [[f32; 3]; 256],
}

impl Catalog {
    pub fn new() -> Self {
        let mut material_to_color = [[0.0, 0.0, 0.0]; 256];
        material_to_color[0] = [0.0, 0.0, 0.0]; // Air
        material_to_color[1] = [1.0, 1.0, 1.0]; // Solid block
        // Add more materials and their colors as needed
        Catalog { material_to_color }
    }
}

pub const CHUNK_SIZE: usize = 16;

pub struct Chunk {
    voxels: [[[Voxel; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
    cached_mesh: Option<Mesh>,
    catalog: Rc<Catalog>,
}

fn is_in_bounds(pos: [usize; 3]) -> bool {
    let [x, y, z] = pos;
    x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE
}

impl Chunk {
    pub fn new(catalog: Rc<Catalog>) -> Self {
        Chunk {
            voxels: [[[Voxel { material: 0 }; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
            cached_mesh: None,
            catalog: catalog
        }
    }    

    pub fn set_voxel(&mut self, pos: [usize; 3], voxel: Voxel) -> Result<(), ()> {
        if is_in_bounds(pos) {
            let [x, y, z] = pos;
            self.voxels[x][y][z] = voxel;
            self.cached_mesh = None;
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn get_voxel(&self, pos: [usize; 3]) -> Option<&Voxel> {
        if is_in_bounds(pos) {
            let [x, y, z] = pos;
            Some(&self.voxels[x][y][z])
        } else {
            None
        }
    }

    /// add more complexity later
    pub fn is_voxel_solid(&self, pos: [usize; 3]) -> bool {
        if let Some(voxel) = self.get_voxel(pos) {
            voxel.material != 0
        } else {
            false
        }
    }

    pub fn ensure_mesh(&mut self) {
        if self.cached_mesh.is_none() {
            let mesh = self.generate_mesh();
            self.cached_mesh = Some(mesh);
        }
    }

    pub fn get_mesh(&mut self) -> &Mesh {
        if self.cached_mesh.is_none() {
            let mesh = self.generate_mesh();
            self.cached_mesh = Some(mesh);
        }
        self.cached_mesh.as_ref().unwrap()
    }

    pub fn get_mesh_ref(&self) -> Option<&Mesh> {
        self.cached_mesh.as_ref()
    }

    fn generate_mesh(&self) -> Mesh {
        #[derive(Clone, Copy, PartialEq)]
        struct MaskCell {
            material: u8,
            normal: [f32; 3],
            is_back_face: bool,
        }

        let sample_material = |pos: [isize; 3]| -> u8 {
            let [x, y, z] = pos;
            if x < 0 || y < 0 || z < 0 || x >= CHUNK_SIZE as isize || y >= CHUNK_SIZE as isize || z >= CHUNK_SIZE as isize {
                return 0;
            }

            self.voxels[x as usize][y as usize][z as usize].material
        };

        let mut mesh = Mesh::new(Vec::new(), Vec::new());

        for norm in 0..3 {
            let tan = (norm + 1) % 3;
            let bitan = (norm + 2) % 3;

            for slice in 0..=CHUNK_SIZE {
                let mut mask: [Option<MaskCell>; CHUNK_SIZE * CHUNK_SIZE] = [None; CHUNK_SIZE * CHUNK_SIZE];

                for i in 0..CHUNK_SIZE {
                    for j in 0..CHUNK_SIZE {
                        let mut pos = [0isize; 3];
                        pos[norm] = slice as isize;
                        pos[tan] = i as isize;
                        pos[bitan] = j as isize;

                        let mut neg = pos;
                        neg[norm] -= 1;

                        let pos_mat = sample_material(pos);
                        let neg_mat = sample_material(neg);
                        
                        let mut pos_normal = [0f32; 3];
                        pos_normal[norm] = 1.0;
                        let mut neg_normal = [0f32; 3];
                        neg_normal[norm] = -1.0;

                        let cell = if neg_mat != 0 && pos_mat == 0 {
                            Some(MaskCell {
                                material: neg_mat,
                                normal: neg_normal,
                                is_back_face: true,
                            })
                        } else if pos_mat != 0 && neg_mat == 0 {
                            Some(MaskCell {
                                material: pos_mat,
                                normal: pos_normal,
                                is_back_face: false,
                            })
                        } else {
                            None
                        };

                        mask[i * CHUNK_SIZE + j] = cell;
                    }
                }

                let mut y = 0;
                while y < CHUNK_SIZE {
                    let mut x = 0;
                    while x < CHUNK_SIZE {
                        let current = mask[x * CHUNK_SIZE + y];
                        if current.is_none() {
                            x += 1;
                            continue;
                        }

                        let current = current.unwrap();

                        let mut width = 1;
                        while x + width < CHUNK_SIZE && mask[(x + width) * CHUNK_SIZE + y] == Some(current) {
                            width += 1;
                        }

                        let mut height = 1;
                        'outer: while y + height < CHUNK_SIZE {
                            for w in 0..width {
                                if mask[(x + w) * CHUNK_SIZE + (y + height)] != Some(current) {
                                    break 'outer;
                                }
                            }
                            height += 1;
                        }

                        let mut base = [0i32; 3];
                        base[norm] = slice as i32;
                        base[tan] = x as i32;
                        base[bitan] = y as i32;

                        let mut du = [0i32; 3];
                        du[tan] = width as i32;

                        let mut dv = [0i32; 3];
                        dv[bitan] = height as i32;

                        let vert_from_pos = |pos: [i32; 3], normal: [f32; 3], material: u8| -> Vertex {
                            let pos_vec = Vec3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32);
                            let color = self.catalog.material_to_color[material as usize];
                            Vertex {
                                pos: [pos_vec.x * VOXEL_SIZE, pos_vec.y * VOXEL_SIZE, pos_vec.z * VOXEL_SIZE],
                                normal,
                                color,
                            }
                        };

                        let vertices = [
                            vert_from_pos(base, current.normal, current.material),
                            vert_from_pos([base[0] + du[0], base[1] + du[1], base[2] + du[2]], current.normal, current.material),
                            vert_from_pos([base[0] + du[0] + dv[0], base[1] + du[1] + dv[1], base[2] + du[2] + dv[2]], current.normal, current.material),
                            vert_from_pos([base[0] + dv[0], base[1] + dv[1], base[2] + dv[2]], current.normal, current.material),
                        ];

                        mesh.add_square_face(vertices, current.is_back_face);

                        for dy in 0..height {
                            for dx in 0..width {
                                mask[(x + dx) * CHUNK_SIZE + (y + dy)] = None;
                            }
                        }

                        x += width;
                    }
                    y += 1;
                }
            }
        }

        println!(
            "Generated mesh with {} vertices and {} indices",
            mesh.vertices().len(),
            mesh.indices().len()
        );

        mesh
    }
}