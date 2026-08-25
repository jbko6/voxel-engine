use crate::voxel::{BRICK::BRICK_SIZE, OctreeNode};

pub const OCTREE_DEPTH: usize = 4;
pub const CHUNK_SIZE: usize = (BRICK_SIZE as usize) * (1 << OCTREE_DEPTH);

pub struct Chunk {
    origin: [isize; 3],
    root: Option<Box<OctreeNode>>
}

impl Chunk {
    pub fn new(origin: [isize; 3]) -> Self {
        Chunk {
            origin,
            root: None
        }
    }

    pub fn fill_out(&mut self) {
        if self.root.is_none() {
            self.root = Some(Box::new(OctreeNode::new(self.origin)));
            self.root.as_mut().unwrap().fill_out(OCTREE_DEPTH);
        }
    }
}