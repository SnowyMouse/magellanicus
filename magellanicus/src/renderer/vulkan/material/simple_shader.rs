use std::sync::Arc;
use std::borrow::ToOwned;
use std::string::ToString;
use std::{println, vec};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::allocator::CommandBufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferInheritanceInfo, CommandBufferInheritanceRenderPassType, CommandBufferInheritanceRenderingInfo, CommandBufferUsage, PrimaryAutoCommandBuffer, SecondaryAutoCommandBuffer};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::format::Format;
use vulkano::image::{Image, ImageAspects, ImageSubresourceRange};
use vulkano::image::sampler::{ComponentMapping, ComponentSwizzle, Sampler, SamplerCreateInfo};
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use vulkano::padded::Padded;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use crate::error::{Error, MResult};
use crate::renderer::{AddShaderBasicShaderData, Renderer};
use crate::renderer::vulkan::{default_allocation_create_info, VulkanMaterial, VulkanMaterialShaderData, VulkanMaterialShaderStage, VulkanMaterialTextureCoordsType, VulkanPipelineData, VulkanPipelineType, VulkanRenderer};
use crate::renderer::vulkan::simple_texture::ModelData;
use crate::renderer::vulkan::vertex::{VulkanModelData, VulkanModelVertex};

pub struct VulkanSimpleShaderMaterial {
    diffuse: Arc<ImageView>,
    diffuse_sampler: Arc<Sampler>
}

impl VulkanSimpleShaderMaterial {
    pub fn new(renderer: &mut Renderer, add_shader_parameter: AddShaderBasicShaderData) -> MResult<Self> {
        let diffuse = if let Some(b) = add_shader_parameter.bitmap.as_ref() {
            renderer.bitmaps[b].bitmaps[0].vulkan.image.clone()
        }
        else if let Some(b) = renderer.default_bitmaps.as_ref().map(|b| &b.default_2d) {
            renderer.bitmaps[b].bitmaps[1].vulkan.image.clone()
        }
        else {
            return Err(Error::from_data_error_string("No bitmap referenced and no default bitmaps provided, either".to_string()))
        };

        let diffuse = ImageView::new(diffuse.clone(), ImageViewCreateInfo {
            subresource_range: ImageSubresourceRange {
                aspects: ImageAspects::COLOR,
                mip_levels: 0..diffuse.mip_levels(),
                array_layers: 0..diffuse.array_layers()
            },
            format: diffuse.format(),
            ..Default::default()
        }).unwrap();

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

    fn generate_stage_commands(&self, renderer: &Renderer, stage: usize, to: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> MResult<()> {
        assert_eq!(0, stage);
        to.bind_pipeline_graphics(renderer.renderer.pipelines[&VulkanPipelineType::SimpleTexture].get_pipeline()).unwrap();

        let pipeline = renderer.renderer.pipelines[&VulkanPipelineType::SimpleTexture].get_pipeline();

        let set = PersistentDescriptorSet::new(
            renderer.renderer.descriptor_set_allocator.as_ref(),
            pipeline.layout().set_layouts()[1].clone(),
            [
                WriteDescriptorSet::sampler(0, self.diffuse_sampler.clone()),
                WriteDescriptorSet::image_view(1, self.diffuse.clone()),
            ],
            []
        )?;

        to.bind_descriptor_sets(
            PipelineBindPoint::Graphics,
            pipeline.layout().clone(),
            1,
            set
        );

        Ok(())
    }

    fn get_texture_coords_type(&self, renderer: &Renderer, stage: usize) -> VulkanMaterialTextureCoordsType {
        assert_eq!(0, stage);
        VulkanMaterialTextureCoordsType::Model
    }
}
