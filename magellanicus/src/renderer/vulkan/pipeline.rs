use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::pipeline::GraphicsPipeline;
use crate::error::MResult;

pub mod solid_color;
pub mod simple_texture;
mod pipeline_loader;
mod color_box;
pub mod shader_environment;

pub trait VulkanPipelineData: Send + Sync + 'static {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline>;
}

pub fn load_all_pipelines(device: Arc<Device>) -> MResult<BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>>> {
    let mut pipelines: BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>> = BTreeMap::new();

    pipelines.insert(VulkanPipelineType::SolidColor, Arc::new(solid_color::SolidColorShader::new(device.clone())?));
    pipelines.insert(VulkanPipelineType::SimpleTexture, Arc::new(simple_texture::SimpleTextureShader::new(device.clone())?));
    pipelines.insert(VulkanPipelineType::ColorBox, Arc::new(color_box::ColorBox::new(device.clone())?));
    pipelines.insert(VulkanPipelineType::ShaderEnvironment, Arc::new(shader_environment::ShaderEnvironment::new(device.clone())?));

    Ok(pipelines)
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VulkanPipelineType {
    /// Writes a solid color.
    ///
    /// Useful for testing.
    SolidColor,

    /// Draws a texture.
    SimpleTexture,

    /// Draw a box of a given color.
    ColorBox,

    /// shader_environment
    ShaderEnvironment,
}
