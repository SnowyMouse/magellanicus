use alloc::sync::Arc;
use alloc::string::String;

pub struct Sky {
    pub geometry: Option<Arc<String>>,

    pub outdoor_fog_color: [f32; 3],
    pub outdoor_fog_maximum_density: f32,
    pub outdoor_fog_start_distance: f32,
    pub outdoor_fog_opaque_distance: f32,

    pub indoor_fog_color: [f32; 3],
    pub indoor_fog_maximum_density: f32,
    pub indoor_fog_start_distance: f32,
    pub indoor_fog_opaque_distance: f32,
}
