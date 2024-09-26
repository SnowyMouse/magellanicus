use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::string::String;
use alloc::vec::Vec;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use data::*;

pub use parameters::*;
use crate::renderer::vulkan::VulkanRenderer;
use player_viewport::*;

mod parameters;
mod vulkan;
mod data;
mod player_viewport;

pub struct Renderer {
    renderer: VulkanRenderer,
    player_viewports: Vec<PlayerViewport>,

    bitmaps: BTreeMap<Arc<String>, Bitmap>,
    shaders: BTreeMap<Arc<String>, Shader>,
    geometries: BTreeMap<Arc<String>, Geometry>,
    skies: BTreeMap<Arc<String>, Sky>,
    bsps: BTreeMap<Arc<String>, BSP>
}

impl Renderer {
    /// Initialize a new renderer.
    ///
    /// If rendering to a window is desired, set `surface` to true.
    ///
    /// Errors if:
    /// - `parameters` is invalid
    /// - the renderer backend could not be initialized for some reason
    pub fn new(parameters: RendererParameters, surface: Arc<impl HasRawWindowHandle + HasRawDisplayHandle + Send + Sync + 'static>) -> Result<Self, String> {
        if !(1..=4).contains(&parameters.number_of_viewports) {
            return Err(alloc::format!("number of viewports was set to {}, but only 1-4 are supported", parameters.number_of_viewports))
        }

        let player_viewports = Vec::with_capacity(parameters.number_of_viewports);

        // TODO: add player viewports

        Ok(Self {
            renderer: VulkanRenderer::new(&parameters, surface.clone()).map_err(|e| alloc::format!("Vulkan init fail: {e}"))?,
            player_viewports,
            bitmaps: BTreeMap::new(),
            shaders: BTreeMap::new(),
            geometries: BTreeMap::new(),
            skies: BTreeMap::new(),
            bsps: BTreeMap::new()
        })
    }

    /// Clear all data without resetting the renderer.
    ///
    /// All objects added with `add_` methods will be cleared.
    pub fn reset(&mut self) {
        self.bitmaps.clear();
        self.shaders.clear();
        self.geometries.clear();
        self.skies.clear();
        self.bsps.clear();
    }

    /// Add a bitmap with the given parameters.
    ///
    /// If the bitmap has the same path as an already loaded bitmap, that bitmap will be replaced.
    ///
    /// This will error if:
    /// - `bitmap` is invalid
    /// - replacing a bitmap would break any dependencies (HUDs, shaders, etc.)
    pub fn add_bitmap(&mut self, path: &str, bitmap: AddBitmapParameter) -> Result<(), String> {
        todo!()
    }

    /// Add a shader.
    ///
    /// If the shader has a same path as an already loaded shader, that shader will be replaced.
    ///
    /// This will error if:
    /// - `shader` is invalid
    /// - `shader` contains invalid dependencies
    /// - replacing a shader would break any dependencies
    pub fn add_shader(&mut self, path: &str, shader: AddShaderParameter) -> Result<(), String> {
        todo!()
    }

    /// Add a geometry.
    ///
    /// This will error if:
    /// - `geometry` is invalid
    /// - `geometry` contains invalid dependencies
    /// - replacing a geometry would break any dependencies
    pub fn add_geometry(&mut self, path: &str, geometry: AddGeometryParameter) -> Result<(), String> {
        todo!()
    }

    /// Add a sky.
    ///
    /// This will error if:
    /// - `sky` is invalid
    /// - `sky` contains invalid dependencies
    pub fn add_sky(&mut self, path: &str, sky: AddSkyParameter) -> Result<(), String> {
        todo!()
    }

    /// Set the current BSP.
    ///
    /// This will error if:
    /// - `bsp` is invalid
    /// - `bsp` contains invalid dependencies
    pub fn set_bsp(&mut self, path: &str, bsp: SetBSPParameter) -> Result<(), String> {
        todo!()
    }
}
