use std::{format, println};
use std::num::NonZeroUsize;
use vulkano::image::{Image, ImageAspects, ImageCreateInfo, ImageSubresourceLayers, ImageTiling, ImageType, ImageUsage, SampleCount};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocatePreference, MemoryTypeFilter};
use crate::error::{Error, MResult};
use crate::renderer::{AddBitmapBitmapParameter, BitmapFormat, BitmapType, Renderer};
use crate::renderer::vulkan::{default_allocation_create_info, VulkanRenderer};
use std::vec::Vec;
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::DeviceSize;
use vulkano::format::Format;
use std::string::ToString;
use vulkano::command_buffer::allocator::CommandBufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, BufferImageCopy, CommandBufferUsage, CopyBufferToImageInfo, PrimaryCommandBufferAbstract};
use vulkano::sync::GpuFuture;
use crate::renderer::mipmap_iterator::{MipmapFaceIterator, MipmapTextureIterator, MipmapType};

pub struct VulkanBitmapData {
    pub image: Arc<Image>
}

impl VulkanBitmapData {
    pub fn new(vulkan_renderer: &mut VulkanRenderer, parameter: &AddBitmapBitmapParameter) -> MResult<Self> {
        let (image_type, depth) = match parameter.bitmap_type {
            BitmapType::Dim3D { depth } => (ImageType::Dim3d, depth),
            _ => (ImageType::Dim2d, 1)
        };

        let mut transcoded_pixels: Vec<u8> = Vec::new();

        let (format, bytes) = match parameter.format {
            BitmapFormat::DXT1 => (Format::BC1_RGBA_UNORM_BLOCK, &parameter.data),
            BitmapFormat::DXT3 => (Format::BC2_UNORM_BLOCK, &parameter.data),
            BitmapFormat::DXT5 => (Format::BC3_UNORM_BLOCK, &parameter.data),
            BitmapFormat::BC7 => (Format::BC7_UNORM_BLOCK, &parameter.data),

            // TODO: VERIFY
            BitmapFormat::A8R8G8B8 => (Format::B8G8R8A8_UNORM, &parameter.data),
            BitmapFormat::X8R8G8B8 => (Format::B8G8R8A8_UNORM, &parameter.data),
            BitmapFormat::R5G6B5 => (Format::R5G6B5_UNORM_PACK16, &parameter.data),
            BitmapFormat::A1R5G5B5 => (Format::A1R5G5B5_UNORM_PACK16, &parameter.data),
            BitmapFormat::A4R4G4B4 => (Format::A4R4G4B4_UNORM_PACK16, &parameter.data),

            // TODO: VERIFY ALL OF THE MONOCHROME MEMES

            BitmapFormat::A8 => {
                transcoded_pixels.reserve_exact(parameter.data.len() * 4);
                for pixel in parameter.data.iter() {
                    transcoded_pixels.push(0xFF);
                    transcoded_pixels.push(0xFF);
                    transcoded_pixels.push(0xFF);
                    transcoded_pixels.push(*pixel);
                }
                (Format::B8G8R8A8_UNORM, &parameter.data)
            },

            BitmapFormat::Y8 => {
                transcoded_pixels.reserve_exact(parameter.data.len() * 4);
                for pixel in parameter.data.iter() {
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(0xFF);
                }
                (Format::B8G8R8A8_UNORM, &parameter.data)
            },

            BitmapFormat::AY8 => {
                transcoded_pixels.reserve_exact(parameter.data.len() * 4);
                for pixel in parameter.data.iter() {
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(*pixel);
                }
                (Format::B8G8R8A8_UNORM, &parameter.data)
            },

            BitmapFormat::A8Y8 => {
                transcoded_pixels.reserve_exact(parameter.data.len() * 2);
                for p in parameter.data.chunks(2) {
                    let &[a, y] = p else {
                        unreachable!()
                    };
                    transcoded_pixels.push(y);
                    transcoded_pixels.push(y);
                    transcoded_pixels.push(y);
                    transcoded_pixels.push(a);
                }
                (Format::B8G8R8A8_UNORM, &parameter.data)
            },

            // TODO: P8
            BitmapFormat::P8 => {
                transcoded_pixels.reserve_exact(parameter.data.len() * 4);
                for pixel in parameter.data.iter() {
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(*pixel);
                    transcoded_pixels.push(0xFF);
                }
                (Format::B8G8R8A8_UNORM, &parameter.data)
            }
        };

        let image = Image::new(
            vulkan_renderer.memory_allocator.clone(),
            ImageCreateInfo {
                image_type,
                format,
                extent: [parameter.resolution.width, parameter.resolution.height, depth],
                mip_levels: parameter.mipmap_count + 1,
                array_layers: if parameter.bitmap_type == BitmapType::Cubemap { 6 } else { 1 },
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                allocate_preference: MemoryAllocatePreference::AlwaysAllocate,
                ..Default::default()
            },
        )?;

        let upload_buffer = Buffer::new_slice(
            vulkan_renderer.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            default_allocation_create_info(),
            bytes.len() as DeviceSize,
        )?;

        upload_buffer
            .write()
            .map_err(|e| Error::from_vulkan_error(e.to_string()))?
            .copy_from_slice(bytes);

        let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
            &vulkan_renderer.command_buffer_allocator,
            vulkan_renderer.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        let iterator = MipmapTextureIterator::new(
            NonZeroUsize::new(parameter.resolution.width as usize).unwrap(),
            NonZeroUsize::new(parameter.resolution.height as usize).unwrap(),
            match parameter.bitmap_type {
                BitmapType::Cubemap => MipmapType::Cubemap,
                BitmapType::Dim2D => MipmapType::TwoDimensional,
                BitmapType::Dim3D { depth } => MipmapType::ThreeDimensional(NonZeroUsize::new(depth as usize).unwrap())
            },
            NonZeroUsize::new(parameter.format.block_pixel_length()).unwrap(),
            Some(parameter.mipmap_count as usize),
        );

        let mut offset = 0;
        let block_size = parameter.format.block_byte_size();
        let pixel_size = parameter.format.block_pixel_length();
        for i in iterator {
            let size = block_size * i.block_count;
            command_buffer_builder.copy_buffer_to_image(CopyBufferToImageInfo {
                regions: [
                    BufferImageCopy {
                        image_subresource: ImageSubresourceLayers {
                            aspects: ImageAspects::COLOR,
                            mip_level: i.mipmap_index as u32,
                            array_layers: i.face_index as u32..(i.face_index as u32 + 1)
                        },
                        buffer_offset: offset,
                        buffer_image_height: (i.block_height * pixel_size) as u32,
                        buffer_row_length: (i.block_width * pixel_size) as u32,
                        image_offset: [0,0,0],
                        image_extent: [i.width as u32, i.height as u32, i.depth as u32],
                        ..Default::default()
                    }
                ].into(),
                ..CopyBufferToImageInfo::buffer_image(
                    upload_buffer.clone(),
                    image.clone()
                )
            })?;

            offset += size as DeviceSize;
        }



        let buffer = command_buffer_builder.build()?;
        vulkan_renderer.execute_command_list(buffer);

        Ok(Self { image })
    }
}
