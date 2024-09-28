use crate::error::MResult;
use crate::renderer::{AddShaderBasicShaderData, AddShaderData, AddShaderParameter, Renderer};
use crate::renderer::vulkan::{VulkanRenderer, VulkanShaderData};

pub struct Shader {
    pub vulkan: VulkanShaderData,
    pub shader_type: ShaderType
}

impl Shader {
    pub fn load_from_parameters(renderer: &mut Renderer, add_shader_parameter: AddShaderParameter) -> MResult<Self> {
        match add_shader_parameter.data {
            AddShaderData::BasicShader(n) => Self::load_basic_shader(renderer, n)
        }
    }

    fn load_basic_shader(renderer: &mut Renderer, data: AddShaderBasicShaderData) -> MResult<Self> {
        todo!()
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
