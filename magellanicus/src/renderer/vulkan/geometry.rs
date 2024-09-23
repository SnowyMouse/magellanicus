use std::iter::empty;
use std::sync::Arc;
use crate::vertex::{LightmapVertex, ModelVertex};
use std::vec::Vec;
use crate::error::{Error, MResult};
use crate::renderer::vulkan::vertex::*;

pub struct VulkanMaterialData {
    pub buffers: Arc<VulkanMaterialVertexBuffers>
}

pub struct VulkanMaterialVertexBuffers {
    pub positions: Vec<VulkanModelVertexPosition>,
    pub normals: Vec<VulkanModelVertexNormal>,
    pub binormals: Vec<VulkanModelVertexBinormal>,
    pub tangents: Vec<VulkanModelVertexTangent>,
    pub texture_coords: Vec<VulkanModelVertexTextureCoords>,
    pub lightmap_coords: Option<Vec<VulkanModelVertexLightmapTextureCoords>>,
    pub indices: Vec<u16>
}

impl VulkanMaterialVertexBuffers {
    /// Load buffers from the given vertices.
    ///
    /// Errors if:
    /// - `lightmap_vertices` is not empty but does not have the same number of vertices as `vertices`
    /// - `indices` contains vertex indices that are out-of-bounds (i.e. `index >= vertices.collect().len()`)
    /// - `vertices` contains more than 65535 vertices
    pub fn new(
        vertices: impl IntoIterator<Item = ModelVertex>,
        lightmap_vertices: impl IntoIterator<Item = LightmapVertex>,
        indices: impl IntoIterator<Item = u16>
    ) -> MResult<Arc<VulkanMaterialVertexBuffers>> {
        Self::new_from_iters(
            vertices.into_iter(),
            lightmap_vertices.into_iter(),
            indices.into_iter()
        )
    }

    fn new_from_iters(
        vertices: impl Iterator<Item = ModelVertex>,
        lightmap_vertices: impl Iterator<Item = LightmapVertex>,
        indices: impl Iterator<Item = u16>
    ) -> MResult<Arc<VulkanMaterialVertexBuffers>> {
        // Prevent allocating/loading too many vertices
        const MAX_VERTEX_LIMIT: usize = u16::MAX as usize;
        const MAX_VERTEX_ALLOC_LIMIT: usize = MAX_VERTEX_LIMIT + 1;

        let size_hint = vertices.size_hint().0;
        if size_hint > MAX_VERTEX_LIMIT {
            return Err(Error::DataError { error: std::format!("Vertex iterator will exceed the vertex limit ({size_hint} > 65535)") })
        }

        let mut positions_buf: Vec<VulkanModelVertexPosition> = Vec::with_capacity(size_hint);
        let mut normals_buf: Vec<VulkanModelVertexNormal> = Vec::with_capacity(size_hint);
        let mut binormals_buf: Vec<VulkanModelVertexBinormal> = Vec::with_capacity(size_hint);
        let mut tangents_buf: Vec<VulkanModelVertexTangent> = Vec::with_capacity(size_hint);
        let mut texture_coords_buf: Vec<VulkanModelVertexTextureCoords> = Vec::with_capacity(size_hint);

        // Don't take more than MAX_VERTEX_ALLOC_LIMIT in case size_hint vastly underestimated the actual vertex count
        for ModelVertex { position, normal, binormal, tangent, texture_coords } in vertices.take(MAX_VERTEX_ALLOC_LIMIT) {
            positions_buf.push(VulkanModelVertexPosition { position });
            normals_buf.push(VulkanModelVertexNormal { normal });
            binormals_buf.push(VulkanModelVertexBinormal { binormal });
            tangents_buf.push(VulkanModelVertexTangent { tangent });
            texture_coords_buf.push(VulkanModelVertexTextureCoords { texture_coords });
        }

        positions_buf.shrink_to_fit();
        normals_buf.shrink_to_fit();
        binormals_buf.shrink_to_fit();
        tangents_buf.shrink_to_fit();
        texture_coords_buf.shrink_to_fit();

        let vertex_count = positions_buf.len();

        if vertex_count > (u16::MAX as usize) {
            return Err(Error::DataError { error: std::format!("Vertex iterator exceeded the vertex limit ({vertex_count} > 65535)") })
        }

        let mut lightmap_vertices = lightmap_vertices.peekable();
        let mut lightmap_coords_buf = if lightmap_vertices.peek().is_some() {
            let mut lightmap_coords = Vec::<VulkanModelVertexLightmapTextureCoords>::with_capacity(vertex_count + 1);
            for i in lightmap_vertices.take(vertex_count + 1) {
                lightmap_coords.push(VulkanModelVertexLightmapTextureCoords { lightmap_texture_coords: i.lightmap_texture_coords })
            }
            if lightmap_coords.len() != vertex_count {
                return Err(Error::DataError { error: std::format!("Lightmap vertex coordinates count ({}) != vertices count ({vertex_count})", lightmap_coords.len()) })
            }
            lightmap_coords.shrink_to_fit();
            Some(lightmap_coords)
        }
        else {
            None
        };

        let mut indices_buf = Vec::with_capacity(indices.size_hint().0);
        for i in indices {
            if i as usize >= vertex_count {
                return Err(Error::DataError { error: std::format!("vertex index {i} out-of-bounds") })
            }
            indices_buf.push(i);
        }
        indices_buf.shrink_to_fit();

        let buffers = VulkanMaterialVertexBuffers {
            positions: positions_buf,
            normals: normals_buf,
            binormals: binormals_buf,
            tangents: tangents_buf,
            texture_coords: texture_coords_buf,
            lightmap_coords: lightmap_coords_buf,
            indices: indices_buf
        };

        Ok(Arc::new(buffers))
    }
}
