use crate::Renderer;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

#[derive(Debug)]
pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Mesh { vertices, indices }
    }

    pub fn vertices(&self) -> &Vec<Vertex> {
        &self.vertices
    }

    pub fn vertices_mut(&mut self) -> &mut Vec<Vertex> {
        &mut self.vertices
    }

    pub fn indices(&self) -> &Vec<u32> {
        &self.indices
    }

    pub fn indices_mut(&mut self) -> &mut Vec<u32> {
        &mut self.indices
    }

    pub fn add_square_face(&mut self, vertices: [Vertex; 4], is_back_face: bool) {
        let start_index = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&vertices);

        if is_back_face {
            self.indices.extend_from_slice(&[
                start_index,
                start_index + 1,
                start_index + 2,
                start_index,
                start_index + 2,
                start_index + 3,
            ]);
        } else {
            self.indices.extend_from_slice(&[
                start_index,
                start_index + 2,
                start_index + 1,
                start_index,
                start_index + 3,
                start_index + 2,
            ]);
        }
    }
}

impl Renderer {
    pub fn load_obj(&self, path: &str) -> Option<Mesh> {

        // return none if the file doesn't exist
        let models =
            tobj::load_obj(path, &tobj::LoadOptions::default());

        if models.is_err() {
            println!("Failed to load OBJ file: {}", path);
            return None;
        }

        let (models, _) = models.unwrap();

        println!("Loaded {} models from {}", models.len(), path);

        if models.len() == 0 {
            print!("No models found in OBJ file: {}", path);
            return None;
        }

        let model = &models[0];

        let mesh = &model.mesh;

        let indices = mesh.indices.clone();

        let normals_present = mesh.normals.len() > 0;
        let colors_present = mesh.vertex_color.len() > 0;

        let vertices = mesh
            .positions
            .chunks(3)
            .enumerate()
            .map(|(i, pos)| {
                let normal = if normals_present {
                    let normals = &mesh.normals[i * 3..i * 3 + 3];
                    [normals[0], normals[1], normals[2]]
                } else {
                    [0.0, 0.0, 0.0]
                };
                let color = if colors_present {
                    let colors = &mesh.vertex_color[i * 3..i * 3 + 3];
                    [colors[0], colors[1], colors[2]]
                } else {
                    [1.0, 1.0, 1.0]
                };
                Vertex {
                    pos: [pos[0], pos[1], pos[2]],
                    normal,
                    color,
                }
            })
            .collect::<Vec<Vertex>>();

        println!(
            "Loaded {} vertices and {} indices from {}",
            vertices.len(),
            indices.len(),
            path
        );

        Some(Mesh::new(vertices, indices))
    }
}