use crate::renderer::vulkan::shader::pipeline_loader::VertexPipelineInput;

pub mod solid_color;
mod pipeline_loader;

pub trait VulkanPipelineData {
    fn vertex_inputs(&self) -> &'static [VertexPipelineInput];
}

pub struct VulkanShaderData {

}
