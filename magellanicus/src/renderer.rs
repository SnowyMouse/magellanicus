use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use alloc::vec;
use alloc::borrow::ToOwned;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use data::*;

pub use parameters::*;
use crate::renderer::vulkan::VulkanRenderer;
use player_viewport::*;
use crate::error::{Error, MResult};

pub use player_viewport::Camera;
use glam::Vec3;

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

    default_bitmaps: DefaultBitmaps,
    current_bsp: Option<Arc<String>>
}

impl Renderer {
    /// Initialize a new renderer.
    ///
    /// Errors if:
    /// - `parameters` is invalid
    /// - the renderer backend could not be initialized for some reason
    pub unsafe fn new(surface: &(impl HasRawWindowHandle + HasRawDisplayHandle), parameters: RendererParameters) -> MResult<Self> {
        if parameters.resolution.height == 0 || parameters.resolution.width == 0 {
            return Err(Error::DataError { error: "resolution has 0 on one or more dimensions".to_owned() })
        }

        let mut player_viewports = vec![PlayerViewport::default(); parameters.number_of_viewports];

        match parameters.number_of_viewports {
            1 => {
                player_viewports[0].rel_x = 0.0;
                player_viewports[0].rel_y = 0.0;
                player_viewports[0].rel_width = 1.0;
                player_viewports[0].rel_height = 1.0;
            }
            2 => {
                player_viewports[0].rel_x = 0.0;
                player_viewports[0].rel_y = 0.0;
                player_viewports[0].rel_width = 1.0;
                player_viewports[0].rel_height = 0.5;

                player_viewports[1].rel_x = 0.0;
                player_viewports[1].rel_y = 0.5;
                player_viewports[1].rel_width = 1.0;
                player_viewports[1].rel_height = 0.5;
            }
            3 => {
                player_viewports[0].rel_x = 0.0;
                player_viewports[0].rel_y = 0.0;
                player_viewports[0].rel_width = 1.0;
                player_viewports[0].rel_height = 0.5;

                player_viewports[1].rel_x = 0.0;
                player_viewports[1].rel_y = 0.5;
                player_viewports[1].rel_width = 0.5;
                player_viewports[1].rel_height = 0.5;

                player_viewports[2].rel_x = 0.5;
                player_viewports[2].rel_y = 0.5;
                player_viewports[2].rel_width = 0.5;
                player_viewports[2].rel_height = 0.5;
            }
            4 => {
                player_viewports[0].rel_x = 0.0;
                player_viewports[0].rel_y = 0.0;
                player_viewports[0].rel_width = 0.5;
                player_viewports[0].rel_height = 0.5;

                player_viewports[1].rel_x = 0.5;
                player_viewports[1].rel_y = 0.0;
                player_viewports[1].rel_width = 0.5;
                player_viewports[1].rel_height = 0.5;

                player_viewports[2].rel_x = 0.0;
                player_viewports[2].rel_y = 0.5;
                player_viewports[2].rel_width = 0.5;
                player_viewports[2].rel_height = 0.5;

                player_viewports[3].rel_x = 0.5;
                player_viewports[3].rel_y = 0.5;
                player_viewports[3].rel_width = 0.5;
                player_viewports[3].rel_height = 0.5;
            }
            n => return Err(Error::DataError { error: format!("number of viewports was set to {n}, but only 1-4 are supported") })
        }

        let mut result = Self {
            renderer: VulkanRenderer::new(&parameters, surface)?,
            player_viewports,
            bitmaps: BTreeMap::new(),
            shaders: BTreeMap::new(),
            geometries: BTreeMap::new(),
            skies: BTreeMap::new(),
            bsps: BTreeMap::new(),
            current_bsp: None,
            default_bitmaps: DefaultBitmaps::default()
        };

        populate_default_bitmaps(&mut result)?;

        Ok(result)
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
        self.default_bitmaps = DefaultBitmaps::default();

        populate_default_bitmaps(self).unwrap();
    }

