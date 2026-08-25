pub struct Palette {
    pub material_to_color: [[f32; 3]; 256],
}

impl Palette {
    pub fn new() -> Self {
        let mut material_to_color = [[0.0, 0.0, 0.0]; 256];
        material_to_color[0] = [0.0, 0.0, 0.0]; // Air
        material_to_color[1] = [1.0, 1.0, 1.0]; // Solid block
        // Add more materials and their colors as needed
        Palette { material_to_color }
    }
}