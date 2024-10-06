use alloc::string::String;
use alloc::format;
use crate::error::{Error, MResult};
pub use crate::renderer::data::ShaderType;
use crate::renderer::{BitmapType, Renderer};
use crate::renderer::data::Bitmap;

pub struct AddShaderParameter {
    pub data: AddShaderData
}

impl AddShaderParameter {
    pub(crate) fn validate(&self, renderer: &Renderer) -> MResult<()> {
        match &self.data {
            AddShaderData::BasicShader(AddShaderBasicShaderData { bitmap, .. }) => {
                if let Some(bitmap) = bitmap {
                    if !renderer.bitmaps.contains_key(bitmap) {
                        return Err(Error::DataError { error: format!("Referenced bitmap {bitmap} is not loaded.") })
                    }
                }
            },
            AddShaderData::ShaderEnvironment(shader_data) => {
                shader_data.validate(renderer)?;
            }
        }
        Ok(())
    }
}

pub enum AddShaderData {
    /// Basic pipeline that just renders a single texture. This does not map to an actual tag group
    /// and is to be removed once all shaders are implemented
    BasicShader(AddShaderBasicShaderData),

    /// Renders a shader_environment texture.
    ShaderEnvironment(AddShaderEnvironmentShaderData)
}

pub struct AddShaderBasicShaderData {
    pub bitmap: Option<String>,
    pub shader_type: ShaderType,
    pub alpha_tested: bool
}

#[derive(Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum ShaderEnvironmentType {
    Normal,
    Blended,
    BlendedBaseSpecular
}

#[derive(Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum ShaderEnvironmentMapFunction {
    DoubleBiasedMultiply,
    Multiply,
    DoubleBiasedAdd
}

#[derive(Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum ShaderReflectionType {
    BumpedCubeMap,
    FlatCubeMap,
    BumpedRadiosity
}

#[derive(Clone)]
pub struct AddShaderEnvironmentShaderData {
    pub alpha_tested: bool,

    pub shader_environment_type: ShaderEnvironmentType,
    pub base_map: Option<String>,

    pub detail_map_function: ShaderEnvironmentMapFunction,
    pub primary_detail_map: Option<String>,
    pub primary_detail_map_scale: f32,
    pub secondary_detail_map: Option<String>,
    pub secondary_detail_map_scale: f32,

    pub micro_detail_map: Option<String>,
    pub micro_detail_map_scale: f32,
    pub micro_detail_map_function: ShaderEnvironmentMapFunction,

    pub bump_map: Option<String>,
    pub bump_map_scale: f32,

    pub reflection_cube_map: Option<String>,
    pub reflection_type: ShaderReflectionType
}
impl AddShaderEnvironmentShaderData {
    pub(crate) fn validate(&self, renderer: &Renderer) -> MResult<()> {
        check_bitmap(renderer, &self.base_map, BitmapType::Dim2D, "base map")?;
        check_bitmap(renderer, &self.primary_detail_map, BitmapType::Dim2D, "primary detail map")?;
        check_bitmap(renderer, &self.secondary_detail_map, BitmapType::Dim2D, "secondary detail map")?;
        check_bitmap(renderer, &self.micro_detail_map, BitmapType::Dim2D, "micro detail map")?;
        check_bitmap(renderer, &self.bump_map, BitmapType::Dim2D, "bump map")?;
        check_bitmap(renderer, &self.reflection_cube_map, BitmapType::Cubemap, "reflection cube map")?;
        Ok(())
    }
}

fn check_bitmap(renderer: &Renderer, reference: &Option<String>, bitmap_type: BitmapType, name: &str) -> MResult<()> {
    let Some(bitmap_path) = reference.as_ref() else {
        return Ok(())
    };

    let Some(bitmap) = renderer.bitmaps.get(bitmap_path) else {
        return Err(Error::from_data_error_string(format!("{name} {bitmap_path} is not loaded")))
    };

    expect_bitmap_or_else(bitmap, bitmap_type, name)
}

fn expect_bitmap_or_else(bitmap: &Bitmap, bitmap_type: BitmapType, name: &str) -> MResult<()> {
    let Some((bad_index, bad_bitmap)) = bitmap.bitmaps
        .iter()
        .enumerate()
        .find(|a| a.1.bitmap_type != bitmap_type) else {
        return Ok(())
    };

    Err(Error::from_data_error_string(format!("Bitmap #{bad_index} of {name} is {:?}, expected {bitmap_type:?}", bad_bitmap.bitmap_type)))
}
