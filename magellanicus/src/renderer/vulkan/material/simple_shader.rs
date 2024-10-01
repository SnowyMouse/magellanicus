use std::sync::Arc;
use std::borrow::ToOwned;
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
        let diffuse = renderer
            .bitmaps
            .get(&add_shader_parameter.bitmap)
            .and_then(|b| b.bitmaps.get(0))
            .ok_or_else(|| Error::from_vulkan_impl_error("failed to get bitmap".to_owned()))?
            .vulkan
            .image
            .clone();

        let diffuse = ImageView::new(diffuse.clone(), ImageViewCreateInfo {
            component_mapping: ComponentMapping {
                a: ComponentSwizzle::One,
                ..Default::default()
            },
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

    fn generate_stage_commands(&self, renderer: &Renderer, stage: usize, vulkan_model_data: &VulkanModelData, to: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> MResult<()> {
        assert_eq!(0, stage);
        let pipeline = renderer.renderer.pipelines[&VulkanPipelineType::SimpleTexture].get_pipeline();

        let uniform_buffer = Buffer::from_data(
            renderer.renderer.memory_allocator.clone(),
            BufferCreateInfo { usage: BufferUsage::UNIFORM_BUFFER, ..Default::default() },
            default_allocation_create_info(),
            ModelData {
                world: vulkan_model_data.world,
                proj: vulkan_model_data.proj,
                view: vulkan_model_data.view,
                offset: Padded::from(vulkan_model_data.offset).into(),
                rotation: [Padded::from(vulkan_model_data.rotation[0]).into(), Padded::from(vulkan_model_data.rotation[1]).into(), Padded::from(vulkan_model_data.rotation[2]).into()],
            }
        )?;

        let set = PersistentDescriptorSet::new(
            renderer.renderer.descriptor_set_allocator.as_ref(),
            pipeline.layout().set_layouts()[0].clone(),
            [
                WriteDescriptorSet::buffer(0, uniform_buffer),
                WriteDescriptorSet::sampler(1, self.diffuse_sampler.clone()),
                WriteDescriptorSet::image_view(2, self.diffuse.clone()),
            ],
            []
        )?;

        to.bind_descriptor_sets(
            PipelineBindPoint::Graphics,
            pipeline.layout().clone(),
            0,
            set
        );

        to.bind_pipeline_graphics(pipeline.clone())?;
        Ok(())
    }

    fn get_texture_coords_type(&self, renderer: &Renderer, stage: usize) -> VulkanMaterialTextureCoordsType {
        assert_eq!(0, stage);
        VulkanMaterialTextureCoordsType::Model
    }
}
