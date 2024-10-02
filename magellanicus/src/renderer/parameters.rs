mod bitmap;
mod geometry;
mod shader;
mod bsp;
mod sky;

pub use bitmap::*;
pub use geometry::*;
pub use shader::*;
pub use bsp::*;
pub use sky::*;

/// Used for initializing a renderer.
///
/// These fields can be changed later with their respective set_* methods.
pub struct RendererParameters {
    /// Resolution of the renderer in (width, height)
    ///
    /// Default = 640x480
    pub resolution: Resolution,

    // TODO: Separate number_of_viewports from this?
    /// Number of viewports (must be 1-4)
    ///
    /// Default = 1
    pub number_of_viewports: usize,

    /// Enable vSync.
    ///
    /// Default = false
    pub vsync: bool
}

impl Default for RendererParameters {
    fn default() -> Self {
        Self {
            resolution: Resolution { width: 640, height: 480 },
            number_of_viewports: 1,
            vsync: false
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32
}
