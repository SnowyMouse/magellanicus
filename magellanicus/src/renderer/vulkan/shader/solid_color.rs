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
use crate::renderer::vulkan::shader::pipeline_loader::{load_pipeline, PipelineSettings, VertexPipelineInput};
use crate::renderer::vulkan::vertex::{VulkanModelVertexNormal, VulkanModelVertexPosition, VulkanModelVertexTextureCoords};

mod vertex {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/renderer/vulkan/shader/solid_color/vertex.vert"
    }
}

mod fragment {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/renderer/vulkan/shader/solid_color/fragment.frag"
    }
}

pub struct SolidColorShader {
    pub pipeline: Arc<GraphicsPipeline>
}

impl SolidColorShader {
    pub fn new(device: Arc<Device>) -> MResult<Self> {
        let pipeline = load_pipeline(device, vertex::load, fragment::load, &PipelineSettings {
            vertex_inputs: &[
                VertexPipelineInput::Position,
            ],
            writes_depth: true
        })?;

        Ok(Self { pipeline })
    }
}
