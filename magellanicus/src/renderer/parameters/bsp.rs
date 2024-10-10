use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use alloc::borrow::ToOwned;
use alloc::vec;
use glam::Vec3;
use crate::error::{Error, MResult};
use crate::renderer::data::{Bitmap, Shader, ShaderType};
use crate::renderer::Renderer;
use crate::vertex::{LightmapVertex, ModelTriangle, ModelVertex};

pub struct AddBSPParameter {
    /// Path to the bitmap.
    ///
    /// If `Some`, this bitmap MUST already be imported.
    pub lightmap_bitmap: Option<String>,

    /// All geometries of the BSP.
    pub lightmap_sets: Vec<AddBSPParameterLightmapSet>,

    /// BSP data
    pub bsp_data: BSPData
}

pub struct AddBSPParameterLightmapSet {
    /// The bitmap index of the lightmap.
    ///
    /// This cannot be `Some` if `SetBSPParameter::lightmap_bitmap` is `None`.
    ///
    /// NOTE: This refers to the bitmap index, not a sequence index.
    pub lightmap_index: Option<usize>,

    /// Describes all materials/geometries.
    pub materials: Vec<AddBSPParameterLightmapMaterial>
}

pub struct AddBSPParameterLightmapMaterial {
    /// Describes pipeline vertices.
    pub shader_vertices: Vec<ModelVertex>,

    /// Describes lightmap vertices.
    ///
    /// Must be None or have the same length as `vertices`
    pub lightmap_vertices: Option<Vec<LightmapVertex>>,

    /// Describes each triangle.
    pub surfaces: Vec<ModelTriangle>,

    /// Describes the pipeline used for material.
    pub shader: String
}

