use alloc::vec::Vec;
use alloc::string::String;
use crate::error::MResult;
use crate::renderer::{AddBitmapParameter, Renderer, Resolution};
use crate::renderer::vulkan::VulkanBitmapData;

pub struct Bitmap {
    pub bitmaps: Vec<BitmapBitmap>,
    pub sequences: Vec<BitmapSequence>
}

impl Bitmap {
    pub fn load_from_parameters(renderer: &mut Renderer, parameter: AddBitmapParameter) -> MResult<Self> {
        parameter.validate()?;

        let mut bitmaps = Vec::with_capacity(parameter.bitmaps.len());
        for b in parameter.bitmaps {
            let bitmap = BitmapBitmap {
                resolution: b.resolution,
                bitmap_type: b.bitmap_type,
                vulkan: VulkanBitmapData::new(&mut renderer.renderer, &b)?
            };
            bitmaps.push(bitmap);
        }

        Ok(Self {
            sequences: parameter.sequences,
            bitmaps
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BitmapType {
    Dim2D,
    Dim3D { depth: u32 },
    Cubemap
}

pub struct BitmapBitmap {
    pub vulkan: VulkanBitmapData,
    pub resolution: Resolution,
    pub bitmap_type: BitmapType
}

pub enum BitmapSequence {
    Bitmap { first: usize, count: usize },
    Sprites { sprites: Vec<BitmapSprite> }
}

pub struct BitmapSprite {
    pub bitmap: usize,
    pub top: f32,
    pub left: f32,
    pub bottom: f32,
    pub right: f32
}
