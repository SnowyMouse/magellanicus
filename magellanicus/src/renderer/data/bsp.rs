use std::prelude::rust_2015::Vec;
use crate::error::MResult;
use crate::renderer::{AddBSPParameter, AddShaderParameter, Renderer};
use crate::renderer::vulkan::{VulkanBSPData, VulkanBSPGeometryData};

#[derive(Default)]
pub struct BSP {
    pub vulkan: VulkanBSPData,
    pub geometries: Vec<BSPGeometry>
}

impl BSP {
    pub fn load_from_parameters(renderer: &mut Renderer, add_bsp_parameter: AddBSPParameter) -> MResult<Self> {
        let add_bsp_iterator = add_bsp_parameter
            .lightmap_sets
            .iter()
            .map(|i| i.materials.iter().zip(core::iter::repeat(i.lightmap_index)))
            .flatten();

        let count = add_bsp_iterator.clone().count();
        let mut geometries = Vec::with_capacity(count);

        let vulkan = VulkanBSPData::new(renderer, &add_bsp_parameter)?;
        for (material, lightmap_index) in add_bsp_iterator {
            geometries.push(BSPGeometry {
                vulkan: VulkanBSPGeometryData::new(renderer, &add_bsp_parameter, material, lightmap_index)?,
                lightmap_index: material.lightmap_vertices.as_ref().and(lightmap_index),
            })
        }

        Ok(Self { vulkan, geometries })
    }
}

pub struct BSPGeometry {
    pub vulkan: VulkanBSPGeometryData,
    pub lightmap_index: Option<usize>
}
