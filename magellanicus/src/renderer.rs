use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use alloc::borrow::ToOwned;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use data::*;

pub use parameters::*;
use crate::renderer::vulkan::VulkanRenderer;
use player_viewport::*;
use crate::error::{Error, MResult};

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
    bsps: BTreeMap<Arc<String>, BSP>,

    current_bsp: Option<Arc<String>>
}

impl Renderer {
    /// Initialize a new renderer.
    ///
    /// If rendering to a window is desired, set `surface` to true.
    ///
    /// Errors if:
    /// - `parameters` is invalid
    /// - the renderer backend could not be initialized for some reason
    pub fn new(parameters: RendererParameters, surface: Arc<impl HasRawWindowHandle + HasRawDisplayHandle + Send + Sync + 'static>) -> MResult<Self> {
        if !(1..=4).contains(&parameters.number_of_viewports) {
            return Err(Error::DataError { error: format!("number of viewports was set to {}, but only 1-4 are supported", parameters.number_of_viewports) })
        }

        let player_viewports = Vec::with_capacity(parameters.number_of_viewports);

        // TODO: add player viewports

        Ok(Self {
            renderer: VulkanRenderer::new(&parameters, surface.clone())?,
            player_viewports,
            bitmaps: BTreeMap::new(),
            shaders: BTreeMap::new(),
            geometries: BTreeMap::new(),
            skies: BTreeMap::new(),
            bsps: BTreeMap::new(),
            current_bsp: None
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
        self.current_bsp = None;
    }

    /// Add a bitmap with the given parameters.
    ///
    /// If the bitmap has the same path as an already loaded bitmap, that bitmap will be replaced.
    ///
    /// This will error if:
    /// - `bitmap` is invalid
    /// - replacing a bitmap would break any dependencies (HUDs, shaders, etc.)
    pub fn add_bitmap(&mut self, path: &str, bitmap: AddBitmapParameter) -> MResult<()> {
        bitmap.validate()?;

        let bitmap = Bitmap::load_from_parameters(self, bitmap)?;
        self.bitmaps.insert(Arc::new(path.to_owned()), bitmap);
        Ok(())
    }

    /// Add a shader.
    ///
    /// If the shader has a same path as an already loaded shader, that shader will be replaced.
    ///
    /// This will error if:
    /// - `shader` is invalid
    /// - `shader` contains invalid dependencies
    /// - replacing a shader would break any dependencies
    pub fn add_shader(&mut self, path: &str, shader: AddShaderParameter) -> MResult<()> {
        shader.validate(self)?;

        let shader = Shader::load_from_parameters(self, shader)?;
        self.shaders.insert(Arc::new(path.to_owned()), shader);
        Ok(())
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

    /// Add a BSP.
    ///
    /// This will error if:
    /// - `bsp` is invalid
    /// - `bsp` contains invalid dependencies
    pub fn add_bsp(&mut self, path: &str, bsp: AddBSPParameter) -> Result<(), String> {
        todo!()
    }

    /// Set the current BSP.
    ///
    /// If `path` is `None`, the BSP will be unloaded.
    ///
    /// Returns `Err` if `path` refers to a BSP that isn't loaded.
    pub fn set_current_bsp(&mut self, path: Option<&str>) -> Result<(), String> {
        if let Some(p) = path {
            let key = self
                .bsps
                .keys()
                .find(|f| f.as_str() == p)
                .map(|b| b.clone());

            if key.is_none() {
                return Err(format!("Can't set current BSP to {path:?}: that BSP is not loaded"))
            }

            self.current_bsp = key;
        }
        else {
            self.current_bsp = None;
        }

        Ok(())
    }
}
