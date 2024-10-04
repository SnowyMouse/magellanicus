use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use alloc::string::ToString;
use vulkano::padded::Padded;

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[derive(BufferContents, Vertex)]
pub struct VulkanModelVertex {
    #[format(R32G32B32_SFLOAT)]
    pub position: [f32; 3],

    #[format(R32G32B32_SFLOAT)]
    pub normal: [f32; 3],

    #[format(R32G32B32_SFLOAT)]
    pub binormal: [f32; 3],

    #[format(R32G32B32_SFLOAT)]
    pub tangent: [f32; 3]
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[derive(BufferContents, Vertex)]
pub struct VulkanModelVertexTextureCoords {
    #[format(R32G32_SFLOAT)]
    pub texture_coords: [f32; 2],
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[derive(BufferContents, Vertex)]
pub struct VulkanModelVertexLightmapTextureCoords {
    #[format(R32G32_SFLOAT)]
    pub lightmap_texture_coords: [f32; 2],
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[derive(BufferContents)]
pub struct VulkanModelData {
    pub world: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub offset: Padded<[f32; 3], 4>,
    pub rotation: [Padded<[f32; 3], 4>; 3],
}
