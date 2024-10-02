use alloc::string::String;
use alloc::sync::Arc;
use vulkano::device::Device;
use alloc::string::ToString;
use std::vec;
use vulkano::format::Format;
use vulkano::pipeline::{DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::{CullMode, FrontFace, RasterizationState};
use vulkano::pipeline::graphics::subpass::PipelineRenderingCreateInfo;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use crate::error::MResult;
use crate::renderer::vulkan::pipeline::pipeline_loader::{load_pipeline, DepthAccess, PipelineSettings};
use crate::renderer::vulkan::vertex::{VulkanModelVertex, VulkanModelVertexTextureCoords};
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

pub use vertex::ModelData;

pub struct SimpleTextureShader {
    pub pipeline: Arc<GraphicsPipeline>
}

impl SimpleTextureShader {
    pub fn new(device: Arc<Device>, color_format: Format) -> MResult<Self> {
        use vulkano::pipeline::Pipeline;

        let pipeline = load_pipeline(device, vertex::load, fragment::load, &PipelineSettings {
            depth_access: DepthAccess::DepthWrite,
            vertex_buffer_descriptions: vec![VulkanModelVertex::per_vertex(), VulkanModelVertexTextureCoords::per_vertex()],
            alpha_blending: false
        }, color_format)?;

        Ok(Self { pipeline })
    }
}

impl VulkanPipelineData for SimpleTextureShader {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline> {
        self.pipeline.clone()
    }
}
