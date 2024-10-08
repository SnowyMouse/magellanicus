use crate::error::MResult;
use crate::renderer::vulkan::{default_allocation_create_info, VulkanMaterial, VulkanPipelineType};
use crate::renderer::{AddShaderEnvironmentShaderData, DefaultType, Renderer, ShaderEnvironmentMapFunction, ShaderEnvironmentType};
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::sampler::Sampler;
use vulkano::image::view::ImageView;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};

pub struct VulkanShaderEnvironmentMaterial {
    map_sampler: Arc<Sampler>,
    base_map: Arc<ImageView>,
    primary_detail_map: Arc<ImageView>,
    secondary_detail_map: Arc<ImageView>,
    micro_detail_map: Arc<ImageView>,
    bump_map: Arc<ImageView>,
    alpha_tested: bool,
    detail_map_function: ShaderEnvironmentMapFunction,
    micro_detail_map_function: ShaderEnvironmentMapFunction,
    shader_environment_type: ShaderEnvironmentType,
    primary_detail_map_scale: f32,
    secondary_detail_map_scale: f32,
    bump_map_scale: f32,
    micro_detail_map_scale: f32,
    pipeline: Arc<GraphicsPipeline>,
    descriptor_set: Arc<PersistentDescriptorSet>
}

impl VulkanShaderEnvironmentMaterial {
    pub fn new(renderer: &mut Renderer, add_shader_parameter: AddShaderEnvironmentShaderData) -> MResult<Self> {
        let base_map = renderer
            .get_or_default_2d(&add_shader_parameter.base_map, 0, DefaultType::White)
            .vulkan
            .image
            .clone();

        let primary_detail_map = renderer
            .get_or_default_2d(&add_shader_parameter.primary_detail_map, 0, DefaultType::Null)
            .vulkan
            .image
            .clone();

        let secondary_detail_map = renderer
            .get_or_default_2d(&add_shader_parameter.secondary_detail_map, 0, DefaultType::Null)
            .vulkan
            .image
            .clone();

        let micro_detail_map = renderer
            .get_or_default_2d(&add_shader_parameter.micro_detail_map, 0, DefaultType::Null)
            .vulkan
            .image
            .clone();

        let bump_map = renderer
            .get_or_default_2d(&add_shader_parameter.bump_map, 0, DefaultType::Vector)
            .vulkan
            .image
            .clone();

        let pipeline = renderer
            .renderer
            .pipelines[&VulkanPipelineType::ShaderEnvironment]
            .get_pipeline();

        let uniform = super::super::pipeline::shader_environment::ShaderEnvironmentData {
            primary_detail_map_scale: add_shader_parameter.primary_detail_map_scale,
            secondary_detail_map_scale: add_shader_parameter.secondary_detail_map_scale,
            bump_map_scale: add_shader_parameter.bump_map_scale,
            micro_detail_map_scale: add_shader_parameter.micro_detail_map_scale,
            flags: add_shader_parameter.alpha_tested as u32,
            shader_environment_type: add_shader_parameter.shader_environment_type as u32,
            detail_map_function: add_shader_parameter.detail_map_function as u32,
            micro_detail_map_function: add_shader_parameter.micro_detail_map_function as u32,
        };

        let map_sampler = renderer.renderer.default_2d_sampler.clone();
        let base_map = ImageView::new_default(base_map)?;
        let primary_detail_map = ImageView::new_default(primary_detail_map)?;
        let secondary_detail_map = ImageView::new_default(secondary_detail_map)?;
        let micro_detail_map = ImageView::new_default(micro_detail_map)?;
        let bump_map = ImageView::new_default(bump_map)?;

        let uniform_buffer = Buffer::from_data(
            renderer.renderer.memory_allocator.clone(),
            BufferCreateInfo { usage: BufferUsage::UNIFORM_BUFFER, ..Default::default() },
            default_allocation_create_info(),
            uniform
        )?;

        let descriptor_set = PersistentDescriptorSet::new(
            renderer.renderer.descriptor_set_allocator.as_ref(),
            pipeline.layout().set_layouts()[3].clone(),
            [
                WriteDescriptorSet::buffer(0, uniform_buffer),
                WriteDescriptorSet::sampler(1, map_sampler.clone()),
                WriteDescriptorSet::image_view(2, base_map.clone()),
                WriteDescriptorSet::image_view(3, primary_detail_map.clone()),
                WriteDescriptorSet::image_view(4, secondary_detail_map.clone()),
                WriteDescriptorSet::image_view(5, micro_detail_map.clone()),
                WriteDescriptorSet::image_view(6, bump_map.clone()),
            ],
            []
        )?;

        let shader_data = Self {
            map_sampler,
            detail_map_function: add_shader_parameter.detail_map_function,
            micro_detail_map_function: add_shader_parameter.micro_detail_map_function,
            alpha_tested: add_shader_parameter.alpha_tested,
            primary_detail_map_scale: add_shader_parameter.primary_detail_map_scale,
            secondary_detail_map_scale: add_shader_parameter.secondary_detail_map_scale,
            bump_map_scale: add_shader_parameter.bump_map_scale,
            micro_detail_map_scale: add_shader_parameter.micro_detail_map_scale,
            shader_environment_type: add_shader_parameter.shader_environment_type,
            pipeline,
            descriptor_set,
            base_map,
            primary_detail_map,
            secondary_detail_map,
            micro_detail_map,
            bump_map
        };

        Ok(shader_data)
    }
}

impl VulkanMaterial for VulkanShaderEnvironmentMaterial {
    fn generate_commands(
        &self,
        _renderer: &Renderer,
        index_count: u32,
        repeat_shader: bool,
        to: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>
    ) -> MResult<()> {
        if !repeat_shader {
            let pipeline = self.pipeline.clone();
            to.bind_pipeline_graphics(pipeline.clone())?;
            to.bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                pipeline.layout().clone(),
                3,
                self.descriptor_set.clone()
            )?;
        }
        to.draw_indexed(index_count, 1, 0, 0, 0)?;
        Ok(())
    }
}
