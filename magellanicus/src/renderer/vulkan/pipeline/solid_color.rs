use crate::error::MResult;
use crate::renderer::vulkan::pipeline::pipeline_loader::{load_pipeline, DepthAccess, PipelineSettings};
use crate::renderer::vulkan::vertex::VulkanModelVertex;
use crate::renderer::vulkan::{VulkanPipelineData, OFFLINE_PIPELINE_COLOR_FORMAT};
use alloc::sync::Arc;
use std::vec;
use vulkano::device::Device;
use vulkano::image::SampleCount;
use vulkano::pipeline::graphics::color_blend::ColorBlendAttachmentState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::GraphicsPipeline;

mod vertex {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/renderer/vulkan/pipeline/solid_color/vertex.vert"
    }
}

mod fragment {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/renderer/vulkan/pipeline/solid_color/fragment.frag"
    }
}

pub struct SolidColorShader {
    pub pipeline: Arc<GraphicsPipeline>
}

impl SolidColorShader {
    pub fn new(device: Arc<Device>, samples: SampleCount) -> MResult<Self> {
        let pipeline = load_pipeline(device, vertex::load, fragment::load, &PipelineSettings {
            depth_access: DepthAccess::DepthWrite,
            vertex_buffer_descriptions: vec![VulkanModelVertex::per_vertex()],
            alpha_blending: false,
            color_blend_attachment_state: ColorBlendAttachmentState::default(),
            samples
        }, OFFLINE_PIPELINE_COLOR_FORMAT)?;

        Ok(Self { pipeline })
    }
}

impl VulkanPipelineData for SolidColorShader {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline> {
        self.pipeline.clone()
    }
}
