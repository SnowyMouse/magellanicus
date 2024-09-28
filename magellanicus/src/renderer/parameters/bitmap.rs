mod mipmap_iterator;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use alloc::borrow::ToOwned;
use core::fmt::Display;
use core::num::NonZeroUsize;
use crate::error::{Error, MResult};
use crate::renderer::parameters::bitmap::mipmap_iterator::{MipmapFaceIterator, MipmapType};
use crate::renderer::Resolution;

pub struct AddBitmapParameter {
    pub bitmaps: Vec<AddBitmapBitmapParameter>,
    pub sequences: Vec<AddBitmapSequenceParameter>
}

impl AddBitmapParameter {
    pub(crate) fn validate(&self) -> MResult<()> {
        let invalid_sequence_error = self.sequences
            .iter()
            .enumerate()
            .find_map(|(sequence_index, sequence)| {
                match sequence {
                    // Find invalid bitmap ranges
                    AddBitmapSequenceParameter::Bitmap { count, .. } if *count == 0 => None,
                    AddBitmapSequenceParameter::Bitmap { count, first } => first
                        .checked_add(*count - 1)
                        .and_then(|count| self.bitmaps.get(*first..=count))
                        .is_none()
                        .then(|| format!("Sequence {sequence_index} has an invalid range {first}..({first}+{count}); only {} bitmap(s)", self.bitmaps.len())),

                    // Find invalid sprite indices
                    AddBitmapSequenceParameter::Sprites { sprites } => sprites
                        .iter()
                        .enumerate()
                        .find_map(|(sprite_index, BitmapSprite { bitmap, .. })| self.bitmaps.get(*bitmap).is_none().then(|| {
                            format!("Sprite {sprite_index} of sequence {sequence_index} refers to bitmap {bitmap} which is not a valid index")
                        })),
                }
            });

        if let Some(error) = invalid_sequence_error {
            return Err(Error::DataError { error })
        }

        let invalid_bitmap_error = self.bitmaps
            .iter()
            .enumerate()
            .find_map(|(bitmap_index, bitmap)| {
                let Resolution { width, height } = bitmap.resolution;
                let reported_mipmap_count = bitmap.mipmap_count;

                let (Some(width_nz), Some(height_nz)) = (NonZeroUsize::new(width as usize), NonZeroUsize::new(height as usize)) else {
                    return Some(format!("Bitmap #{bitmap_index} has 0 on one or more dimensions ({width}x{height})"))
                };
                if bitmap.data.is_empty() {
                    return Some(format!("Bitmap #{bitmap_index} has no pixel data"))
                }

                // Block length
                let block_length = NonZeroUsize::new(bitmap.format.block_pixel_length()).unwrap();

                // Get mipmap type
                let mipmap_type = match bitmap.bitmap_type {
                    BitmapType::Dim2D => MipmapType::TwoDimensional,
                    BitmapType::Dim3D { depth } => match NonZeroUsize::new(depth as usize) {
                        Some(n) => MipmapType::ThreeDimensional(n),
                        None => return Some(format!("Bitmap #{bitmap_index} has a depth of 0"))
                    },
                    BitmapType::Cubemap => MipmapType::Cubemap
                };

                let highest_dimension = width.max(height).max(match bitmap.bitmap_type { BitmapType::Dim3D { depth } => depth, _ => 1 });
                let log_of_highest_dim = highest_dimension.ilog2();
                let highest_possible_mipmap_count = if highest_dimension == (1 << log_of_highest_dim) {
                    log_of_highest_dim - 0
                }
                else {
                    log_of_highest_dim + 1
                };

                if reported_mipmap_count > highest_possible_mipmap_count {
                    return Some(format!("Bitmap #{bitmap_index} ({width}x{height}) reports a mipmap count of {reported_mipmap_count}, but the highest mipmap count possible is {highest_possible_mipmap_count}"))
                }

                let mipmaps = MipmapFaceIterator::new(
                    width_nz, height_nz, mipmap_type, block_length, Some(reported_mipmap_count as usize)
                );

                let bytes_per_block = bitmap.format.block_byte_size();
                let Some((block_count, bytes_count)) = mipmaps
                    .map(|b| (b.block_count as u64, b.block_count as u64 * bytes_per_block as u64))
                    .reduce(|a, b| (a.0 + b.0, a.1 + b.1))
                    .and_then(|(a,b)| Some((usize::try_from(a).ok()?, usize::try_from(b).ok()?)))
                else {
                    return Some(format!("Bitmap #{bitmap_index} can't get block count"))
                };

                let actual_length = bitmap.data.len();
                if bytes_count != actual_length {
                    return Some(format!("Bitmap #{bitmap_index} ({width}x{height}) has an incorrect number of bytes (expected {bytes_count} ({block_count} * {bytes_per_block}), got {actual_length})"))
                }

                None
            });

        if let Some(error) = invalid_bitmap_error {
            return Err(Error::DataError { error })
        }

        Ok(())
    }
}

pub use super::super::data::BitmapSequence as AddBitmapSequenceParameter;
pub use super::super::data::BitmapSprite;
pub use super::super::data::BitmapType;

pub struct AddBitmapBitmapParameter {
    pub format: BitmapFormat,
    pub bitmap_type: BitmapType,
    pub resolution: Resolution,
    pub mipmap_count: u32,
    pub data: Vec<u8>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BitmapFormat {
    DXT1,
    DXT3,
    DXT5,
    BC7,
    A8R8G8B8,
    X8R8G8B8,
    R5G6B5,
    A1R5G5B5,
    A4R4G4B4,

    A8,
    Y8,
    AY8,
    A8Y8,
    P8
}
impl BitmapFormat {
    pub fn block_pixel_length(self) -> usize {
        match self {
            Self::DXT1 => 4,
            Self::DXT3 => 4,
            Self::DXT5 => 4,
            Self::BC7 => 4,
            Self::A8R8G8B8 => 1,
            Self::X8R8G8B8 => 1,
            Self::R5G6B5 => 1,
            Self::A1R5G5B5 => 1,
            Self::A4R4G4B4 => 1,
            Self::A8 => 1,
            Self::Y8 => 1,
            Self::AY8 => 1,
            Self::A8Y8 => 1,
            Self::P8 => 1,
        }
    }
    pub fn block_byte_size(self) -> usize {
        match self {
            Self::DXT1 => 8,
            Self::DXT3 => 16,
            Self::DXT5 => 16,
            Self::BC7 => 16,
            Self::A8R8G8B8 => 4,
            Self::X8R8G8B8 => 4,
            Self::R5G6B5 => 2,
            Self::A1R5G5B5 => 2,
            Self::A4R4G4B4 => 2,
            Self::A8 => 1,
            Self::Y8 => 1,
            Self::AY8 => 1,
            Self::A8Y8 => 2,
            Self::P8 => 1,
        }
    }
}
