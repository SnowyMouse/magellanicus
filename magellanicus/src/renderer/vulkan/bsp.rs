use crate::error::MResult;
use crate::renderer::{AddBSPParameter, AddBSPParameterLightmapMaterial, Renderer};

use std::boxed::Box;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::vec::Vec;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::image::sampler::{Sampler, SamplerCreateInfo};
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use crate::renderer::vulkan::default_allocation_create_info;
use crate::renderer::vulkan::vertex::{VulkanModelVertex, VulkanModelVertexTextureCoords};
use crate::vertex::{LightmapVertex, ModelVertex};

#[derive(Default)]
pub struct VulkanBSPData {
    pub images: BTreeMap<usize, (Arc<ImageView>, Arc<Sampler>)>
}

impl VulkanBSPData {
    pub fn new(renderer: &mut Renderer, param: &AddBSPParameter) -> MResult<Self> {
        let mut images = BTreeMap::new();
        if let Some(n) = &param.lightmap_bitmap {
            let image = renderer
                .bitmaps
                .get(param.lightmap_bitmap.as_ref().unwrap())
                .unwrap();

            for i in param.lightmap_sets.iter().filter_map(|b| b.lightmap_index) {
                if images.contains_key(&i) {
                    continue;
                }

                let image = image.bitmaps[i].vulkan.image.clone();

                let lightmap = ImageView::new(
                    image.clone(),
                    ImageViewCreateInfo::from_image(image.as_ref())
                )?;

                let sampler = Sampler::new(
                    renderer.renderer.device.clone(),
                    SamplerCreateInfo::simple_repeat_linear_no_mipmap()
                )?;

                images.insert(i, (lightmap, sampler));
            }
        }

        Ok(Self { images })
    }
}

pub struct VulkanBSPGeometryData {
    pub vertex_buffer: Subbuffer<[VulkanModelVertex]>,
    pub texture_coords_buffer: Subbuffer<[VulkanModelVertexTextureCoords]>,
    pub lightmap_texture_coords_buffer: Option<Subbuffer<[VulkanModelVertexTextureCoords]>>
}

impl VulkanBSPGeometryData {
    pub fn new(renderer: &mut Renderer, param: &AddBSPParameter, material: &AddBSPParameterLightmapMaterial, lightmap_index: Option<usize>) -> MResult<Self> {
        let vertex_buffer = Buffer::from_iter(
            renderer.renderer.memory_allocator.clone(),
            BufferCreateInfo { usage: BufferUsage::VERTEX_BUFFER, ..Default::default() },
            default_allocation_create_info(),
            material.shader_vertices.iter().map(|v| {
                VulkanModelVertex {
                    position: v.position,
                    normal: v.normal,
                    binormal: v.binormal,
                    tangent: v.tangent,
                }
            })
        )?;

        let texture_coords_buffer = Buffer::from_iter(
            renderer.renderer.memory_allocator.clone(),
            BufferCreateInfo { usage: BufferUsage::VERTEX_BUFFER, ..Default::default() },
            default_allocation_create_info(),
            material.shader_vertices.iter().map(|v| {
                VulkanModelVertexTextureCoords {
                    texture_coords: v.texture_coords
                }
            })
        )?;

        let lightmap_texture_coords_buffer: Option<Subbuffer<[VulkanModelVertexTextureCoords]>> = if let Some(v) = material.lightmap_vertices.as_ref().and_then(|f| lightmap_index.is_some().then_some(f)) {
            let buffer = Buffer::from_iter(
                renderer.renderer.memory_allocator.clone(),
                BufferCreateInfo { usage: BufferUsage::VERTEX_BUFFER, ..Default::default() },
                default_allocation_create_info(),
                v
                    .iter()
                    .map(|v| {
                        VulkanModelVertexTextureCoords {
                            texture_coords: v.lightmap_texture_coords
                        }
                    })
            )?;
            Some(buffer)
        }
        else {
            None
        };

        Ok(VulkanBSPGeometryData { vertex_buffer, texture_coords_buffer, lightmap_texture_coords_buffer })
    }
}
