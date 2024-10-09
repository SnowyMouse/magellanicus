use alloc::string::String;

mod bitmap;
mod geometry;
mod pipeline;
mod bsp;
mod sky;
mod helper;
mod player_viewport;
mod vertex;
mod material;

use crate::error::{Error, MResult};
use crate::renderer::data::BSP;
use crate::renderer::vulkan::helper::{build_swapchain, LoadedVulkan};
use crate::renderer::vulkan::vertex::{VulkanFogData, VulkanModelData, VulkanModelVertex};
use crate::renderer::{DefaultType, Renderer, RendererParameters, Resolution, MSAA};
pub use bitmap::*;
pub use bsp::*;
pub use geometry::*;
use glam::{Mat3, Mat4, Vec3};
pub use material::*;
pub use pipeline::*;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::borrow::ToOwned;
use std::boxed::Box;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::Arc;
use std::vec::Vec;
use std::{eprintln, format, vec};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo};
use vulkano::command_buffer::{AutoCommandBufferBuilder, BlitImageInfo, CommandBufferInheritanceInfo, CommandBufferInheritanceRenderPassType, CommandBufferInheritanceRenderingInfo, CommandBufferUsage, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract, RenderingAttachmentInfo, RenderingInfo, ResolveImageInfo, SecondaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::{Device, Queue};
use vulkano::format::Format;
use vulkano::image::sampler::{Sampler, SamplerCreateInfo};
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage, SampleCount};
use vulkano::instance::Instance;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::padded::Padded;
use vulkano::pipeline::graphics::rasterization::CullMode;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::{Pipeline, PipelineBindPoint};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::swapchain::{acquire_next_image, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainPresentInfo};
use vulkano::sync::GpuFuture;
use vulkano::{Validated, ValidationError, VulkanError};

pub(crate) static OFFLINE_PIPELINE_COLOR_FORMAT: Format = Format::R8G8B8A8_UNORM;

pub struct VulkanRenderer {
    current_resolution: Resolution,
    instance: Arc<Instance>,
    device: Arc<Device>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    queue: Arc<Queue>,
    future: Option<Box<dyn GpuFuture + Send + Sync>>,
    pipelines: BTreeMap<VulkanPipelineType, Arc<dyn VulkanPipelineData>>,
    swapchain: Arc<Swapchain>,
    surface: Arc<Surface>,
    swapchain_images: Vec<Arc<Image>>,
    swapchain_image_views: Vec<Arc<ImageView>>,
    default_2d_sampler: Arc<Sampler>,
    samples_per_pixel: SampleCount
}

