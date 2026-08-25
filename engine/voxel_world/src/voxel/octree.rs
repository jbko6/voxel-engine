use voxel_rendering::v2::ArrayMesh;

use crate::voxel::{Brick, Voxel};

pub(crate) trait Octree {
    fn get_voxel(&self, pos: [isize; 3]) -> Option<&Voxel>;
    fn is_leaf(&self) -> bool;
    fn collect_meshes<'a>(&'a mut self, meshes: &mut Vec<&'a ArrayMesh>);
    fn ensure_meshes(&mut self);
}

pub(crate) struct OctreeNode {
    center: [isize; 3],
    children: [Option<Box<dyn Octree>>; 8],
    lod_mesh: Option<ArrayMesh>,
}

impl OctreeNode {
    pub fn new(center: [isize; 3]) -> Self {
        OctreeNode {
            center,
            children: Default::default(),
            lod_mesh: None,
        }
    }

    pub fn fill_out(&mut self, depth: usize) {
        if depth == 0 {
            return;
        }

        for i in 0..8 {
            let offset = [
                if (i & 1) == 0 { -1 } else { 1 },
                if (i & 2) == 0 { -1 } else { 1 },
                if (i & 4) == 0 { -1 } else { 1 },
            ];
            let child_center = [
                self.center[0] + offset[0] * (1 << (depth - 1)),
                self.center[1] + offset[1] * (1 << (depth - 1)),
                self.center[2] + offset[2] * (1 << (depth - 1)),
            ];
            let mut child_node = OctreeNode::new(child_center);
            child_node.fill_out(depth - 1);
            self.children[i] = Some(Box::new(child_node));
        }
    }

    fn pos_to_child(&self, pos: [isize; 3]) -> Option<&Box<dyn Octree>> {
        let mut index = 0;
        if pos[0] >= self.center[0] { index |= 1; }
        if pos[1] >= self.center[1] { index |= 2; }
        if pos[2] >= self.center[2] { index |= 4; }
        self.children[index].as_ref()
    }
}



impl Octree for OctreeNode {
    fn get_voxel(&self, pos: [isize; 3]) -> Option<&Voxel> {
        if let Some(child) = self.pos_to_child(pos) {
            child.get_voxel(pos)
        } else {
            None
        }
    }

    fn is_leaf(&self) -> bool {
        false
    }

    fn ensure_meshes(&mut self) {
        for child in self.children.iter_mut() {
            if let Some(child) = child {
                child.ensure_meshes();
            }
        }
    }

    fn collect_meshes<'a>(&'a mut self, meshes: &mut Vec<&'a ArrayMesh>) {
        self.ensure_meshes();
        for child in self.children.iter_mut() {
            if let Some(child) = child {
                child.collect_meshes(meshes);
            }
        }
    }
}

impl Octree for Brick {
    fn get_voxel(&self, pos: [isize; 3]) -> Option<&Voxel> {
        self.get_voxel(pos)
    }

    fn is_leaf(&self) -> bool {
        true
    }

    fn collect_meshes<'a>(&'a mut self, meshes: &mut Vec<&'a ArrayMesh>) {
        meshes.push(self.get_mesh());
    }

    fn ensure_meshes(&mut self) {
        self.ensure_mesh();
    }
}