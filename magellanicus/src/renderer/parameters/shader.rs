use alloc::string::String;
use alloc::format;
use crate::error::{Error, MResult};
pub use crate::renderer::data::ShaderType;
use crate::renderer::Renderer;

pub struct AddShaderParameter {
    pub data: AddShaderData
}

impl AddShaderParameter {
    pub(crate) fn validate(&self, renderer: &Renderer) -> MResult<()> {
        match &self.data {
            AddShaderData::BasicShader(AddShaderBasicShaderData { bitmap, .. }) => {
                if !renderer.bitmaps.contains_key(bitmap) {
                    return Err(Error::DataError { error: format!("Referenced bitmap {bitmap} is not loaded.") })
                }
            }
        }
        Ok(())
    }
}

pub enum AddShaderData {
    /// Basic pipeline that just renders a single texture. This does not map to an actual pipeline group
    /// and is to be removed once all shaders are implemented
    BasicShader(AddShaderBasicShaderData)
}

pub struct AddShaderBasicShaderData {
    pub bitmap: String,
    pub shader_type: ShaderType,
    pub alpha_tested: bool
}
