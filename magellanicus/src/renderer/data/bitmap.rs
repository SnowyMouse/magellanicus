use alloc::vec::Vec;
use alloc::string::String;

use crate::renderer::AddBitmapParameter;
use crate::renderer::vulkan::VulkanBitmapData;

pub struct Bitmap {
    pub bitmaps: Vec<BitmapBitmap>,
    pub sequences: Vec<BitmapSequence>
}

impl Bitmap {
    pub fn load_from_parameters(parameter: AddBitmapParameter) -> Result<Self, String> {
        Self::verify_parameter(&parameter)?;

        Ok(Self {
            sequences: parameter.sequences,
            bitmaps: todo!()
        })
    }

    fn verify_parameter(parameter: &AddBitmapParameter) -> Result<(), String> {
        let invalid_bitmap_error = parameter.sequences
            .iter()
            .enumerate()
            .find_map(|(sequence_index, sequence)| {
                match sequence {
                    // Find invalid bitmap ranges
                    BitmapSequence::Bitmap { count, .. } if *count == 0 => None,
                    BitmapSequence::Bitmap { count, first } => first
                        .checked_add(*count - 1)
                        .and_then(|count| parameter.bitmaps.get(*first..=count))
                        .is_none()
                        .then(|| alloc::format!("Sequence {sequence_index} has an invalid range {first}..({first}+{count}); only {} bitmap(s)", parameter.bitmaps.len())),

                    // Find invalid sprite indices
                    BitmapSequence::Sprites { sprites } => sprites
                        .iter()
                        .enumerate()
                        .find_map(|(sprite_index, BitmapSprite { bitmap, .. })| parameter.bitmaps.get(*bitmap).is_none().then(|| {
                            alloc::format!("Sprite {sprite_index} of sequence {sequence_index} refers to bitmap {bitmap} which is not a valid index")
                        })),
                }
            });

        match invalid_bitmap_error {
            Some(n) => Err(n),
            None => Ok(())
        }
    }
}

pub struct BitmapBitmap {
    pub vulkan: VulkanBitmapData,
    pub resolution: (u16, u16)
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
