use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
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
    pub lightmap_sets: Vec<AddBSPParameterLightmapSet>
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
    pub indices: Vec<ModelTriangle>,

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
        Ok(())
    }
}
