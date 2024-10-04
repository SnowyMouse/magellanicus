#![no_std]
#![allow(dead_code)]

// crate `std` is needed for the Vulkano crate (and thus the vulkan module), but nothing else
extern crate std;
extern crate alloc;

pub mod vertex;
pub mod error;
pub mod renderer;
