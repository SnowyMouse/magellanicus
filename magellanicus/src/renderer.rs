use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use alloc::vec;
use alloc::borrow::ToOwned;
use std::println;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use data::*;

pub use parameters::*;
use crate::renderer::vulkan::VulkanRenderer;
use player_viewport::*;
use crate::error::{Error, MResult};

pub use player_viewport::Camera;
pub use glam::Vec3;

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

    default_bitmaps: Option<DefaultBitmaps>,
    current_bsp: Option<Arc<String>>
}

impl Renderer {
    /// Initialize a new renderer.
    ///
    /// Errors if:
    /// - `parameters` is invalid
    /// - the renderer backend could not be initialized for some reason
    pub fn new(parameters: RendererParameters, surface: Arc<impl HasRawWindowHandle + HasRawDisplayHandle + Send + Sync + 'static>) -> MResult<Self> {
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
            renderer: VulkanRenderer::new(&parameters, surface.clone(), parameters.resolution)?,
            player_viewports,
            bitmaps: BTreeMap::new(),
            shaders: BTreeMap::new(),
            geometries: BTreeMap::new(),
            skies: BTreeMap::new(),
            bsps: BTreeMap::new(),
            current_bsp: None,
            default_bitmaps: None
        };

        Ok(result)
    }

    /// Clear all data without resetting the renderer.
    ///
    /// All objects added with `add_` methods will be cleared. Additionally, the default bitmaps
    /// will be cleared.
    pub fn reset(&mut self) {
        self.bitmaps.clear();
        self.shaders.clear();
        self.geometries.clear();
        self.skies.clear();
        self.bsps.clear();
        self.current_bsp = None;
        self.default_bitmaps = None;
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

    /// Set the default bitmaps.
    ///
    /// If any bitmaps are not found or are not correct, this function will return `Err` with no
    /// effect.
    ///
    /// This can only be called once, after which subsequent calls will error with no effect until
    /// cleared.
    pub fn set_default_bitmaps(&mut self, set_default_bitmaps: SetDefaultBitmaps) -> MResult<()> {
        if self.default_bitmaps.is_some() {
            return Err(Error::from_data_error_string("default bitmaps already set; use clear() to clear these".to_owned()))
        }

        let Some(default_2d) = self.bitmaps.get_key_value(&set_default_bitmaps.default_2d) else {
            return Err(Error::from_data_error_string(format!("Bitmap {} was not loaded", set_default_bitmaps.default_2d)))
        };
        let Some(default_3d) = self.bitmaps.get_key_value(&set_default_bitmaps.default_3d) else {
            return Err(Error::from_data_error_string(format!("Bitmap {} was not loaded", set_default_bitmaps.default_3d)))
        };
        let Some(default_cubemap) = self.bitmaps.get_key_value(&set_default_bitmaps.default_cubemap) else {
            return Err(Error::from_data_error_string(format!("Bitmap {} was not loaded", set_default_bitmaps.default_cubemap)))
        };

        if default_2d.1.bitmaps.len() != 4
            || !default_2d.1.bitmaps.iter().all(|b| b.bitmap_type == BitmapType::Dim2D)
            || !default_2d.1.sequences.iter().all(|b| matches!(b, BitmapSequence::Bitmap { .. })) {
            return Err(Error::from_data_error_string(format!("Bitmap {} is not 4x 2D bitmaps", set_default_bitmaps.default_2d)))
        }

        if default_3d.1.bitmaps.len() != 4
            || !default_3d.1.bitmaps.iter().all(|b| matches!(b.bitmap_type, BitmapType::Dim3D { .. }))
            || !default_3d.1.sequences.iter().all(|b| matches!(b, BitmapSequence::Bitmap { .. })) {
            return Err(Error::from_data_error_string(format!("Bitmap {} is not 4x 3D bitmaps", set_default_bitmaps.default_3d)))
        }

        if default_cubemap.1.bitmaps.len() != 4
            || !default_cubemap.1.bitmaps.iter().all(|b| b.bitmap_type == BitmapType::Cubemap)
            || !default_cubemap.1.sequences.iter().all(|b| matches!(b, BitmapSequence::Bitmap { .. })) {
            return Err(Error::from_data_error_string(format!("Bitmap {} is not 4x cubemap bitmaps", set_default_bitmaps.default_cubemap)))
        }

        self.default_bitmaps = Some(DefaultBitmaps {
            default_2d: default_2d.0.to_owned(),
            default_3d: default_3d.0.to_owned(),
            default_cubemap: default_cubemap.0.to_owned(),
        });

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
            fov: camera.fov
        }
    }

    /// Get the camera data for the given viewport.
    ///
    /// # Panics
    ///
    /// Panics if `viewport >= self.viewport_count()`
    pub fn get_camera(&self, viewport: usize) -> Camera {
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
}
