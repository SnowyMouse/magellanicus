use crate::error::MResult;
use crate::renderer::vulkan::pipeline::pipeline_loader::{load_pipeline, DepthAccess, PipelineSettings};
use crate::renderer::vulkan::vertex::VulkanModelVertex;
use crate::renderer::vulkan::VulkanPipelineData;
use std::sync::Arc;
use std::vec;
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::GraphicsPipeline;

mod vertex {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/renderer/vulkan/pipeline/color_box/vertex.vert"
    }
}

mod fragment {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/renderer/vulkan/pipeline/color_box/fragment.frag"
    }
}

pub struct ColorBox {
    pub pipeline: Arc<GraphicsPipeline>
}

impl ColorBox {
    pub fn new(device: Arc<Device>, color_format: Format) -> MResult<Self> {
        let pipeline = load_pipeline(device, vertex::load, fragment::load, &PipelineSettings {
            depth_access: DepthAccess::NoDepth,
            vertex_buffer_descriptions: vec![VulkanModelVertex::per_vertex()],
            alpha_blending: true
        }, color_format)?;

        Ok(Self { pipeline })
    }
}

impl VulkanPipelineData for ColorBox {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline> {
        self.pipeline.clone()
    }
}