    /// Add a bitmap with the given parameters.
    ///
    /// Note that replacing bitmaps is not yet supported.
    ///
    /// This will error if:
    /// - `bitmap` is invalid
    /// - replacing a bitmap would break any dependencies (HUDs, shaders, etc.)
    pub fn add_bitmap(&mut self, path: &str, bitmap: AddBitmapParameter) -> MResult<()> {
        let bitmap_path = Arc::new(path.to_owned());
        if self.bsps.contains_key(&bitmap_path) {
            return Err(Error::from_data_error_string(format!("{path} already exists (replacing bitmaps is not yet supported)")))
        }

        bitmap.validate()?;
        let bitmap = Bitmap::load_from_parameters(self, bitmap)?;
        self.bitmaps.insert(bitmap_path, bitmap);
        Ok(())
    }

    /// Add a shader.
    ///
    /// Note that replacing shaders is not yet supported.
    ///
    /// This will error if:
    /// - `pipeline` is invalid
    /// - `pipeline` contains invalid dependencies
    /// - replacing a pipeline would break any dependencies
    pub fn add_shader(&mut self, path: &str, shader: AddShaderParameter) -> MResult<()> {
        let shader_path = Arc::new(path.to_owned());
        if self.shaders.contains_key(&shader_path) {
            return Err(Error::from_data_error_string(format!("{path} already exists (replacing shaders is not yet supported)")))
        }

        shader.validate(self)?;
        let shader = Shader::load_from_parameters(self, shader)?;
        self.shaders.insert(shader_path, shader);
        Ok(())
    }

    /// Add a geometry.
    ///
    /// Note that replacing geometries is not yet supported.
    ///
    /// This will error if:
    /// - `geometry` is invalid
    /// - `geometry` contains invalid dependencies
    /// - replacing a geometry would break any dependencies
    #[allow(unused_variables)]
    pub fn add_geometry(&mut self, path: &str, geometry: AddGeometryParameter) -> MResult<()> {
        todo!()
    }

    /// Add a sky.
    ///
    /// This will error if:
    /// - `sky` is invalid
    /// - `sky` contains invalid dependencies
    pub fn add_sky(&mut self, path: &str, sky: AddSkyParameter) -> MResult<()> {
        sky.validate(self)?;

        self.skies.insert(Arc::new(path.to_owned()), Sky {
            geometry: sky.geometry.map(|s| self.geometries.get_key_value(&s).unwrap().0.clone()),
            outdoor_fog_color: sky.outdoor_fog_color,
            outdoor_fog_maximum_density: sky.outdoor_fog_maximum_density,
            outdoor_fog_start_distance: sky.outdoor_fog_start_distance,
            outdoor_fog_opaque_distance: sky.outdoor_fog_opaque_distance,
            indoor_fog_color: sky.indoor_fog_color,
            indoor_fog_maximum_density: sky.indoor_fog_maximum_density,
            indoor_fog_start_distance: sky.indoor_fog_start_distance,
            indoor_fog_opaque_distance: sky.indoor_fog_opaque_distance,
        });

        Ok(())
    }

    /// Add a BSP.
    ///
    /// Note that replacing BSPs is not yet supported.
    ///
    /// This will error if:
    /// - `bsp` is invalid
    /// - `bsp` contains invalid dependencies
    pub fn add_bsp(&mut self, path: &str, bsp: AddBSPParameter) -> MResult<()> {
        let bsp_path = Arc::new(path.to_owned());
        if self.bsps.contains_key(&bsp_path) {
            return Err(Error::from_data_error_string(format!("{path} already exists (replacing BSPs is not yet supported)")))
        }

        bsp.validate(self)?;
        let bsp = BSP::load_from_parameters(self, bsp)?;
        self.bsps.insert(bsp_path, bsp);
        Ok(())
    }

    /// Set the current BSP.
    ///
    /// If `path` is `None`, the BSP will be unloaded.
    ///
    /// Returns `Err` if `path` refers to a BSP that isn't loaded.
    pub fn set_current_bsp(&mut self, path: Option<&str>) -> MResult<()> {
        if let Some(p) = path {
            let key = self
                .bsps
                .keys()
                .find(|f| f.as_str() == p)
                .map(|b| b.clone());

            if key.is_none() {
                return Err(Error::from_data_error_string(format!("Can't set current BSP to {path:?}: that BSP is not loaded")))
            }

            self.current_bsp = key;
        }
        else {
            self.current_bsp = None;
        }

        Ok(())
    }

