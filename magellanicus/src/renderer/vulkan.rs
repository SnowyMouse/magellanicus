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
use std::{eprintln, format, vec};
use std::fmt::{Debug, Display};
use std::println;
use std::boxed::Box;
use std::collections::BTreeMap;
use std::time::Instant;
use std::vec::Vec;
use glam::{Mat3, Mat4, Vec3};
use raw_window_handle::{HasDisplayHandle, HasRawDisplayHandle, HasRawWindowHandle};
use vulkano::command_buffer::allocator::{CommandBufferAllocator, StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo};
use vulkano::descriptor_set::allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo};
use vulkano::device::{Device, DeviceExtensions, Queue};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::swapchain::{acquire_next_image, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainPresentInfo};
use vulkano::{Validated, ValidationError, Version, VulkanError, VulkanLibrary};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, ClearColorImageInfo, CommandBufferInheritanceInfo, CommandBufferInheritanceRenderPassType, CommandBufferInheritanceRenderingInfo, CommandBufferUsage, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract, RenderingAttachmentInfo, RenderingInfo, SecondaryAutoCommandBuffer, SubpassContents};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::format::Format;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage};
use vulkano::image::view::ImageView;
use vulkano::padded::Padded;
use vulkano::pipeline::graphics::rasterization::CullMode;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;
pub use bitmap::*;
pub use geometry::*;
pub use pipeline::*;
pub use bsp::*;
pub use sky::*;
pub use material::*;
pub use player_viewport::*;
use crate::error::{Error, MResult};
use crate::renderer::{Renderer, RendererParameters, Resolution};
use crate::renderer::data::BSP;
use crate::renderer::vulkan::helper::{build_swapchain, LoadedVulkan};
use crate::renderer::vulkan::vertex::{VulkanModelData, VulkanModelVertex};

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
    output_format: Format,
    swapchain: Arc<Swapchain>,
    surface: Arc<Surface>,
    swapchain_images: Vec<Arc<Image>>,
    swapchain_image_views: Vec<Arc<ImageView>>,
}

