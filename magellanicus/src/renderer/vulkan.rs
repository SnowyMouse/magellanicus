use alloc::string::{String, ToString};

mod bitmap;
mod geometry;
mod shader;
mod bsp;
mod sky;
mod helper;
mod player_viewport;
mod vertex;

use alloc::sync::Arc;
use alloc::format;
use core::fmt::{Debug, Display};
use std::println;
use raw_window_handle::{HasDisplayHandle, HasRawDisplayHandle, HasRawWindowHandle};
use vulkano::command_buffer::allocator::{CommandBufferAllocator, StandardCommandBufferAllocator};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::DeviceExtensions;
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::swapchain::Surface;
use vulkano::{ValidationError, Version, VulkanLibrary};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract};
use vulkano::sync::GpuFuture;
pub use bitmap::*;
pub use geometry::*;
pub use shader::*;
pub use bsp::*;
pub use sky::*;
pub use player_viewport::*;
use crate::error::{Error, MResult};
use crate::renderer::Resolution;

pub struct VulkanRenderer {
    current_resolution: Resolution,
    instance: Arc<Instance>,
    command_buffer_allocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
}

impl VulkanRenderer {
    pub fn new(renderer_parameters: &super::RendererParameters, surface: Arc<impl HasRawWindowHandle + HasRawDisplayHandle + Send + Sync + 'static>) -> MResult<Self> {
        let (instance, device, queue) = helper::load_vulkan_and_get_queue(surface)?;

        let command_buffer_allocator =
            StandardCommandBufferAllocator::new(device.clone(), Default::default());
        let descriptor_set_allocator =
            StandardDescriptorSetAllocator::new(device.clone(), Default::default());

        let solid_color_shader = solid_color::SolidColorShader::new(device.clone())?;

        unsafe { exit(134); }

        Ok(Self {
            current_resolution: renderer_parameters.resolution,
            instance,
            command_buffer_allocator,
            descriptor_set_allocator
        })
    }
}

extern "C" {
    fn exit(code: i32) -> !;
}

fn default_allocation_create_info() -> AllocationCreateInfo {
    AllocationCreateInfo {
        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
        ..Default::default()
    }
}

impl<T: Display> From<vulkano::Validated<T>> for Error {
    fn from(value: vulkano::Validated<T>) -> Self {
        match value {
            vulkano::Validated::ValidationError(v) => v.into(),
            vulkano::Validated::Error(e) => Self::from_vulkan_error(format!("Vulkan error! {e}"))
        }
    }
}

impl From<alloc::boxed::Box<ValidationError>> for Error {
    fn from(value: alloc::boxed::Box<ValidationError>) -> Self {
        // Self::from_vulkan_error(format!("Validation error! {value:?}"))
        panic!("Validation error! {value:?}\n\n-----------\n\nBACKTRACE:\n\n{}\n\n-----------\n\n", std::backtrace::Backtrace::force_capture())
    }
}

impl From<vulkano::LoadingError> for Error {
    fn from(value: vulkano::LoadingError) -> Self {
        Self::from_vulkan_error(format!("Loading error! {value:?}"))
    }
}

impl Error {
    fn from_vulkan_error(error: String) -> Self {
        Self::GraphicsAPIError { backend: "Vulkan", error }
    }
}
