use std::rc::Rc;

use glam::Vec3;
use voxel_rendering::v2::{Mesh, ArrayMesh, Renderer, Vertex};

use crate::voxel::{Palette, VOXEL_SIZE, Voxel};

pub const BRICK_SIZE: usize = 16;

pub struct Brick {
    origin: [isize; 3],
    voxels: [[[Voxel; BRICK_SIZE]; BRICK_SIZE]; BRICK_SIZE],
    cached_mesh: Option<ArrayMesh>,
    palette: Rc<Palette>,
}

fn is_in_bounds(pos: [usize; 3]) -> bool {
    let [x, y, z] = pos;
    x < BRICK_SIZE && y < BRICK_SIZE && z < BRICK_SIZE
}

impl Brick {
    pub fn new(origin: [isize; 3], palette: Rc<Palette>) -> Self {
        Brick {
            origin: origin,
            voxels: [[[Voxel { material: 0 }; BRICK_SIZE]; BRICK_SIZE]; BRICK_SIZE],
            cached_mesh: None,
            palette: palette
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

    pub fn get_voxel(&self, pos: [isize; 3]) -> Option<&Voxel> {
        let local_pos = [
            (pos[0] - self.origin[0]) as usize,
            (pos[1] - self.origin[1]) as usize,
            (pos[2] - self.origin[2]) as usize,
        ];
        self.get_local_voxel(local_pos)
    }

    pub fn get_local_voxel(&self, pos: [usize; 3]) -> Option<&Voxel> {
        if is_in_bounds(pos) {
            let [x, y, z] = pos;
            Some(&self.voxels[x][y][z])
        } else {
            None
        }
    }

    /// add more complexity later
    pub fn is_voxel_solid(&self, pos: [usize; 3]) -> bool {
        if let Some(voxel) = self.get_local_voxel(pos) {
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

    pub fn get_mesh(&mut self) -> &ArrayMesh {
        if self.cached_mesh.is_none() {
            let mesh = self.generate_mesh();
            self.cached_mesh = Some(mesh);
        }
        self.cached_mesh.as_ref().unwrap()
    }

    pub fn get_mesh_ref(&self) -> Option<&ArrayMesh> {
        self.cached_mesh.as_ref()
    }

    fn generate_mesh(&self) -> ArrayMesh {
        #[derive(Clone, Copy, PartialEq)]
        struct MaskCell {
            material: u8,
            normal: [f32; 3],
            is_back_face: bool,
        }

        let sample_material = |pos: [isize; 3]| -> u8 {
            let [x, y, z] = pos;
            if x < 0 || y < 0 || z < 0 || x >= BRICK_SIZE as isize || y >= BRICK_SIZE as isize || z >= BRICK_SIZE as isize {
                return 0;
            }

            self.voxels[x as usize][y as usize][z as usize].material
        };

        let mut mesh = ArrayMesh::new();

        for norm in 0..3 {
            let tan = (norm + 1) % 3;
            let bitan = (norm + 2) % 3;

            for slice in 0..=BRICK_SIZE {
                let mut mask: [Option<MaskCell>; BRICK_SIZE * BRICK_SIZE] = [None; BRICK_SIZE * BRICK_SIZE];

                for i in 0..BRICK_SIZE {
                    for j in 0..BRICK_SIZE {
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

                        mask[i * BRICK_SIZE + j] = cell;
                    }
                }

                let mut y = 0;
                while y < BRICK_SIZE {
                    let mut x = 0;
                    while x < BRICK_SIZE {
                        let current = mask[x * BRICK_SIZE + y];
                        if current.is_none() {
                            x += 1;
                            continue;
                        }

                        let current = current.unwrap();

                        let mut width = 1;
                        while x + width < BRICK_SIZE && mask[(x + width) * BRICK_SIZE + y] == Some(current) {
                            width += 1;
                        }

                        let mut height = 1;
                        'outer: while y + height < BRICK_SIZE {
                            for w in 0..width {
                                if mask[(x + w) * BRICK_SIZE + (y + height)] != Some(current) {
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
                            let color = self.palette.material_to_color[material as usize];
                            Vertex {
                                pos: [pos_vec.x * VOXEL_SIZE, pos_vec.y * VOXEL_SIZE, pos_vec.z * VOXEL_SIZE],
                                normal: Some(normal),
                                color: Some(color),
                            }
                        };

                        let vertices = [
                            vert_from_pos(base, current.normal, current.material),
                            vert_from_pos([base[0] + du[0], base[1] + du[1], base[2] + du[2]], current.normal, current.material),
                            vert_from_pos([base[0] + du[0] + dv[0], base[1] + du[1] + dv[1], base[2] + du[2] + dv[2]], current.normal, current.material),
                            vert_from_pos([base[0] + dv[0], base[1] + dv[1], base[2] + dv[2]], current.normal, current.material),
                        ];

                        mesh.add_square_face(&vertices, current.is_back_face);

                        for dy in 0..height {
                            for dx in 0..width {
                                mask[(x + dx) * BRICK_SIZE + (y + dy)] = None;
                            }
                        }

                        x += width;
                    }
                    y += 1;
                }
            }
        }

        mesh
    }
}