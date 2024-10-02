mod simple_shader;

use std::sync::Arc;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, SecondaryAutoCommandBuffer};
use crate::error::MResult;
use crate::renderer::{AddShaderData, AddShaderParameter, Renderer};
use crate::renderer::vulkan::{VulkanPipelineData, VulkanRenderer};
use crate::renderer::vulkan::material::simple_shader::VulkanSimpleShaderMaterial;
use crate::renderer::vulkan::vertex::VulkanModelData;

pub struct VulkanMaterialShaderData {
    pub pipeline_data: Arc<dyn VulkanMaterial>
}

impl VulkanMaterialShaderData {
    pub fn new_from_parameters(renderer: &mut Renderer, shader: AddShaderParameter) -> MResult<Self> {
        match shader.data {
            AddShaderData::BasicShader(shader) => {
                let shader = Arc::new(VulkanSimpleShaderMaterial::new(renderer, shader)?);
                Ok(Self { pipeline_data: shader })
            }
        }
    }
}

pub enum VulkanMaterialShaderStage {
    Diffuse,
    Reflection,
    Detail,
    Lightmap,
}

#[derive(Copy, Clone, PartialEq)]
pub enum VulkanMaterialTextureCoordsType {
    Model,
    Lightmaps
}

pub trait VulkanMaterial: Send + Sync + 'static {
    /// Get all stages for the shader.
    fn get_stages(&self) -> &[VulkanMaterialShaderStage];

    /// Execute the stage.
    ///
    /// # Panics
    ///
    /// Panics if `stage >= self.get_stages().len()`
    fn generate_stage_commands(&self, renderer: &Renderer, stage: usize, to: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> MResult<()>;

    /// Get the texture coords type that needs to be bound.
    ///
    /// # Panics
    ///
    /// Panics if `stage >= self.get_stages().len()`
    fn get_texture_coords_type(&self, renderer: &Renderer, stage: usize) -> VulkanMaterialTextureCoordsType;

    /// Return `true` if the material is transparent.
    ///
    /// If so, it needs to be rendered back-to-front.
    ///
    /// Default: `false`
    fn is_transparent(&self) -> bool {
        false
    }

    /// Return `true` if the material is two-sided.
    ///
    /// If so, depth sorting needs to be disabled when rendering.
    ///
    /// Default: `false`
    fn is_two_sided(&self) -> bool {
        false
    }
}
