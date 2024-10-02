use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::pipeline::{GraphicsPipeline, Pipeline};
use crate::error::MResult;
use crate::renderer::vulkan::pipeline::pipeline_loader::load_pipeline;

pub mod solid_color;
pub mod simple_texture;
mod pipeline_loader;
mod color_box;

pub trait VulkanPipelineData: Send + Sync + 'static {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline>;
}

pub fn load_all_pipelines(device: Arc<Device>, color_format: Format) -> MResult<BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>>> {
    let mut pipelines: BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>> = BTreeMap::new();

    pipelines.insert(VulkanPipelineType::SolidColor, Arc::new(solid_color::SolidColorShader::new(device.clone(), color_format)?));
    pipelines.insert(VulkanPipelineType::SimpleTexture, Arc::new(simple_texture::SimpleTextureShader::new(device.clone(), color_format)?));
    pipelines.insert(VulkanPipelineType::ColorBox, Arc::new(color_box::ColorBox::new(device.clone(), color_format)?));

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
}
