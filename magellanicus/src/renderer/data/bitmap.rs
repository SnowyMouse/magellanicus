use alloc::vec::Vec;
use alloc::string::String;
use alloc::vec;
use alloc::sync::Arc;
use core::iter;
use crate::error::MResult;
use crate::renderer::{AddBitmapBitmapParameter, AddBitmapParameter, AddBitmapSequenceParameter, BitmapFormat, Renderer, Resolution};
use crate::renderer::vulkan::VulkanBitmapData;

#[derive(Default)]
pub struct DefaultBitmaps {
    pub default_2d: Arc<String>,
    pub default_3d: Arc<String>,
    pub default_cubemap: Arc<String>,
}

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

#[derive(Clone)]
pub enum BitmapSequence {
    Bitmap { first: usize, count: usize },
    Sprites { sprites: Vec<BitmapSprite> }
}

#[derive(Clone)]
pub struct BitmapSprite {
    pub bitmap: usize,
    pub top: f32,
    pub left: f32,
    pub bottom: f32,
    pub right: f32
}

pub fn populate_default_bitmaps(renderer: &mut Renderer) -> MResult<()> {
    fn make_add_bitmap_parameter(renderer: &mut Renderer, path: &str, bitmap_type: BitmapType) -> MResult<Arc<String>> {
        fn to_rgbaf32(v: [f32; 4]) -> [u8; 16] {
            let r = v[0].to_le_bytes();
            let g = v[1].to_le_bytes();
            let b = v[2].to_le_bytes();
            let a = v[3].to_le_bytes();
            [
                r[0],r[1],r[2],r[3],
                g[0],g[1],g[2],g[3],
                b[0],b[1],b[2],b[3],
                a[0],a[1],a[2],a[3],
            ]
        }

        // note: black is fully transparent in source data, but all release builds are fully opaque
        // due to a bug with tool.exe
        let null: [u8; 16] = to_rgbaf32([0.0, 0.0, 0.0, 0.0]);
        let white: [u8; 16] = to_rgbaf32([1.0, 1.0, 1.0, 1.0]);
        let gray: [u8; 16] = to_rgbaf32([0.5, 0.5, 0.5, 1.0]);
        let vector: [u8; 16] = to_rgbaf32([0.5, 0.5, 1.0, 1.0]);

        let black_data: Vec<u8>;
        let white_data: Vec<u8>;
        let gray_data: Vec<u8>;
        let blue_gray_data: Vec<u8>;

        if bitmap_type == BitmapType::Cubemap {
            black_data = iter::repeat(null)
                .take(6)
                .flatten()
                .collect();
            white_data = iter::repeat(white)
                .take(6)
                .flatten()
                .collect();
            gray_data = iter::repeat(gray)
                .take(6)
                .flatten()
                .collect();
            blue_gray_data = iter::repeat(vector)
                .take(6)
                .flatten()
                .collect();
        }
        else {
            black_data = null.to_vec();
            white_data = white.to_vec();
            gray_data = gray.to_vec();
            blue_gray_data = vector.to_vec();
        }

        let add_data = AddBitmapParameter {
            bitmaps: vec![
                AddBitmapBitmapParameter {
                    format: BitmapFormat::R32G32B32A32SFloat,
                    bitmap_type,
                    resolution: Resolution { width: 1, height: 1 },
                    mipmap_count: 0,
                    data: black_data,
                },
                AddBitmapBitmapParameter {
                    format: BitmapFormat::R32G32B32A32SFloat,
                    bitmap_type,
                    resolution: Resolution { width: 1, height: 1 },
                    mipmap_count: 0,
                    data: white_data,
                },
                AddBitmapBitmapParameter {
                    format: BitmapFormat::R32G32B32A32SFloat,
                    bitmap_type,
                    resolution: Resolution { width: 1, height: 1 },
                    mipmap_count: 0,
                    data: gray_data,
                },
                AddBitmapBitmapParameter {
                    format: BitmapFormat::R32G32B32A32SFloat,
                    bitmap_type,
                    resolution: Resolution { width: 1, height: 1 },
                    mipmap_count: 0,
                    data: blue_gray_data,
                }
            ],
            sequences: vec![
                AddBitmapSequenceParameter::Bitmap { first: 0, count: 1 },
                AddBitmapSequenceParameter::Bitmap { first: 1, count: 1 },
                AddBitmapSequenceParameter::Bitmap { first: 2, count: 1 },
                AddBitmapSequenceParameter::Bitmap { first: 3, count: 1 },
            ],
        };

        use alloc::string::ToString;

        renderer.add_bitmap(path, add_data)?;
        Ok(renderer.bitmaps.get_key_value(&path.to_string()).unwrap().0.clone())
    }

    let default_2d = make_add_bitmap_parameter(renderer, "~default_2d", BitmapType::Dim2D)?;
    let default_3d = make_add_bitmap_parameter(renderer, "~default_3d", BitmapType::Dim3D { depth: 1 })?;
    let default_cubemap = make_add_bitmap_parameter(renderer, "~default_cubemap", BitmapType::Cubemap)?;

    renderer.default_bitmaps = DefaultBitmaps {
        default_2d,
        default_3d,
        default_cubemap
    };

    Ok(())
}
