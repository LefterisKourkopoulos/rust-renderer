use crate::gfx::texture;

/// Everything that actually varies between this project's render pipelines.
/// Anything not listed here (blending, topology, winding, multisampling) is
/// fixed at the defaults below, so a new pass only spells out what differs.
pub struct RenderPipelineConfig<'a> {
    pub label: &'a str,
    pub bind_group_layouts: &'a [&'a wgpu::BindGroupLayout],
    pub shader: &'a wgpu::ShaderModule,
    pub vertex_buffers: &'a [wgpu::VertexBufferLayout<'a>],
    pub color_format: wgpu::TextureFormat,
    /// `true` for passes that render geometry against the depth buffer; `false`
    /// for fullscreen passes that only sample it.
    pub depth_write: bool,
}

pub fn create_render_pipeline(
    device: &wgpu::Device,
    config: &RenderPipelineConfig,
) -> wgpu::RenderPipeline {
    let bind_group_layouts = config
        .bind_group_layouts
        .iter()
        .map(|layout| Some(*layout))
        .collect::<Vec<_>>();

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{} Layout", config.label)),
        bind_group_layouts: &bind_group_layouts,
        immediate_size: 0,
    });

    let vertex_buffers = config
        .vertex_buffers
        .iter()
        .map(|buffer| Some(buffer.clone()))
        .collect::<Vec<_>>();

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(config.label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: config.shader,
            entry_point: Some("vs_main"),
            buffers: &vertex_buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: config.shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: config.depth_write.then(|| wgpu::DepthStencilState {
            format: texture::Texture::DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}
