use crate::error::MResult;
use crate::renderer::{AddBSPParameter, AddBSPParameterLightmapMaterial, DefaultType, Renderer};

use crate::renderer::vulkan::vertex::{VulkanModelVertex, VulkanModelVertexTextureCoords};
use crate::renderer::vulkan::{default_allocation_create_info, VulkanPipelineType};
use crate::vertex::ModelTriangle;
use std::collections::BTreeMap;
use std::string::String;
use std::sync::Arc;
use std::vec::Vec;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::sampler::{Sampler, SamplerAddressMode, SamplerCreateInfo};
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use vulkano::pipeline::Pipeline;

pub struct VulkanBSPData {
    pub lightmap_images: BTreeMap<usize, Arc<PersistentDescriptorSet>>,
    pub cluster_surface_index_buffers: Vec<Vec<Vec<Option<Subbuffer<[u16]>>>>>,
    pub null_lightmaps: Arc<PersistentDescriptorSet>
}

impl VulkanBSPData {
    pub fn new(renderer: &mut Renderer, param: &AddBSPParameter, surfaces_ranges: &Vec<Vec<Vec<Vec<ModelTriangle>>>>) -> MResult<Self> {
        let shader_environment_pipeline = renderer.renderer.pipelines[&VulkanPipelineType::ShaderEnvironment].get_pipeline();
        let mut images = BTreeMap::new();
        if let Some(n) = &param.lightmap_bitmap {
            let image = renderer
                .bitmaps
                .get(n)
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
                    SamplerCreateInfo {
                        address_mode: [
                            SamplerAddressMode::ClampToEdge,
                            SamplerAddressMode::ClampToEdge,
                            SamplerAddressMode::ClampToEdge
                        ],
                        ..SamplerCreateInfo::simple_repeat_linear_no_mipmap()
                    }
                )?;

                let descriptor_set = PersistentDescriptorSet::new(
                    renderer.renderer.descriptor_set_allocator.as_ref(),
                    shader_environment_pipeline.layout().set_layouts()[1].clone(),
                    [
                        WriteDescriptorSet::sampler(0, sampler),
                        WriteDescriptorSet::image_view(1, lightmap),
                    ],
                    []
                )?;

                images.insert(i, descriptor_set);
            }
        }

        let cluster_surface_index_buffers: Vec<Vec<Vec<Option<Subbuffer<[u16]>>>>> = surfaces_ranges
            .iter()
            .map(|cluster| cluster
                .iter()
                .map(|lightmap| {
                    lightmap
                        .iter()
                        .map(|material| {
                            if material.is_empty() {
                                None
                            }
                            else {
                                let indices: Vec<u16> = material
                                    .iter()
                                    .map(|triangle| triangle.indices.iter().copied())
                                    .flatten()
                                    .collect();
                                let index_buffer = Buffer::from_iter(
                                    renderer.renderer.memory_allocator.clone(),
                                    BufferCreateInfo { usage: BufferUsage::INDEX_BUFFER, ..Default::default() },
                                    default_allocation_create_info(),
                                    indices
                                ).unwrap();
                                Some(index_buffer)
                            }
                        })
                        .collect()
                })
                .collect()
            )
            .collect();

        let null_set = PersistentDescriptorSet::new(
            renderer.renderer.descriptor_set_allocator.as_ref(),
            shader_environment_pipeline.layout().set_layouts()[1].clone(),
            [
                WriteDescriptorSet::sampler(0, renderer.renderer.default_2d_sampler.clone()),
                WriteDescriptorSet::image_view(1, ImageView::new_default(renderer.get_default_2d(DefaultType::White).vulkan.image.clone())?),
            ],
            []
        ).unwrap();

        Ok(Self { lightmap_images: images, cluster_surface_index_buffers, null_lightmaps: null_set })
    }
}

pub struct VulkanBSPGeometryData {
    pub vertex_buffer: Subbuffer<[VulkanModelVertex]>,
    pub texture_coords_buffer: Subbuffer<[VulkanModelVertexTextureCoords]>,
    pub lightmap_texture_coords_buffer: Option<Subbuffer<[VulkanModelVertexTextureCoords]>>,
    pub index_buffer: Subbuffer<[u16]>,
    pub shader: Arc<String>
}

impl VulkanBSPGeometryData {
    pub fn new(renderer: &mut Renderer, _param: &AddBSPParameter, material: &AddBSPParameterLightmapMaterial, lightmap_index: Option<usize>) -> MResult<Self> {
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

        let (shader, ..) = renderer.shaders.get_key_value(&material.shader).expect("shader?????");

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

        let index_iter: Vec<u16> = material
            .surfaces
            .iter()
            .map(|t| t.indices.iter())
            .flatten()
            .copied()
            .collect();

        let index_buffer = Buffer::from_iter(
            renderer.renderer.memory_allocator.clone(),
            BufferCreateInfo { usage: BufferUsage::INDEX_BUFFER, ..Default::default() },
            default_allocation_create_info(),
            index_iter
        )?;

        Ok(VulkanBSPGeometryData { vertex_buffer, texture_coords_buffer, lightmap_texture_coords_buffer, shader: shader.clone(), index_buffer })
    }
}
