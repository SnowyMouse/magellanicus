use alloc::string::{String, ToString};

mod bitmap;
mod geometry;
mod pipeline;
mod bsp;
mod sky;
mod helper;
mod player_viewport;
mod vertex;
mod material;

use std::sync::Arc;
use std::{format, vec};
use std::fmt::{Debug, Display};
use std::println;
use std::boxed::Box;
use std::collections::BTreeMap;
use std::vec::Vec;
use raw_window_handle::{HasDisplayHandle, HasRawDisplayHandle, HasRawWindowHandle};
use vulkano::command_buffer::allocator::{CommandBufferAllocator, StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo};
use vulkano::descriptor_set::allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo};
use vulkano::device::{Device, DeviceExtensions, Queue};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::swapchain::{Surface, Swapchain};
use vulkano::{ValidationError, Version, VulkanLibrary};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferInheritanceInfo, CommandBufferInheritanceRenderPassType, CommandBufferInheritanceRenderingInfo, CommandBufferUsage, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract, SecondaryAutoCommandBuffer};
use vulkano::format::Format;
use vulkano::image::Image;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::sync::GpuFuture;
pub use bitmap::*;
pub use geometry::*;
pub use pipeline::*;
pub use bsp::*;
pub use sky::*;
pub use material::*;
pub use player_viewport::*;
use crate::error::{Error, MResult};
use crate::renderer::Resolution;
use crate::renderer::vulkan::helper::{build_swapchain, LoadedVulkan};
use crate::renderer::vulkan::solid_color::SolidColorShader;

pub struct VulkanRenderer {
    current_resolution: Resolution,
    instance: Arc<Instance>,
    device: Arc<Device>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    queue: Arc<Queue>,
    future: Option<Box<dyn GpuFuture>>,
    pipelines: BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>>,
    output_format: Format,
    swapchain: Arc<Swapchain>,
    surface: Arc<Surface>,
    swapchain_images: Vec<Arc<Image>>
}

impl VulkanRenderer {
    pub fn new(
        renderer_parameters: &super::RendererParameters,
        surface: Arc<impl HasRawWindowHandle + HasRawDisplayHandle + Send + Sync + 'static>,
        resolution: Resolution
    ) -> MResult<Self> {
        let LoadedVulkan { device, instance, surface, queue} = helper::load_vulkan_and_get_queue(surface)?;

        let command_buffer_allocator = StandardCommandBufferAllocator::new(
            device.clone(),
            StandardCommandBufferAllocatorCreateInfo {
                primary_buffer_count: 32,
                secondary_buffer_count: 16 * 1024,
                ..Default::default()
            }
        );

        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
            device.clone(),
            StandardDescriptorSetAllocatorCreateInfo {
                set_count: 16 * 1024,
                ..Default::default()
            }
        );

        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let future = Some(vulkano::sync::now(device.clone()).boxed());

        let output_format = device
            .physical_device()
            .surface_formats(surface.as_ref(), Default::default())
            .unwrap()[0]
            .0;

        let (swapchain, swapchain_images) = build_swapchain(device.clone(), surface.clone(), output_format, resolution)?;

        let pipelines = load_all_pipelines(device.clone())?;

        Ok(Self {
            current_resolution: renderer_parameters.resolution,
            instance,
            command_buffer_allocator,
            descriptor_set_allocator,
            memory_allocator,
            device,
            queue,
            future,
            pipelines,
            output_format,
            swapchain,
            surface,
            swapchain_images
        })
    }

    fn execute_command_list(&mut self, command_buffer: Arc<impl PrimaryCommandBufferAbstract + 'static>) {
        let future = self.future
            .take()
            .expect("no future?")
            .then_execute(self.queue.clone(), command_buffer)
            .expect("failed to execute the command list")
            .boxed();
        self.future = Some(future)
    }

    fn generate_secondary_buffer_builder(&self) -> MResult<AutoCommandBufferBuilder<SecondaryAutoCommandBuffer>> {
        let result = AutoCommandBufferBuilder::secondary(
            &self.command_buffer_allocator,
            self.queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit,
            CommandBufferInheritanceInfo {
                render_pass: Some(CommandBufferInheritanceRenderPassType::BeginRendering(CommandBufferInheritanceRenderingInfo {
                    color_attachment_formats: vec![Some(self.output_format)],
                    depth_attachment_format: Some(Format::D16_UNORM),
                    ..CommandBufferInheritanceRenderingInfo::default()
                })),
                ..CommandBufferInheritanceInfo::default()
            }
        )?;
        Ok(result)
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

impl From<Box<ValidationError>> for Error {
    fn from(value: Box<ValidationError>) -> Self {
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
    fn from_vulkan_impl_error(error: String) -> Self {
        Self::GraphicsAPIError { backend: "Vulkan-IMPL", error }
    }
}
