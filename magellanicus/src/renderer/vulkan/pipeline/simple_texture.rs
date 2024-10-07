use std::sync::Arc;
use vulkano::device::Device;
use std::vec;
use vulkano::format::Format;
use vulkano::pipeline::graphics::color_blend::{AttachmentBlend, ColorBlendAttachmentState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use crate::error::MResult;
use crate::renderer::vulkan::pipeline::pipeline_loader::{load_pipeline, DepthAccess, PipelineSettings};
use crate::renderer::vulkan::vertex::{VulkanModelVertex, VulkanModelVertexLightmapTextureCoords, VulkanModelVertexTextureCoords};
use crate::renderer::vulkan::VulkanPipelineData;

mod vertex {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/renderer/vulkan/pipeline/simple_texture/vertex.vert"
    }
}

mod fragment {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/renderer/vulkan/pipeline/simple_texture/fragment.frag"
    }
}

pub struct SimpleTextureShader {
    pub pipeline: Arc<GraphicsPipeline>
}

impl SimpleTextureShader {
    pub fn new(device: Arc<Device>, color_format: Format) -> MResult<Self> {
        let pipeline = load_pipeline(device, vertex::load, fragment::load, &PipelineSettings {
            depth_access: DepthAccess::DepthWrite,
            vertex_buffer_descriptions: vec![
                VulkanModelVertex::per_vertex(),
                VulkanModelVertexTextureCoords::per_vertex(),
                VulkanModelVertexLightmapTextureCoords::per_vertex()
            ],
            alpha_blending: false,
            color_blend_attachment_state: ColorBlendAttachmentState {
                blend: Some(AttachmentBlend::additive()),
                ..ColorBlendAttachmentState::default()
            }
        }, color_format)?;

        Ok(Self { pipeline })
    }
}

impl VulkanPipelineData for SimpleTextureShader {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline> {
        self.pipeline.clone()
    }
}
