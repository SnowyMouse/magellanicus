use crate::error::MResult;
use crate::renderer::vulkan::pipeline::pipeline_loader::{load_pipeline, DepthAccess, PipelineSettings};
use crate::renderer::vulkan::vertex::{VulkanModelVertex, VulkanModelVertexLightmapTextureCoords, VulkanModelVertexTextureCoords};
use crate::renderer::vulkan::{VulkanPipelineData, OFFLINE_PIPELINE_COLOR_FORMAT};
use std::sync::Arc;
use std::vec;
use vulkano::device::Device;
use vulkano::image::SampleCount;
use vulkano::pipeline::graphics::color_blend::ColorBlendAttachmentState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::GraphicsPipeline;

mod vertex {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/renderer/vulkan/pipeline/shader_environment/vertex.vert"
    }
}

// FIXME: remove the ./
mod fragment {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "./src/renderer/vulkan/pipeline/shader_environment/fragment.frag"
    }
}

pub use fragment::ShaderEnvironmentData;

pub struct ShaderEnvironment {
    pub pipeline: Arc<GraphicsPipeline>
}

impl ShaderEnvironment {
    pub fn new(device: Arc<Device>, samples: SampleCount) -> MResult<Self> {
        let pipeline = load_pipeline(device, vertex::load, fragment::load, &PipelineSettings {
            depth_access: DepthAccess::DepthWrite,
            vertex_buffer_descriptions: vec![VulkanModelVertex::per_vertex(), VulkanModelVertexTextureCoords::per_vertex(), VulkanModelVertexLightmapTextureCoords::per_vertex()],
            alpha_blending: false,
            samples,
            color_blend_attachment_state: ColorBlendAttachmentState::default()
        }, OFFLINE_PIPELINE_COLOR_FORMAT)?;

        Ok(Self { pipeline })
    }
}

impl VulkanPipelineData for ShaderEnvironment {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline> {
        self.pipeline.clone()
    }
}
