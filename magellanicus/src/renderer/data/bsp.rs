use alloc::vec::Vec;
use core::ops::Range;
use crate::error::MResult;
use crate::renderer::{AddBSPParameter, AddBSPParameterLightmapMaterial, AddShaderParameter, BSPCluster, BSPData, BSPPortal, Renderer};
use crate::renderer::vulkan::{VulkanBSPData, VulkanBSPGeometryData};
use crate::vertex::ModelTriangle;

#[derive(Default)]
pub struct BSP {
    pub vulkan: VulkanBSPData,
    pub geometries: Vec<BSPGeometry>,
    pub bsp_data: BSPData,
    pub cluster_surfaces: Vec<Vec<usize>>,
}

impl BSP {
    pub fn load_from_parameters(renderer: &mut Renderer, mut add_bsp_parameter: AddBSPParameter) -> MResult<Self> {
        struct BSPMaterialData<'a> {
            material_reflexive_index: usize,
            material_data: &'a AddBSPParameterLightmapMaterial,
            lightmap_reflexive_index: usize,
            lightmap_bitmap_index: Option<usize>
        }

        let add_bsp_iterator = add_bsp_parameter
            .lightmap_sets
            .iter()
            .enumerate()
            .map(|i|
                i.1
                    .materials
                    .iter()
                    .enumerate()
                    .zip(core::iter::repeat((i.0, i.1.lightmap_index)))
            )
            .flatten()
            .map(|(material, lightmap)| {
                BSPMaterialData {
                    material_reflexive_index: material.0,
                    material_data: material.1,
                    lightmap_reflexive_index: lightmap.0,
                    lightmap_bitmap_index: lightmap.1
                }
            });

        let count = add_bsp_iterator.clone().count();
        let mut geometries = Vec::with_capacity(count);

        for data in add_bsp_iterator {
            geometries.push(BSPGeometry {
                vulkan: VulkanBSPGeometryData::new(renderer, &add_bsp_parameter, data.material_data, data.lightmap_bitmap_index)?,
                lightmap_index: data.material_data.lightmap_vertices.as_ref().and(data.lightmap_bitmap_index),
                material_reflexive_index: data.material_reflexive_index,
                lightmap_reflexive_index: data.lightmap_reflexive_index
            })
        }

        let mut bsp_data = &mut add_bsp_parameter.bsp_data;
        let mut cluster_surfaces: Vec<Vec<usize>> = Vec::with_capacity(bsp_data.clusters.len());

        // Get all surfaces for all clusters
        for cluster in &mut bsp_data.clusters {
            for subcluster in &mut cluster.subclusters {
                subcluster.surface_indices.sort();
                subcluster.surface_indices.dedup();
            }

            let mut all_surfaces_iter = cluster
                .subclusters
                .iter()
                .map(|c| c.surface_indices.iter())
                .flatten();

            let mut all_surfaces: Vec<usize> = Vec::with_capacity(all_surfaces_iter.clone().count());
            all_surfaces.extend(all_surfaces_iter);
            all_surfaces.sort();
            all_surfaces.dedup();
            all_surfaces.shrink_to_fit();
            cluster_surfaces.push(all_surfaces);
        }

        // Get all ranges for all lightmap sets
        let mut index = 0usize;
        let surfaces_ranges: Vec<Vec<Range<usize>>> = add_bsp_parameter.lightmap_sets.iter().map(|l| {
            l.materials.iter().map(|mat| {
                let new_index = index + mat.surfaces.len();
                let range = index..new_index;
                index = new_index;
                range
            }).collect()
        }).collect();

        // Now convert into triangle indices
        //
        // Maps clusters to lightmaps to materials to triangles
        let mut so_many_vectors: Vec<Vec<Vec<Vec<ModelTriangle>>>> = Vec::with_capacity(bsp_data.clusters.len());
        for surfaces_in_cluster in &cluster_surfaces {
            let surface_ranges_filtered: Vec<Vec<Vec<ModelTriangle>>> = surfaces_ranges
                .iter()
                .enumerate()
                .map(|(lightmap_set_index, lightmap_set)| {
                    lightmap_set.iter().enumerate().map(|(material_index, material_range)| {
                        surfaces_in_cluster.iter().filter_map(|index| if material_range.contains(index) {
                            Some(add_bsp_parameter
                                .lightmap_sets[lightmap_set_index]
                                .materials[material_index]
                                .surfaces[*index - material_range.start])
                        }
                        else {
                            None
                        }).collect()
                    }).collect()
                }).collect();
            so_many_vectors.push(surface_ranges_filtered);
        }

        let vulkan = VulkanBSPData::new(renderer, &add_bsp_parameter, &so_many_vectors)?;

        Ok(Self { vulkan, geometries, bsp_data: add_bsp_parameter.bsp_data, cluster_surfaces })
    }
}

pub struct BSPGeometry {
    pub vulkan: VulkanBSPGeometryData,
    pub lightmap_index: Option<usize>,

    pub material_reflexive_index: usize,
    pub lightmap_reflexive_index: usize
}
