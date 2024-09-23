use alloc::string::{String, ToString};
use alloc::sync::Arc;
use std::println;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo, QueueFlags};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::Surface;
use vulkano::{Validated, Version, VulkanError, VulkanLibrary};
use crate::error::{Error, MResult};

pub fn load_vulkan_and_get_queue(surface: Arc<impl HasRawWindowHandle + HasRawDisplayHandle + Send + Sync + 'static>) -> MResult<(Arc<Instance>, Arc<Device>, Arc<Queue>)> {
    let library = VulkanLibrary::new()?;

    let mut enabled_extensions = Surface::required_extensions(surface.as_ref());
    let mut device_extensions_13 = DeviceExtensions::empty();
    device_extensions_13.khr_swapchain = true;

    let device_extensions_12 = DeviceExtensions {
        khr_dynamic_rendering: true,
        ..device_extensions_13
    }.clone();

    let instance = Instance::new(library.clone(), InstanceCreateInfo {
        enabled_extensions,
        ..Default::default()
    })?;

    let mut surface_ref = Surface::from_window(instance.clone(), surface.clone())?;

    let (physical_device, queue_family_index, device_extensions) = find_best_gpu(
        instance.clone(),
        device_extensions_12,
        device_extensions_13,
        surface_ref.clone()
    ).ok_or_else(|| Error::from_vulkan_error("No suitable Vulkan-compatible GPUs found".to_string()))?;

    let (device, mut queues) = create_device_and_queues(
        physical_device,
        device_extensions,
        queue_family_index
    )?;
    let queue = queues.next().ok_or_else(|| Error::from_vulkan_error("Unable to make a device queue".to_string()))?;

    Ok((instance, device, queue))
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
                ..Features::default()
            },
            ..Default::default()
        },
    )
}

fn find_best_gpu(
    instance: Arc<Instance>,
    device_extensions_12: DeviceExtensions,
    device_extensions_13: DeviceExtensions,
    surface: Arc<Surface>
) -> Option<(Arc<PhysicalDevice>, u32, DeviceExtensions)> {
    instance
        .enumerate_physical_devices()
        .unwrap()
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