impl VulkanRenderer {
    pub unsafe fn new(
        renderer_parameters: &RendererParameters,
        surface: &(impl HasRawWindowHandle + HasRawDisplayHandle)
    ) -> MResult<Self> {
        let LoadedVulkan { device, instance, surface, queue} = helper::load_vulkan_and_get_queue(surface)?;

        let samples_per_pixel = match renderer_parameters.msaa {
            MSAA::NoMSAA => SampleCount::Sample1,
            MSAA::MSAA2x => SampleCount::Sample2,
            MSAA::MSAA4x => SampleCount::Sample4,
            MSAA::MSAA8x => SampleCount::Sample8,
            MSAA::MSAA16x => SampleCount::Sample16
        };

        let color = device.physical_device().properties().sampled_image_color_sample_counts;
        let depth = device.physical_device().properties().sampled_image_depth_sample_counts;
        let intersection = color & depth;
        if !intersection.contains_enum(samples_per_pixel) {
            return Err(
                Error::from_vulkan_impl_error(format!("{}x MSAA is unsupported by your device; only these are supported:{}",
                                                      renderer_parameters.msaa as u32,
                                                      intersection.into_iter().map(|s| format!(" {}", s as u32)).collect::<String>())));
        }

        let command_buffer_allocator = StandardCommandBufferAllocator::new(
            device.clone(),
            StandardCommandBufferAllocatorCreateInfo {
                primary_buffer_count: 32,
                secondary_buffer_count: 0,
                ..Default::default()
            }
        );

        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
            device.clone(),
            StandardDescriptorSetAllocatorCreateInfo {
                set_count: 16 * 1024,
                ..Default::default()
            }
        ));

        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let future = Some(vulkano::sync::now(device.clone()).boxed_send_sync());

        let output_format = device
            .physical_device()
            .surface_formats(surface.as_ref(), Default::default())?[0]
            .0;

        let (swapchain, swapchain_images) = build_swapchain(device.clone(), surface.clone(), output_format, renderer_parameters)?;

        let pipelines = load_all_pipelines(device.clone(), samples_per_pixel)?;

        let swapchain_image_views = swapchain_images.iter().map(|v| {
            ImageView::new_default(v.clone()).unwrap()
        }).collect();

        let default_2d_sampler = Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear())?;

        Ok(Self {
            current_resolution: renderer_parameters.resolution,
            instance,
            command_buffer_allocator,
            descriptor_set_allocator,
            device,
            queue,
            future,
            pipelines,
            swapchain,
            surface,
            swapchain_image_views,
            memory_allocator,
            swapchain_images,
            default_2d_sampler,
            samples_per_pixel
        })
    }

    pub fn draw_frame(renderer: &mut Renderer) -> MResult<bool> {
        let vulkan_renderer = &mut renderer.renderer;

        let (image_index, suboptimal, acquire_future) =
            match acquire_next_image(vulkan_renderer.swapchain.clone(), None).map_err(Validated::unwrap) {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => return Ok(false),
                Err(e) => panic!("failed to acquire next image: {e}"),
            };

        Ok(Self::draw_frame_infallible(renderer, image_index, acquire_future) && !suboptimal)
    }

    pub fn rebuild_swapchain(&mut self, renderer_parameters: &RendererParameters) -> MResult<()> {
        let (swapchain, swapchain_images) = self.swapchain.recreate(
            SwapchainCreateInfo {
                image_extent: [renderer_parameters.resolution.width, renderer_parameters.resolution.height],
                ..self.swapchain.create_info()
            }
        )?;

        self.swapchain = swapchain;
        self.swapchain_images = swapchain_images;
        self.swapchain_image_views = self.swapchain_images.iter().map(|i| ImageView::new_default(i.clone()).unwrap()).collect();
        self.current_resolution = renderer_parameters.resolution;

        Ok(())
    }

    fn draw_frame_infallible(renderer: &mut Renderer, image_index: u32, image_future: SwapchainAcquireFuture) -> bool {
        let default_bsp = BSP::default();
        let currently_loaded_bsp = renderer
            .current_bsp
            .as_ref()
            .and_then(|f| renderer.bsps.get(f))
            .unwrap_or(&default_bsp);

        let mut command_builder = AutoCommandBufferBuilder::primary(
            &renderer.renderer.command_buffer_allocator,
            renderer.renderer.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit
        ).expect("failed to init command builder");

        let final_color_view = renderer.renderer.swapchain_image_views[image_index as usize].clone();

        let color_view = ImageView::new_default(Image::new(
            renderer.renderer.memory_allocator.clone(),
            ImageCreateInfo {
                extent: final_color_view.image().extent(),
                format: OFFLINE_PIPELINE_COLOR_FORMAT,
                image_type: ImageType::Dim2d,
                samples: renderer.renderer.samples_per_pixel,
                usage: ImageUsage::TRANSFER_SRC | ImageUsage::COLOR_ATTACHMENT,
                ..Default::default()
            },
            AllocationCreateInfo::default()
        ).unwrap()).unwrap();

        let depth_image = Image::new(
            renderer.renderer.memory_allocator.clone(),
            ImageCreateInfo {
                extent: final_color_view.image().extent(),
                format: Format::D32_SFLOAT,
                image_type: ImageType::Dim2d,
                samples: color_view.image().samples(),
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                ..Default::default()
            },
            AllocationCreateInfo::default()
        ).unwrap();
        let depth_view = ImageView::new_default(depth_image).unwrap();

        // Clear this real quick
        command_builder.begin_rendering(RenderingInfo {
            color_attachments: vec![Some(RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some([0.0, 0.0, 0.0, 1.0].into()),
                ..RenderingAttachmentInfo::image_view(color_view.clone())
            })],
            depth_attachment: Some(RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some([1.0].into()),
                ..RenderingAttachmentInfo::image_view(depth_view)
            }),
            ..Default::default()
        }).expect("failed to begin rendering");

        let (width, height) = (renderer.renderer.current_resolution.width as f32, renderer.renderer.current_resolution.height as f32);

        for i in &renderer.player_viewports {
            let cluster_index = currently_loaded_bsp.bsp_data.find_cluster(i.camera.position);
            let cluster = cluster_index.map(|c| &currently_loaded_bsp.bsp_data.clusters[c]);
            let sky = cluster.and_then(|c| c.sky.as_ref()).and_then(|s| renderer.skies.get(s));

            let mut fog_data;

            if let Some(sky) = sky {
                // TODO: determine which fog color
                fog_data = FogData {
                    color: [sky.outdoor_fog_color[0], sky.outdoor_fog_color[1], sky.outdoor_fog_color[2], 0.0],
                    distance_from: sky.outdoor_fog_start_distance,
                    distance_to: sky.outdoor_fog_opaque_distance,
                    min_opacity: 0.0,
                    max_opacity: sky.outdoor_fog_maximum_density,
                }
            }
            else {
                fog_data = FogData::default()
            }

            if !i.camera.fog {
                fog_data = FogData::default();
            }

            let viewport = Viewport {
                offset: [i.rel_x * width, i.rel_y * height],
                extent: [i.rel_width * width, i.rel_height * height],
                depth_range: 0.0..=1.0,
            };

            let z_near = 0.0625;
            let mut z_far = currently_loaded_bsp.draw_distance;

            // Occlude things that won't be visible anyway
            if fog_data.max_opacity == 1.0 {
                z_far = z_far.min(fog_data.distance_to);
            }

            z_far = z_far.max(z_near + 1.0);
            let proj = Mat4::perspective_lh(
                i.camera.fov,
                viewport.extent[0] / viewport.extent[1],
                z_near,
                z_far
            );
            let view = Mat4::look_to_lh(
                i.camera.position.into(),
                i.camera.rotation.into(),
                Vec3::new(0.0, 0.0, -1.0)
            );

            command_builder.set_viewport(0, [viewport].into_iter().collect()).unwrap();

            draw_box(
                renderer,
                0.0,
                0.0,
                1.0,
                1.0,
                [fog_data.color[0], fog_data.color[1], fog_data.color[2], 1.0],
                &mut command_builder
            ).unwrap();

            upload_fog_uniform(renderer, &fog_data, &mut command_builder);

            let geo_shader_iterator = currently_loaded_bsp
                .geometry_indices_sorted_by_material
                .iter()
                .map(|g| &currently_loaded_bsp.geometries[*g])
                .map(|g| (g, &renderer.shaders.get(&g.vulkan.shader).expect("no shader?").vulkan.pipeline_data));

            let opaque = geo_shader_iterator.clone().filter(|s| !s.1.is_transparent());

            #[allow(unused_variables)]
            let non_opaque = geo_shader_iterator.clone().filter(|s| s.1.is_transparent());

            upload_main_material_uniform(renderer, i.camera.position.into(), Vec3::default(), Mat3::IDENTITY, view, proj, &mut command_builder);
            command_builder.set_cull_mode(CullMode::Back).unwrap();

            // Draw non-transparent shaders first
            let mut last_shader = None;
            for (geometry, shader) in opaque {
                let this_shader = &geometry.vulkan.shader;
                let repeat_shader = if last_shader != Some(this_shader) {
                    last_shader = Some(this_shader);
                    false
                }
                else {
                    true
                };

                let mut desired_lightmap = geometry.lightmap_index;
                if !i.camera.lightmaps {
                    desired_lightmap = None;
                }

                upload_lightmap_descriptor_set(renderer, desired_lightmap, &mut command_builder);

                let index_buffer = geometry.vulkan.index_buffer.clone();
                let index_count = index_buffer.len() as usize;
                command_builder.bind_index_buffer(index_buffer).expect("can't bind indices");

                command_builder.bind_vertex_buffers(0, (
                    geometry.vulkan.vertex_buffer.clone(),
                    geometry.vulkan.texture_coords_buffer.clone(),
                    if geometry.vulkan.lightmap_texture_coords_buffer.is_none() {
                        geometry.vulkan.texture_coords_buffer.clone()
                    }
                    else {
                        geometry.vulkan.lightmap_texture_coords_buffer.clone().unwrap()
                    }
                )).unwrap();

                shader
                    .generate_commands(renderer, index_count as u32, repeat_shader, &mut command_builder)
                    .expect("can't generate stage commands");
            }
        }

        Self::draw_split_screen_bars(renderer, &mut command_builder, width, height);

        command_builder.end_rendering().expect("failed to end rendering");

        let staging_view = if color_view.image().samples() != SampleCount::Sample1 {
            let resolved_color_view = ImageView::new_default(Image::new(
                renderer.renderer.memory_allocator.clone(),
                ImageCreateInfo {
                    extent: color_view.image().extent(),
                    format: color_view.image().format(),
                    image_type: color_view.image().image_type(),
                    usage: ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST | ImageUsage::COLOR_ATTACHMENT,
                    ..Default::default()
                },
                AllocationCreateInfo::default()
            ).unwrap())
                .unwrap();

            command_builder.resolve_image(ResolveImageInfo::images(color_view.image().clone(), resolved_color_view.image().clone()))
                .expect("resolve fail");

            resolved_color_view
        }
        else {
            color_view
        };

        command_builder.blit_image(BlitImageInfo::images(staging_view.image().clone(), final_color_view.image().clone())).unwrap();

        let commands = command_builder.build().expect("failed to build command builder");

        let mut future = renderer.renderer
            .future
            .take()
            .expect("there's no future :(");

        future.cleanup_finished();

        let swapchain_present = SwapchainPresentInfo::swapchain_image_index(renderer.renderer.swapchain.clone(), image_index);

        match future
            .join(image_future)
            .then_execute(renderer.renderer.queue.clone(), commands)
            .expect("can't execute commands")
            .then_swapchain_present(renderer.renderer.queue.clone(), swapchain_present)
            .then_signal_fence_and_flush() {
            Ok(n) => {
                renderer.renderer.future = Some(n.boxed_send_sync());
                true
            }
            Err(Validated::Error(VulkanError::OutOfDate)) => {
                renderer.renderer.future = Some(vulkano::sync::now(renderer.renderer.device.clone()).boxed_send_sync());
                false
            }
            Err(e) => {
                panic!("Oh, shit! Some bullshit just happened: {e}")
            }
        }
    }

    fn draw_split_screen_bars(renderer: &mut Renderer, command_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, width: f32, height: f32) {
        if renderer.player_viewports.len() <= 1 {
            return;
        }

        let color = [0.0, 0.0, 0.0, 1.0];
        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [width, height],
            depth_range: 0.0..=1.0,
        };
        command_builder.set_viewport(0, [viewport].into_iter().collect()).unwrap();

        let base_thickness = 2.0;
        let scale = (width / 640.0).min(height / 480.0).max(1.0);
        let line_thickness_horizontal = base_thickness / height * scale;
        let line_thickness_vertical = base_thickness / width * scale;

        draw_box(renderer, 0.0, 0.5 - line_thickness_horizontal / 2.0, 1.0, line_thickness_horizontal, color, command_builder)
            .expect("can't draw split screen vertical bar");

        if renderer.player_viewports.len() > 2 {
            let y;
            let line_height;

            if renderer.player_viewports.len() == 3 {
                y = 0.5;
                line_height = 0.5;
            } else {
                y = 0.0;
                line_height = 1.0;
            }

            draw_box(renderer, 0.5 - line_thickness_vertical / 2.0, y, line_thickness_vertical, line_height, color, command_builder)
                .expect("can't draw split screen horizontal bar");
        }
    }

    fn execute_command_list(&mut self, command_buffer: Arc<impl PrimaryCommandBufferAbstract + 'static>) {
        let execution = command_buffer.execute(self.queue.clone()).unwrap();

        let future = self.future
            .take()
            .expect("no future?")
            .join(execution)
            .then_signal_fence_and_flush()
            .expect("failed to signal/flush")
            .boxed_send_sync();

        self.future = Some(future)
    }

    fn generate_secondary_buffer_builder(&self) -> MResult<AutoCommandBufferBuilder<SecondaryAutoCommandBuffer>> {
        let result = AutoCommandBufferBuilder::secondary(
            &self.command_buffer_allocator,
            self.queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit,
            CommandBufferInheritanceInfo {
                render_pass: Some(CommandBufferInheritanceRenderPassType::BeginRendering(CommandBufferInheritanceRenderingInfo {
                    color_attachment_formats: vec![Some(OFFLINE_PIPELINE_COLOR_FORMAT)],
                    depth_attachment_format: Some(Format::D32_SFLOAT),
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

impl<T: Display> From<Validated<T>> for Error {
    fn from(value: Validated<T>) -> Self {
        match value {
            Validated::ValidationError(v) => v.into(),
            Validated::Error(e) => Self::from_vulkan_error(format!("Vulkan error! {e}"))
        }
    }
}

impl From<Box<ValidationError>> for Error {
    fn from(value: Box<ValidationError>) -> Self {
        // FIXME: figure out a more graceful way to do this
        eprintln!("Validation error! {value:?}\n\n-----------\n\nBACKTRACE:\n\n{}\n\n-----------\n\n", std::backtrace::Backtrace::force_capture());
        std::process::abort();
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

fn upload_lightmap_descriptor_set(
    renderer: &Renderer,
    lightmap_index: Option<usize>,
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>
) {
    let pipeline = renderer.renderer.pipelines[&VulkanPipelineType::ShaderEnvironment].get_pipeline();
    let sampler = renderer
        .current_bsp
        .as_ref()
        .and_then(|b| renderer.bsps.get(b))
        .and_then(|b| Some((b, lightmap_index?)))
        .and_then(|(b, i)| b.vulkan.lightmap_images.get(&i))
        .map(|b| b.to_owned())
        .unwrap_or_else(|| {
            let image = ImageView::new_default(renderer.get_default_2d(DefaultType::White).vulkan.image.clone()).unwrap();
            (image, renderer.renderer.default_2d_sampler.clone())
        });

    let set = PersistentDescriptorSet::new(
        renderer.renderer.descriptor_set_allocator.as_ref(),
        pipeline.layout().set_layouts()[1].clone(),
        [
            WriteDescriptorSet::sampler(0, sampler.1),
            WriteDescriptorSet::image_view(1, sampler.0),
        ],
        []
    ).unwrap();

    builder.bind_descriptor_sets(
        PipelineBindPoint::Graphics,
        pipeline.layout().clone(),
        1,
        set
    ).unwrap();
}

struct FogData {
    color: [f32; 4],
    distance_from: f32,
    distance_to: f32,
    min_opacity: f32,
    max_opacity: f32
}

impl Default for FogData {
    fn default() -> Self {
        Self {
            color: [0.0f32; 4],
            distance_from: 0.0,
            distance_to: 1.0,
            min_opacity: 0.0,
            max_opacity: 0.0
        }
    }
}

fn upload_main_material_uniform(
    renderer: &Renderer,
    camera: Vec3,
    offset: Vec3,
    rotation: Mat3,
    view: Mat4,
    proj: Mat4,
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>
) {
    let pipeline = renderer.renderer.pipelines[&VulkanPipelineType::ShaderEnvironment].get_pipeline();
    let model = Mat4::IDENTITY;

    let model_data = VulkanModelData {
        camera: Padded::from(camera.to_array()),
        world: model.to_cols_array_2d(),
        view: view.to_cols_array_2d(),
        proj: proj.to_cols_array_2d(),
        offset: Padded::from(offset.to_array()),
        rotation: [
            Padded::from(rotation.x_axis.to_array()),
            Padded::from(rotation.y_axis.to_array()),
            Padded::from(rotation.z_axis.to_array())
        ],
    };

    let model_uniform_buffer = Buffer::from_data(
        renderer.renderer.memory_allocator.clone(),
        BufferCreateInfo { usage: BufferUsage::UNIFORM_BUFFER, ..Default::default() },
        default_allocation_create_info(),
        model_data
    ).unwrap();

    let set = PersistentDescriptorSet::new(
        renderer.renderer.descriptor_set_allocator.as_ref(),
        pipeline.layout().set_layouts()[0].clone(),
        [
            WriteDescriptorSet::buffer(0, model_uniform_buffer),
        ],
        []
    ).unwrap();

    builder.bind_descriptor_sets(
        PipelineBindPoint::Graphics,
        pipeline.layout().clone(),
        0,
        set
    ).unwrap();
}

fn upload_fog_uniform(
    renderer: &Renderer,
    fog: &FogData,
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>
) {
    let pipeline = renderer.renderer.pipelines[&VulkanPipelineType::ShaderEnvironment].get_pipeline();

    let fog_data = VulkanFogData {
        sky_fog_to: fog.distance_to,
        sky_fog_from: fog.distance_from,
        sky_fog_min_opacity: fog.min_opacity,
        sky_fog_max_opacity: fog.max_opacity,
        sky_fog_color: fog.color
    };

    let fog_uniform_buffer = Buffer::from_data(
        renderer.renderer.memory_allocator.clone(),
        BufferCreateInfo { usage: BufferUsage::UNIFORM_BUFFER, ..Default::default() },
        default_allocation_create_info(),
        fog_data
    ).unwrap();

    let set = PersistentDescriptorSet::new(
        renderer.renderer.descriptor_set_allocator.as_ref(),
        pipeline.layout().set_layouts()[2].clone(),
        [
            WriteDescriptorSet::buffer(0, fog_uniform_buffer),
        ],
        []
    ).unwrap();

    builder.bind_descriptor_sets(
        PipelineBindPoint::Graphics,
        pipeline.layout().clone(),
        2,
        set
    ).unwrap();
}

fn draw_box(renderer: &Renderer, x: f32, y: f32, width: f32, height: f32, color: [f32; 4], command_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> MResult<()> {
    let indices = Buffer::from_iter(
        renderer.renderer.memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::INDEX_BUFFER,
            ..Default::default()
        },
        default_allocation_create_info(),
        [0u16,1,2,0,2,3]
    )?;
    let vertices = Buffer::from_iter(
        renderer.renderer.memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::VERTEX_BUFFER,
            ..Default::default()
        },
        default_allocation_create_info(),
        [
            VulkanModelVertex {
                position: [x, y, 0.5],
                normal: [1.0, 0.0, 0.0],
                binormal: [1.0, 0.0, 0.0],
                tangent: [1.0, 0.0, 0.0]
            },
            VulkanModelVertex {
                position: [x, y + height, 0.5],
                normal: [1.0, 0.0, 0.0],
                binormal: [1.0, 0.0, 0.0],
                tangent: [1.0, 0.0, 0.0]
            },
            VulkanModelVertex {
                position: [x + width, y + height, 0.5],
                normal: [1.0, 0.0, 0.0],
                binormal: [1.0, 0.0, 0.0],
                tangent: [1.0, 0.0, 0.0]
            },
            VulkanModelVertex {
                position: [x + width, y, 0.5],
                normal: [1.0, 0.0, 0.0],
                binormal: [1.0, 0.0, 0.0],
                tangent: [1.0, 0.0, 0.0]
            }
        ]
    )?;

    let pipeline = renderer
        .renderer
        .pipelines[&VulkanPipelineType::ColorBox]
        .get_pipeline();

    let uniform_buffer = Buffer::from_data(
        renderer.renderer.memory_allocator.clone(),
        BufferCreateInfo { usage: BufferUsage::UNIFORM_BUFFER, ..Default::default() },
        default_allocation_create_info(),
        color
    ).unwrap();

    let set = PersistentDescriptorSet::new(
        renderer.renderer.descriptor_set_allocator.as_ref(),
        pipeline.layout().set_layouts()[1].clone(),
        [
            WriteDescriptorSet::buffer(0, uniform_buffer),
        ],
        []
    ).unwrap();

    command_builder.bind_descriptor_sets(
        PipelineBindPoint::Graphics,
        pipeline.layout().clone(),
        1,
        set
    ).unwrap();

    command_builder.set_cull_mode(CullMode::None).unwrap();
    command_builder.bind_index_buffer(indices).unwrap();
    command_builder.bind_vertex_buffers(0, vertices).unwrap();
    command_builder.bind_pipeline_graphics(pipeline).unwrap();
    command_builder.draw_indexed(6, 1, 0, 0, 0).unwrap();

    Ok(())
}