impl AddBSPParameter {
    pub(crate) fn validate(&self, renderer: &Renderer) -> MResult<()> {
        let lightmap_bitmap: Option<(&Bitmap, &str)> = if let Some(path) = self.lightmap_bitmap.as_ref() {
            let Some(bitmap) = renderer.bitmaps.get(path) else {
                return Err(Error::from_data_error_string(format!("BSP refers to lightmap bitmap {path} which is not loaded in the renderer")))
            };
            Some((bitmap, path))
        }
        else {
            None
        };

        for (lightmap_index, lightmap) in self.lightmap_sets.iter().enumerate() {
            if let Some(bitmap_index) = lightmap.lightmap_index {
                let Some((bitmap, path)) = lightmap_bitmap else {
                    return Err(Error::from_data_error_string(format!("BSP lightmap #{lightmap_index} has a bitmap index, but no lightmap bitmap is set")))
                };
                let bitmap_count = bitmap.bitmaps.len();
                if bitmap_index >= bitmap_count {
                    return Err(Error::from_data_error_string(format!("BSP lightmap #{lightmap_index} refers to bitmap #{bitmap_index}, but the referenced bitmap {path} has only {bitmap_count} bitmap(s)")))
                }
            }

            for (material_index, material) in lightmap.materials.iter().enumerate() {
                let vertex_count = material.shader_vertices.len();
                if let Some(lightmap_vertex_count) = material.lightmap_vertices.as_ref().map(|v| v.len()) {
                    if lightmap_vertex_count != vertex_count {
                        return Err(Error::from_data_error_string(format!("BSP material #{material_index} of lightmap #{lightmap_index} has a pipeline vertex count of {vertex_count}, but a lightmap vertex count of {lightmap_vertex_count}")))
                    }
                    if lightmap_bitmap.is_none() {
                        return Err(Error::from_data_error_string(format!("BSP material #{material_index} of lightmap #{lightmap_index} has lightmap vertices when no lightmap bitmap is set")))
                    }
                }

                let shader_path = &material.shader;
                let Some(Shader { shader_type, .. }) = renderer.shaders.get(shader_path) else {
                    return Err(Error::from_data_error_string(format!("BSP material #{material_index} of lightmap #{lightmap_index} references pipeline {shader_path} which is not loaded")))
                };

                // No reason we can't actually render this on a BSP, but these tags are intended to
                // only be rendered on objects.
                if *shader_type == ShaderType::Model {
                    return Err(Error::from_data_error_string(format!("BSP material #{material_index} of lightmap #{lightmap_index} references pipeline {shader_path}, a {shader_type:?} type which isn't allowed for BSPs")))
                }
            }
        }

        self.bsp_data.validate(renderer, self)?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BSPData {
    pub nodes: Vec<BSP3DNode>,
    pub planes: Vec<BSP3DPlane>,
    pub leaves: Vec<BSPLeaf>,
    pub clusters: Vec<BSPCluster>,
    pub portals: Vec<BSPPortal>
}

impl Default for BSPData {
    fn default() -> Self {
        Self {
            nodes: vec![BSP3DNode { front_child: None, back_child: None, plane: 0 }],
            planes: vec![BSP3DPlane { angle: [0.0, 1.0, 0.0], offset: 0.0 }],
            leaves: Vec::new(),
            clusters: Vec::new(),
            portals: Vec::new()
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BSP3DNode {
    pub front_child: Option<BSP3DNodeChild>,
    pub back_child: Option<BSP3DNodeChild>,
    pub plane: usize
}

#[derive(Copy, Clone, Debug)]
pub struct BSPLeaf {
    pub cluster: usize
}

#[derive(Clone, Debug)]
pub struct BSPCluster {
    pub sky: Option<String>,
    pub subclusters: Vec<BSPSubcluster>,
    pub cluster_portals: Vec<usize>
}

#[derive(Clone, Debug)]
pub struct BSPSubcluster {
    pub surface_indices: Vec<usize>,
    pub world_bounds_from: [f32; 3],
    pub world_bounds_to: [f32; 3]
}

#[derive(Clone, Debug)]
pub struct BSPPortal {
    pub front_cluster: usize,
    pub back_cluster: usize
}


#[derive(Copy, Clone, Debug)]
pub enum BSP3DNodeChild {
    Node(usize),
    Leaf(usize)
}

impl BSP3DNodeChild {
    pub fn from_flagged_u32(data: u32) -> Option<Self> {
        if data == 0xFFFFFFFF {
            None
        }
        else if (data & 0x80000000) != 0 {
            Some(Self::Leaf((data as usize) & 0x7FFFFFFF))
        }
        else {
            Some(Self::Node((data as usize) & 0x7FFFFFFF))
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BSP3DPlane {
    pub angle: [f32; 3],
    pub offset: f32
}

impl BSPData {
    pub fn find_cluster(&self, position: [f32; 3]) -> Option<usize> {
        self.find_leaf(position).map(|l| self.leaves[l].cluster)
    }

    pub fn find_leaf(&self, position: [f32; 3]) -> Option<usize> {
        let position = Vec3::from(position);
        let mut node = self.nodes[0];
        loop {
            let plane = self.planes[node.plane];
            let angle = Vec3::from(plane.angle);

            if position.dot(angle) >= plane.offset {
                match node.front_child? {
                    BSP3DNodeChild::Node(n) => node = self.nodes[n],
                    BSP3DNodeChild::Leaf(l) => return Some(l)
                }
            }
            else {
                match node.back_child? {
                    BSP3DNodeChild::Node(n) => node = self.nodes[n],
                    BSP3DNodeChild::Leaf(l) => return Some(l)
                }
            }
        }
    }

    fn validate(&self, renderer: &Renderer, full_parameter: &AddBSPParameter) -> MResult<()> {
        if self.nodes.is_empty() {
            return Err(Error::from_data_error_string("No nodes present".to_owned()))
        }

        let mut tested_nodes = alloc::vec![false; self.nodes.len()];
        for (index, _node) in self.nodes.iter().enumerate() {
            self.validate_3d_node(index, self.nodes.len() + 3, &mut tested_nodes)?;
        }
        for (index, leaf) in self.leaves.iter().enumerate() {
            if leaf.cluster >= self.clusters.len() {
                return Err(Error::from_data_error_string(format!("Leaf #{index} points to cluster #{} which does not exist", leaf.cluster)))
            }
        }

        let total_surface_count = full_parameter
            .lightmap_sets
            .iter()
            .map(|b| b.materials.iter().map(|b| b.surfaces.iter()))
            .flatten()
            .flatten()
            .count();

        for (index, cluster) in self.clusters.iter().enumerate() {
            if let Some(sky) = cluster.sky.as_ref() {
                if !renderer.skies.contains_key(sky) {
                    return Err(Error::from_data_error_string(format!("Cluster #{index} points to sky {sky} which has not been loaded")))
                }
            }
            for (sc_index, subcluster) in cluster.subclusters.iter().enumerate() {
                if subcluster.surface_indices.iter().any(|i| *i >= total_surface_count) {
                    return Err(Error::from_data_error_string(format!("Subcluster {sc_index} of cluster #{index} points to an out-of-bounds surface (there are {total_surface_count} surfaces)")))
                }
            }
            for (p_index, _portal) in cluster.cluster_portals.iter().enumerate() {
                if p_index >= self.portals.len() {
                    return Err(Error::from_data_error_string(format!("Portal {p_index} of cluster #{index} points to an out-of-bounds portal (there are {} surfaces)", self.portals.len())))
                }
            }
        }

        for (p_index, portal) in self.portals.iter().enumerate() {
            if portal.front_cluster >= self.clusters.len() || portal.back_cluster >= self.clusters.len() {
                return Err(Error::from_data_error_string(format!("Portal {p_index} points to an out-of-bounds cluster (there are {} surfaces)", self.clusters.len())))
            }
        }

        Ok(())
    }

    fn validate_3d_node(&self, node: usize, mut remaining_tests: usize, nodes_tested: &mut [bool]) -> MResult<()> {
        // Verified to be OK?
        if nodes_tested[node] {
            return Ok(())
        }
        if remaining_tests == 0 {
            return Err(Error::from_data_error_string("infinite loop detected when traversing nodes".to_owned()))
        }
        remaining_tests -= 1;

        let node_data = &self.nodes[node];
        let front_child = node_data.front_child;
        let back_child = node_data.back_child;

        if let Some(front) = front_child {
            self.validate_child(node, front, remaining_tests, nodes_tested)?;
        }

        if let Some(back) = back_child {
            self.validate_child(node, back, remaining_tests, nodes_tested)?;
        }

        nodes_tested[node] = true;
        Ok(())
    }

    #[inline(always)]
    fn validate_child(&self, node: usize, child: BSP3DNodeChild, remaining_tests: usize, nodes_tested: &mut [bool]) -> MResult<()> {
        match child {
            BSP3DNodeChild::Node(n) => {
                if n >= self.nodes.len() {
                    return Err(Error::from_data_error_string(format!("broken BSP: node #{n}, referenced by node #{node}, does not exist")))
                }
                self.validate_3d_node(n, remaining_tests, nodes_tested)?;
            }
            BSP3DNodeChild::Leaf(n) => {
                if n >= self.leaves.len() {
                    return Err(Error::from_data_error_string(format!("broken BSP: leaf #{n}, referenced by node #{node}, does not exist")))
                }
            }
        }
        Ok(())
    }
}