impl VulkanRenderer {
    pub fn new(
        renderer_parameters: &RendererParameters,
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
            .surface_formats(surface.as_ref(), Default::default())
            .unwrap()[0]
            .0;

        let (swapchain, swapchain_images) = build_swapchain(device.clone(), surface.clone(), output_format, renderer_parameters)?;

        let pipelines = load_all_pipelines(device.clone(), output_format)?;

        let swapchain_image_views = swapchain_images.iter().map(|v| {
            ImageView::new_default(v.clone()).unwrap()
        }).collect();

        Ok(Self {
            current_resolution: renderer_parameters.resolution,
            instance,
            command_buffer_allocator,
            descriptor_set_allocator,
            device,
            queue,
            future,
            pipelines,
            output_format,
            swapchain,
            surface,
            swapchain_image_views,
            memory_allocator,
            swapchain_images
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

        Ok(!suboptimal && Self::draw_frame_infallible(renderer, image_index, acquire_future))
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

        let (color_view) = renderer.renderer.swapchain_image_views[image_index as usize].clone();

        let depth_image = Image::new(
            renderer.renderer.memory_allocator.clone(),
            ImageCreateInfo {
                extent: color_view.image().extent(),
                format: Format::D16_UNORM,
                image_type: ImageType::Dim2d,
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
                ..RenderingAttachmentInfo::image_view(color_view)
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
            let viewport = Viewport {
                offset: [i.rel_x * width, i.rel_y * height],
                extent: [i.rel_width * width, i.rel_height * height],
                depth_range: 0.0..=1.0,
            };
            let proj = Mat4::perspective_lh(
                i.camera.fov,
                viewport.extent[0] / viewport.extent[1],
                0.05,
                1000.0
            );
            let view = Mat4::look_to_lh(
                i.camera.position.into(),
                i.camera.rotation.into(),
                Vec3::new(0.0, 0.0, -1.0)
            );

            command_builder.set_viewport(0, [viewport].into_iter().collect());
            command_builder.set_cull_mode(CullMode::None).unwrap();

            let cluster_index = currently_loaded_bsp.bsp_data.find_cluster(i.camera.position);
            let cluster = cluster_index.map(|c| &currently_loaded_bsp.bsp_data.clusters[c]);
            let sky = cluster.and_then(|c| c.sky.as_ref()).and_then(|s| renderer.skies.get(s));

            if let Some(sky) = sky {
                // TODO: determine which fog color
                draw_box(
                    renderer,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                    [sky.outdoor_fog_color[0], sky.outdoor_fog_color[1], sky.outdoor_fog_color[2], 1.0],
                    &mut command_builder
                );
            };

            upload_mvp_data(renderer, Vec3::default(), Mat3::IDENTITY, view, proj, &mut command_builder);

            for geometry in &currently_loaded_bsp.geometries {
                let shader = renderer.shaders.get(&geometry.vulkan.shader).expect("no shader?");
                let vulkan_shader = &shader.vulkan;
                let stages = vulkan_shader.pipeline_data.get_stages();

                command_builder.bind_index_buffer(geometry.vulkan.index_buffer.clone()).expect("can't bind indices");
                let mut currently_bound_thing = None;

                for (index, stage) in stages.iter().enumerate() {
                    let tcoords_type = Some(vulkan_shader.pipeline_data.get_texture_coords_type(renderer, index));
                    if tcoords_type != currently_bound_thing {
                        command_builder.bind_vertex_buffers(0, (
                            geometry.vulkan.vertex_buffer.clone(),
                            match tcoords_type.unwrap() {
                                VulkanMaterialTextureCoordsType::Model => {
                                    geometry.vulkan.texture_coords_buffer.clone()
                                },
                                VulkanMaterialTextureCoordsType::Lightmaps => {
                                    if geometry.vulkan.lightmap_texture_coords_buffer.is_none() {
                                        continue
                                    }
                                    geometry.vulkan.lightmap_texture_coords_buffer.clone().unwrap()
                                }
                            }
                        ));
                    }

                    command_builder.set_cull_mode(CullMode::Back).unwrap();

                    vulkan_shader
                        .pipeline_data
                        .generate_stage_commands(renderer, index, &mut command_builder)
                        .expect("can't generate stage commands");

                    command_builder
                        .draw_indexed(geometry.vulkan.index_buffer.len() as u32, 1, 0, 0, 0)
                        .expect("can't draw");
                }
            }
        }

        Self::draw_split_screen_bars(renderer, &mut command_builder, width, height);

        command_builder.end_rendering().expect("failed to end rendering");

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

    fn draw_split_screen_bars(renderer: &mut Renderer, mut command_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, width: f32, height: f32) {
        if renderer.player_viewports.len() <= 1 {
            return;
        }

        let color = [0.0, 0.0, 0.0, 1.0];
        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [width, height],
            depth_range: 0.0..=1.0,
        };
        command_builder.set_viewport(0, [viewport].into_iter().collect());

        let base_thickness = 2.0;
        let scale = (width / 640.0).min(height / 480.0).max(1.0);
        let line_thickness_horizontal = base_thickness / height * scale;
        let line_thickness_vertical = base_thickness / width * scale;

        draw_box(renderer, 0.0, 0.5 - line_thickness_horizontal / 2.0, 1.0, line_thickness_horizontal, color, &mut command_builder)
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

            draw_box(renderer, 0.5 - line_thickness_vertical / 2.0, y, line_thickness_vertical, line_height, color, &mut command_builder)
                .expect("can't draw split screen horizontal bar");
        }
    }

    fn execute_command_list(&mut self, command_buffer: Arc<impl PrimaryCommandBufferAbstract + 'static>) {
        let execution = command_buffer.execute(self.queue.clone()).unwrap();

        let future = self.future
            .take()
            .expect("no future?")
            .join(execution)
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

fn upload_mvp_data(renderer: &Renderer, offset: Vec3, rotation: Mat3, view: Mat4, proj: Mat4, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
    let pipeline = renderer.renderer.pipelines[&VulkanPipelineType::SolidColor].get_pipeline();
    let model = Mat4::IDENTITY;
    let model_data = VulkanModelData {
        world: model.to_cols_array_2d(),
        view: view.to_cols_array_2d(),
        proj: proj.to_cols_array_2d(),
        offset: Padded::from(offset.to_array()),
        rotation: [
            Padded::from(rotation.x_axis.to_array()),
            Padded::from(rotation.y_axis.to_array()),
            Padded::from(rotation.z_axis.to_array())
        ]
    };
    let uniform_buffer = Buffer::from_data(
        renderer.renderer.memory_allocator.clone(),
        BufferCreateInfo { usage: BufferUsage::UNIFORM_BUFFER, ..Default::default() },
        default_allocation_create_info(),
        model_data
    ).unwrap();
    let set = PersistentDescriptorSet::new(
        renderer.renderer.descriptor_set_allocator.as_ref(),
        pipeline.layout().set_layouts()[0].clone(),
        [
            WriteDescriptorSet::buffer(0, uniform_buffer),
        ],
        []
    ).unwrap();
    builder.bind_descriptor_sets(
        PipelineBindPoint::Graphics,
        pipeline.layout().clone(),
        0,
        set
    );
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
    );

    command_builder.set_cull_mode(CullMode::None);
    command_builder.bind_index_buffer(indices);
    command_builder.bind_vertex_buffers(0, vertices);
    command_builder.bind_pipeline_graphics(pipeline);
    command_builder.draw_indexed(6, 1, 0, 0, 0).unwrap();

    Ok(())
}
