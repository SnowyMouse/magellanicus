use vulkano::device::Device;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexBufferDescription, VertexDefinition};
use vulkano::pipeline::{DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use std::sync::Arc;
use std::{println, vec};
use std::vec::Vec;
use vulkano::format::Format;
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::{CullMode, FrontFace, RasterizationState};
use vulkano::pipeline::graphics::subpass::PipelineRenderingCreateInfo;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use crate::error::MResult;
use crate::renderer::vulkan::vertex::*;

#[derive(Copy, Clone, Default)]
pub struct PipelineSettings {
    pub vertex_inputs: &'static [VertexPipelineInput],
    pub writes_depth: bool
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum VertexPipelineInput {
    Position,
    TextureCoordinate,
    Normal,
    Binormal,
    Tangent,
    LightmapTextureCoordinate,
}

pub fn load_pipeline(
    device: Arc<Device>,
    load_vertex_shader: fn (Arc<Device>) -> Result<Arc<vulkano::shader::ShaderModule>, vulkano::Validated<vulkano::VulkanError>>,
    load_fragment_shader: fn (Arc<Device>) -> Result<Arc<vulkano::shader::ShaderModule>, vulkano::Validated<vulkano::VulkanError>>,
    settings: &PipelineSettings
) -> MResult<Arc<GraphicsPipeline>> {
    if settings.vertex_inputs.is_empty() {
        panic!("Vertex inputs is empty!")
    }
    for i in settings.vertex_inputs.iter().enumerate() {
        for j in settings.vertex_inputs.iter().enumerate() {
            if i.0 == j.0 {
                continue
            }
            if i.1 == j.1 {
                panic!("Duplicate vertex inputs!")
            }
        }
    }
    if !settings.vertex_inputs.contains(&VertexPipelineInput::Position) {
        panic!("No vertex positions!")
    }

    let vertex_shader = load_vertex_shader(device.clone())?
        .entry_point("main")
        .expect("Missing main() entry point for vertex shader!");
    let fragment_shader = load_fragment_shader(device.clone())?
        .entry_point("main")
        .expect("Missing main() entry point for fragment shader!");

    let inputs = settings.vertex_inputs.iter().map(|m| match m {
        VertexPipelineInput::Position => VulkanModelVertexPosition::per_vertex(),
        VertexPipelineInput::Normal => VulkanModelVertexNormal::per_vertex(),
        VertexPipelineInput::Binormal => VulkanModelVertexBinormal::per_vertex(),
        VertexPipelineInput::Tangent => VulkanModelVertexTangent::per_vertex(),
        VertexPipelineInput::TextureCoordinate => VulkanModelVertexTextureCoords::per_vertex(),
        VertexPipelineInput::LightmapTextureCoordinate => VulkanModelVertexLightmapTextureCoords::per_vertex(),
    }).collect::<Vec<VertexBufferDescription>>();

    let vertex_input_state = inputs.definition(&vertex_shader.info().input_interface)?;
    let stages = [
        PipelineShaderStageCreateInfo::new(vertex_shader),
        PipelineShaderStageCreateInfo::new(fragment_shader),
    ];

    let layout = PipelineLayout::new(
        device.clone(),
        PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
            .into_pipeline_layout_create_info(device.clone())
            .unwrap(),
    )?;

    let subpass = PipelineRenderingCreateInfo {
        color_attachment_formats: vec![Some(Format::R8G8B8A8_UNORM)],
        depth_attachment_format: Some(Format::D16_UNORM),
        ..Default::default()
    };

    let pipeline = GraphicsPipeline::new(
        device.clone(),
        None,
        GraphicsPipelineCreateInfo {
            stages: stages.into_iter().collect(),
            vertex_input_state: Some(vertex_input_state),
            input_assembly_state: Some(InputAssemblyState::default()),
            viewport_state: Some(ViewportState::default()),
            rasterization_state: Some(RasterizationState {
                // FIXME: backface culling
                // cull_mode: CullMode::Back,
                // front_face: FrontFace::Clockwise,
                ..RasterizationState::default()
            }),
            multisample_state: Some(MultisampleState::default()),
            color_blend_state: Some(ColorBlendState::with_attachment_states(
                subpass.color_attachment_formats.len() as u32,
                ColorBlendAttachmentState::default(),
            )),
            dynamic_state: [DynamicState::Viewport].into_iter().collect(),
            depth_stencil_state: Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: settings.writes_depth,
                    compare_op: CompareOp::LessOrEqual
                }),
                ..DepthStencilState::default()
            }),
            subpass: Some(subpass.into()),
            ..GraphicsPipelineCreateInfo::layout(layout)
        }
    )?;

    Ok(pipeline)
}