    /// Rebuild the swapchain.
    ///
    /// You must use this when the window is resized or if the swapchain is invalidated.
    pub fn rebuild_swapchain(&mut self, parameters: RendererParameters) -> MResult<()> {
        if parameters.resolution.height == 0 || parameters.resolution.width == 0 {
            return Err(Error::DataError { error: "resolution has 0 on one or more dimensions".to_owned() })
        }
        self.renderer.rebuild_swapchain(
            &parameters
        )
    }

    /// Set the position, rotation, and FoV of the camera for the given viewport.
    ///
    /// `fov` must be in radians, and `position` must be a vector.
    ///
    /// # Panics
    ///
    /// Panics if `viewport >= self.viewport_count()` or if `!(camera.fov > 0.0 && camera.fov < PI)`
    pub fn set_camera_for_viewport(&mut self, viewport: usize, camera: Camera) {
        assert!(camera.fov > 0.0 && camera.fov < core::f32::consts::PI, "camera.fov is not between 0 (exclusive) and pi (exclusive)");

        let viewport = &mut self.player_viewports[viewport];
        viewport.camera = Camera {
            position: camera.position,
            rotation: Vec3::from(camera.rotation).try_normalize().unwrap_or(Vec3::new(0.0, 1.0, 0.0)).into(),
            fov: camera.fov,
            lightmaps: camera.lightmaps,
            fog: camera.fog
        }
    }

    /// Get the camera data for the given viewport.
    ///
    /// # Panics
    ///
    /// Panics if `viewport >= self.viewport_count()`
    pub fn get_camera_for_viewport(&self, viewport: usize) -> Camera {
        self.player_viewports[viewport].camera
    }

    /// Get the number of viewports.
    pub fn get_viewport_count(&self) -> usize {
        self.player_viewports.len()
    }

    /// Draw a frame.
    ///
    /// If `true`, the swapchain needs rebuilt.
    pub fn draw_frame(&mut self) -> MResult<bool> {
        VulkanRenderer::draw_frame(self)
    }

    fn get_default_2d(&self, default_type: DefaultType) -> &BitmapBitmap {
        &self.bitmaps[&self.default_bitmaps.default_2d].bitmaps[default_type as usize]
    }
    fn get_or_default_2d(&self, bitmap: &Option<String>, bitmap_index: usize, default_type: DefaultType) -> &BitmapBitmap {
        match bitmap.as_ref() {
            Some(n) => &self.bitmaps[n].bitmaps[bitmap_index],
            None => &self.get_default_2d(default_type)
        }
    }
    fn get_or_default_3d(&self, bitmap: &Option<String>, bitmap_index: usize, default_type: DefaultType) -> &BitmapBitmap {
        match bitmap.as_ref() {
            Some(n) => &self.bitmaps[n].bitmaps[bitmap_index],
            None => &self.bitmaps[&self.default_bitmaps.default_3d].bitmaps[default_type as usize]
        }
    }
    fn get_or_default_cubemap(&self, bitmap: &Option<String>, bitmap_index: usize, default_type: DefaultType) -> &BitmapBitmap {
        match bitmap.as_ref() {
            Some(n) => &self.bitmaps[n].bitmaps[bitmap_index],
            None => &self.bitmaps[&self.default_bitmaps.default_cubemap].bitmaps[default_type as usize]
        }
    }
}

#[repr(usize)]
enum DefaultType {
    /// Describes a map with all channels set to 0x00.
    ///
    /// This provides a texture that does nothing on alpha blend, min, add, or subtract.
    Null,

    /// Describes a map with all channels set to 0xFF.
    ///
    /// This provides a texture that does nothing on multiply/min.
    White,

    /// Describes a map with red, green, and blue set to 0x7F and alpha set to 0xFF.
    ///
    /// This provides a texture that does nothing on double multiply.
    Gray,

    /// Describes a map with red and green set to 0x7F and blue and alpha set to 0xFF.
    ///
    /// This provides a neutral vector map.
    Vector
}
