use crate::error::{Error, MResult};
use crate::renderer::RendererParameters;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::string::ToString;
use std::sync::Arc;
use std::vec::Vec;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo, QueueFlags};
use vulkano::format::Format;
use vulkano::image::{Image, ImageUsage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::swapchain::{PresentMode, Surface, Swapchain, SwapchainCreateInfo};
use vulkano::{Validated, Version, VulkanError, VulkanLibrary};

pub struct LoadedVulkan {
    pub instance: Arc<Instance>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub surface: Arc<Surface>,
}

pub fn load_vulkan_and_get_queue(surface: Arc<impl HasRawWindowHandle + HasRawDisplayHandle + Send + Sync + 'static>) -> MResult<LoadedVulkan> {
    let library = VulkanLibrary::new()?;

    let enabled_extensions = Surface::required_extensions(surface.as_ref());
    let device_extensions_13 = DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::empty()
    };

    let device_extensions_12 = DeviceExtensions {
        khr_dynamic_rendering: true,
        ext_4444_formats: true,
        ext_extended_dynamic_state: true,
        ..device_extensions_13
    }.clone();

    let required_device_features = Features {
        ..Features::empty()
    };

    let instance = Instance::new(library.clone(), InstanceCreateInfo {
        enabled_extensions,
        ..Default::default()
    })?;

    let surface = Surface::from_window(instance.clone(), surface.clone())?;

    let (physical_device, queue_family_index, device_extensions) = find_best_gpu(
        instance.clone(),
        device_extensions_12,
        device_extensions_13,
        required_device_features,
        surface.clone()
    ).ok_or_else(|| Error::from_vulkan_error("No suitable Vulkan-compatible GPUs found".to_string()))?;

    let (device, mut queues) = create_device_and_queues(
        physical_device,
        device_extensions,
        queue_family_index
    )?;
    let queue = queues.next().ok_or_else(|| Error::from_vulkan_error("Unable to make a device queue".to_string()))?;

    Ok(LoadedVulkan { instance, device, queue, surface })
}

fn create_device_and_queues(physical_device: Arc<PhysicalDevice>, device_extensions: DeviceExtensions, queue_family_index: u32) -> Result<(Arc<Device>, impl ExactSizeIterator<Item=Arc<Queue>> + Sized), Validated<VulkanError>> {
    Device::new(
        physical_device,
        DeviceCreateInfo {
            enabled_extensions: device_extensions,
            queue_create_infos: alloc::vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_features: Features {
                dynamic_rendering: true,
                extended_dynamic_state: true,
                ..Features::default()
            },
            ..Default::default()
        },
    )
}

pub fn build_swapchain(device: Arc<Device>, surface: Arc<Surface>, image_format: Format, renderer_parameters: &RendererParameters) -> MResult<(Arc<Swapchain>, Vec<Arc<Image>>)> {
    let surface_capabilities = device
        .physical_device()
        .surface_capabilities(surface.as_ref(), Default::default())
        .unwrap();

    let result = Swapchain::new(
        device.clone(),
        surface,
        SwapchainCreateInfo {
            min_image_count: surface_capabilities.min_image_count.max(2),
            image_format,
            image_extent: [renderer_parameters.resolution.width, renderer_parameters.resolution.height],
            image_usage: ImageUsage::COLOR_ATTACHMENT,
            present_mode: if renderer_parameters.vsync {
                // This is guaranteed to be supported as per the Vulkan standard.
                PresentMode::Fifo
            } else {
                // This should be supported, but it is not technically required.
                PresentMode::Immediate
            },

            // The alpha mode indicates how the alpha value of the final image will behave. For
            // example, you can choose whether the window will be opaque or transparent.
            composite_alpha: surface_capabilities
                .supported_composite_alpha
                .into_iter()
                .next()
                .unwrap(),

            ..Default::default()
        },
    )?;

    Ok(result)
}

fn find_best_gpu(
    instance: Arc<Instance>,
    device_extensions_12: DeviceExtensions,
    device_extensions_13: DeviceExtensions,
    required_device_features: Features,
    surface: Arc<Surface>
) -> Option<(Arc<PhysicalDevice>, u32, DeviceExtensions)> {
    instance
        .enumerate_physical_devices()
        .unwrap()
        .filter(|device| device.supported_features().contains(&required_device_features))
        .filter_map(|device| {
            if device.api_version() >= Version::V1_3 {
                if device.supported_extensions().contains(&device_extensions_13) {
                    Some((device, device_extensions_13))
                }
                else {
                    None
                }
            }
            else if device.api_version() >= Version::V1_2 {
                if device.supported_extensions().contains(&device_extensions_12) {
                    Some((device, device_extensions_12))
                }
                else {
                    None
                }
            }
            else {
                None
            }
        })
        .filter_map(|(device, extensions)| {
            device.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.intersects(QueueFlags::GRAPHICS) && (device.surface_support(i as u32, surface.as_ref()).unwrap_or(false))
                })
                .map(|i| (device, i as u32, extensions))
        })
        .min_by_key(|(p, ..)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            _ => u32::MAX,
        })
}
