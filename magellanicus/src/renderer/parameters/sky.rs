use alloc::format;
use alloc::string::String;
use crate::error::{Error, MResult};
use crate::renderer::Renderer;

pub struct AddSkyParameter {
    pub geometry: Option<String>,

    pub outdoor_fog_color: [f32; 3],
    pub outdoor_fog_maximum_density: f32,
    pub outdoor_fog_start_distance: f32,
    pub outdoor_fog_opaque_distance: f32,

    pub indoor_fog_color: [f32; 3],
    pub indoor_fog_maximum_density: f32,
    pub indoor_fog_start_distance: f32,
    pub indoor_fog_opaque_distance: f32,
}

impl AddSkyParameter {
    pub(crate) fn validate(&self, renderer: &Renderer) -> MResult<()> {
        if !(0.0..=1.0).contains(&self.outdoor_fog_maximum_density) {
            return Err(Error::from_data_error_string(format!("Outdoor fog density is {} which is not between 0 and 1", self.outdoor_fog_maximum_density)))
        }
        if !(0.0..=1.0).contains(&self.indoor_fog_maximum_density) {
            return Err(Error::from_data_error_string(format!("Indoor fog density is {} which is not between 0 and 1", self.indoor_fog_maximum_density)))
        }
        if self.outdoor_fog_start_distance > self.outdoor_fog_opaque_distance || self.outdoor_fog_start_distance < 0.0 {
            return Err(Error::from_data_error_string(format!("Outdoor fog starting distance is {} which is not between 0.0 and {} (opaque distance)", self.indoor_fog_maximum_density, self.indoor_fog_opaque_distance)))
        }
        if self.indoor_fog_start_distance > self.indoor_fog_opaque_distance || self.indoor_fog_start_distance < 0.0 {
            return Err(Error::from_data_error_string(format!("Indoor fog starting distance is {} which is not between 0.0 and {} (opaque distance)", self.indoor_fog_maximum_density, self.indoor_fog_opaque_distance)))
        }
        if let Some(s) = self.geometry.as_ref() {
            if !renderer.geometries.contains_key(s) {
                return Err(Error::from_data_error_string(format!("Fog references skybox geometry {s} which is not loaded")))
            }
        }
        Ok(())
    }
}
