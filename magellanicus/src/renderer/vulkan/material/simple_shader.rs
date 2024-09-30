use std::sync::Arc;
use std::borrow::ToOwned;
use std::vec;
use vulkano::command_buffer::allocator::CommandBufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferInheritanceInfo, CommandBufferInheritanceRenderPassType, CommandBufferInheritanceRenderingInfo, CommandBufferUsage, SecondaryAutoCommandBuffer};
use vulkano::format::Format;
use vulkano::image::Image;
use vulkano::image::sampler::{Sampler, SamplerCreateInfo};
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use vulkano::pipeline::GraphicsPipeline;
use crate::error::{Error, MResult};
use crate::renderer::{AddShaderBasicShaderData, Renderer};
use crate::renderer::vulkan::{VulkanMaterial, VulkanMaterialShaderData, VulkanMaterialShaderStage, VulkanMaterialTextureCoordsType, VulkanPipelineData, VulkanPipelineType, VulkanRenderer};

pub struct VulkanSimpleShaderMaterial {
    diffuse: Arc<ImageView>,
    diffuse_sampler: Arc<Sampler>
}

impl VulkanSimpleShaderMaterial {
    pub fn new(renderer: &mut Renderer, add_shader_parameter: AddShaderBasicShaderData) -> MResult<Self> {
        let diffuse = renderer
            .bitmaps
            .get(&add_shader_parameter.bitmap)
            .and_then(|b| b.bitmaps.get(0))
            .ok_or_else(|| Error::from_vulkan_impl_error("failed to get bitmap".to_owned()))?
            .vulkan
            .image
            .clone();

        let diffuse = ImageView::new(
            diffuse.clone(),
            ImageViewCreateInfo::from_image(diffuse.as_ref())
        )?;

        let diffuse_sampler = Sampler::new(
            renderer.renderer.device.clone(),
            SamplerCreateInfo::simple_repeat_linear_no_mipmap()
        )?;

        Ok(Self { diffuse, diffuse_sampler })
    }
}

impl VulkanMaterial for VulkanSimpleShaderMaterial {
    fn get_stages(&self) -> &[VulkanMaterialShaderStage] {
        &[VulkanMaterialShaderStage::Diffuse]
    }

    fn generate_stage_commands(&self, renderer: &Renderer, stage: usize) -> MResult<Arc<SecondaryAutoCommandBuffer>> {
        assert_eq!(1, stage);
        let mut builder = renderer.renderer.generate_secondary_buffer_builder()?;
        builder.bind_pipeline_graphics(renderer.renderer.pipelines[&VulkanPipelineType::SolidColor].get_pipeline())?;
        Ok(builder.build()?)
    }

    fn get_texture_coords_type(&self, renderer: &Renderer, stage: usize) -> VulkanMaterialTextureCoordsType {
        assert_eq!(1, stage);
        VulkanMaterialTextureCoordsType::Model
    }
}
