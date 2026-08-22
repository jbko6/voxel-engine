Palette
- 256 possible colors, indexed by number (u8)

Voxels
- u8 material, which refers to a palette
  - the '0' material refers to air

Brick
- 16 x 16 x 16 dense array of voxels
- Caches mesh as an index in some mega-buffer

Octree
- Node with 8 children stored in Vec
- Stores cached LOD mesh as index in some mega-buffer

Chunk
- container for the octree root

for now - no infinite world, just one octree

start with pointer-based octree
later transition to flat vecs with the octree nodes storing indexes
(for use with acceleration structure in raytracing, with AABB tree properties)


Other to-do:
Build an allocator for the vertex and index "mega-buffer"s
For other meshes: have some other buffer where their data can be stored





rendering

- need more abstractions
- forward+ rendering
- more advanced buffer support
  - voxels don't need index buffers, quads don't share vertices
    - or possibly find a way for quads to share vertices? -- could be done for LOD meshes
  - some sort of fixed size buffer with advanced allocation/deallocatiin
    - this could be used for keeping the closest chunks in host visible memory so staging changes isn't needed
    - could keep this chunk persistently mapped
    - would want to keep the number of changes at a minimum, i.e. be careful about allocate and deallocate chunks
- although i don't need complex descriptor pool management, still need a better interface for dealing with that



great article on teardown: https://acko.net/blog/teardown-frame-teardown/

the ideal renderer: proper raytracing, directly using the voxel octree as AABB acceleration structure
alternative: aggressive LOD, threaded meshing, forward+ render

good article for implementing forward+ rendering: https://www.3dgep.com/forward-plus/#Forward



push buffer device address to an "objects" buffer
shaders accesses object data using gl_DrawID
object data contains direct buffer device address to vertex data, index data, normal data, etc. as well a bit field that tells if those buffer
device address are valid (if the object has that data)