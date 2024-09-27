use crate::renderer::vulkan::VulkanShaderData;

pub struct Shader {
    pub vulkan: VulkanShaderData,
    pub shader_type: ShaderType
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ShaderType {
    Environment,
    Model,
    TransparentGeneric,
    TransparentChicago,
    TransparentGlass,
    TransparentMeter,
    TransparentPlasma,
    TransparentWater
}
