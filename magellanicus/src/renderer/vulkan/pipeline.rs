use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::pipeline::GraphicsPipeline;
use crate::error::MResult;
use crate::renderer::vulkan::pipeline::pipeline_loader::load_pipeline;

pub mod solid_color;
mod pipeline_loader;

pub trait VulkanPipelineData {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline>;
}

pub fn load_all_pipelines(device: Arc<Device>) -> MResult<BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>>> {
    let mut pipelines: BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>> = BTreeMap::new();
    pipelines.insert(VulkanPipelineType::SolidColor, Arc::new(solid_color::SolidColorShader::new(device.clone())?));
    Ok(pipelines)
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VulkanPipelineType {
    /// Writes a solid color.
    ///
    /// Useful for testing.
    SolidColor,
}
