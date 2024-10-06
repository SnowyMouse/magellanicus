use crate::error::MResult;
use crate::renderer::vulkan::{default_allocation_create_info, VulkanMaterial, VulkanPipelineType};
use crate::renderer::{AddShaderEnvironmentShaderData, DefaultType, Renderer, ShaderEnvironmentMapFunction, ShaderEnvironmentType};
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::sampler::Sampler;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::rasterization::CullMode;
use vulkano::pipeline::{Pipeline, PipelineBindPoint};

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

        Ok(Self {
            map_sampler: renderer.renderer.default_2d_sampler.clone(),
            detail_map_function: add_shader_parameter.detail_map_function,
            micro_detail_map_function: add_shader_parameter.micro_detail_map_function,
            base_map: ImageView::new_default(base_map)?,
            primary_detail_map: ImageView::new_default(primary_detail_map)?,
            secondary_detail_map: ImageView::new_default(secondary_detail_map)?,
            micro_detail_map: ImageView::new_default(micro_detail_map)?,
            bump_map: ImageView::new_default(bump_map)?,
            alpha_tested: add_shader_parameter.alpha_tested,
            primary_detail_map_scale: add_shader_parameter.primary_detail_map_scale,
            secondary_detail_map_scale: add_shader_parameter.secondary_detail_map_scale,
            bump_map_scale: add_shader_parameter.bump_map_scale,
            micro_detail_map_scale: add_shader_parameter.micro_detail_map_scale,
            shader_environment_type: add_shader_parameter.shader_environment_type
        })
    }
}

impl VulkanMaterial for VulkanShaderEnvironmentMaterial {
    fn generate_commands(
        &self,
        renderer: &Renderer,
        index_count: u32,
        to: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>
    ) -> MResult<()> {
        let pipeline = renderer.renderer.pipelines[&VulkanPipelineType::ShaderEnvironment].get_pipeline();
        to.bind_pipeline_graphics(pipeline.clone())?;
        to.set_cull_mode(CullMode::Back)?;

        let uniform = super::super::pipeline::shader_environment::ShaderEnvironmentData {
            primary_detail_map_scale: self.primary_detail_map_scale,
            secondary_detail_map_scale: self.secondary_detail_map_scale,
            bump_map_scale: self.bump_map_scale,
            micro_detail_map_scale: self.micro_detail_map_scale,
            flags: self.alpha_tested as u32,
            shader_environment_type: self.shader_environment_type as u32,
            detail_map_function: self.detail_map_function as u32,
            micro_detail_map_function: self.micro_detail_map_function as u32,
        };

        let uniform_buffer = Buffer::from_data(
            renderer.renderer.memory_allocator.clone(),
            BufferCreateInfo { usage: BufferUsage::UNIFORM_BUFFER, ..Default::default() },
            default_allocation_create_info(),
            uniform
        )?;

        let set = PersistentDescriptorSet::new(
            renderer.renderer.descriptor_set_allocator.as_ref(),
            pipeline.layout().set_layouts()[2].clone(),
            [
                WriteDescriptorSet::buffer(0, uniform_buffer),
                WriteDescriptorSet::sampler(1, self.map_sampler.clone()),
                WriteDescriptorSet::image_view(2, self.base_map.clone()),
                WriteDescriptorSet::image_view(3, self.primary_detail_map.clone()),
                WriteDescriptorSet::image_view(4, self.secondary_detail_map.clone()),
                WriteDescriptorSet::image_view(5, self.micro_detail_map.clone()),
                WriteDescriptorSet::image_view(6, self.bump_map.clone()),
            ],
            []
        )?;

        to.bind_descriptor_sets(
            PipelineBindPoint::Graphics,
            pipeline.layout().clone(),
            2,
            set
        )?;

        to.draw_indexed(index_count, 1, 0, 0, 0)?;

        Ok(())
    }
}
