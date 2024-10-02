use crate::renderer::vulkan::VulkanPlayerViewport;

#[derive(Copy, Clone, Debug)]
pub struct PlayerViewport {
    /// Relative X of the viewport (0.0-1.0)
    pub rel_x: f32,

    /// Relative Y of the viewport (0.0-1.0)
    pub rel_y: f32,

    /// Width of the viewport (0.0-1.0)
    pub rel_width: f32,

    /// Height of the viewport (0.0-1.0)
    pub rel_height: f32,

    /// Camera data
    pub camera: Camera
}

impl Default for PlayerViewport {
    fn default() -> Self {
        PlayerViewport {
            rel_x: 0.0,
            rel_y: 0.0,
            rel_width: 1.0,
            rel_height: 1.0,
            camera: Camera::default()
        }
    }
}

pub use glam::Vec3 as Vec3;

#[derive(Copy, Clone, Debug)]
pub struct Camera {
    /// FoV in radians (default = 70 degrees)
    pub fov: f32,

    /// Position in the map of the camera
    pub position: Vec3,

    /// Rotation of the camera
    pub rotation: Vec3
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov: 70.0f32.to_radians(),
            position: glam::Vec3::default(),
            rotation: glam::Vec3::from([0.0, 1.0, 0.0])
        }
    }
}
