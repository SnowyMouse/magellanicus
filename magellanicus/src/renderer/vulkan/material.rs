mod simple_shader;

use crate::error::MResult;
use crate::renderer::vulkan::material::simple_shader::VulkanSimpleShaderMaterial;
use crate::renderer::{AddShaderData, AddShaderParameter, Renderer};
use std::sync::Arc;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

/// Material shader data
///
/// Vertex inputs are bound like this:
///
/// - layout 0, location 0 is vertex data, defined as [`VulkanModelVertex`](crate::renderer::vulkan::vertex::VulkanModelVertex)
/// - layout 0, location 1 is texture coordinates, defined as [`VulkanModelVertexTextureCoords`](crate::renderer::vulkan::vertex::VulkanModelVertexTextureCoords)
/// - layout 0, location 2 is lightmap texture coordinates, defined as [`VulkanModelVertexTextureCoords`](crate::renderer::vulkan::vertex::VulkanModelVertexTextureCoords)
///
/// Descriptor sets are bound like this:
///
/// - set 0, binding 0 is ModelData, defined as [`VulkanModelData`](crate::renderer::vulkan::vertex::VulkanModelData)
/// - set 1, binding 0 is a sampler for lightmaps
/// - set 1, binding 1 is an image view for lightmaps
///
/// Nothing will be bound on layout 1+. Anything on set 2+ is shader-specific.

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
    /// Generate rendering commands.
    ///
    /// All vertex buffers (vertices, texture coords, lightmap texture coords) must be bound!
    fn generate_commands(&self, renderer: &Renderer, index_count: u32, to: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> MResult<()>;

    /// Return `true` if the material is transparent.
    ///
    /// If so, it needs to be rendered back-to-front.
    ///
    /// Default: `false`
    fn is_transparent(&self) -> bool {
        false
    }
}
