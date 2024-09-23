use alloc::vec::Vec;

pub struct AddBitmapParameter {
    pub bitmaps: Vec<AddBitmapBitmapParameter>,
    pub sequences: Vec<AddBitmapSequenceParameter>
}

pub use super::super::data::BitmapSequence as AddBitmapSequenceParameter;

pub struct AddBitmapBitmapParameter {
    pub format: BitmapFormat,
    pub resolution: (u16, u16),
    pub mipmap_count: u8,
    pub data: Vec<u8>,
}

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
