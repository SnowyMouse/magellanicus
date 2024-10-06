use crate::error::MResult;
use crate::renderer::vulkan::VulkanMaterialShaderData;
use crate::renderer::{AddShaderData, AddShaderParameter, Renderer};

pub struct Shader {
    pub vulkan: VulkanMaterialShaderData,
    pub shader_type: ShaderType
}

impl Shader {
    pub fn load_from_parameters(renderer: &mut Renderer, add_shader_parameter: AddShaderParameter) -> MResult<Self> {
        let shader_type = match &add_shader_parameter.data {
            AddShaderData::BasicShader(s) => s.shader_type,
            AddShaderData::ShaderEnvironment(_) => ShaderType::Environment
        };

        let vulkan = VulkanMaterialShaderData::new_from_parameters(
            renderer,
            add_shader_parameter
        )?;

        Ok(Self { vulkan, shader_type })
    }
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
