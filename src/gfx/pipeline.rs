use crate::gfx::texture;

pub struct RenderPipelineConfig<'a> {
    pub label: &'a str,
    pub bind_group_layouts: &'a [&'a wgpu::BindGroupLayout],
    pub shader: &'a wgpu::ShaderModule,
    pub vertex_buffers: &'a [wgpu::VertexBufferLayout<'a>],
    pub color_format: Option<wgpu::TextureFormat>,
    pub depth_write: bool,
    pub topology: wgpu::PrimitiveTopology,
    pub cull_mode: Option<wgpu::Face>,
    pub depth_bias: wgpu::DepthBiasState,
}

impl<'a> RenderPipelineConfig<'a> {
    pub fn new(
        label: &'a str,
        bind_group_layouts: &'a [&'a wgpu::BindGroupLayout],
        shader: &'a wgpu::ShaderModule,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            label,
            bind_group_layouts,
            shader,
            vertex_buffers: &[],
            color_format: Some(color_format),
            depth_write: true,
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            depth_bias: wgpu::DepthBiasState::default(),
        }
    }

    pub fn vertex_buffers(mut self, buffers: &'a [wgpu::VertexBufferLayout<'a>]) -> Self {
        self.vertex_buffers = buffers;
        self
    }

    pub fn depth_write(mut self, depth_write: bool) -> Self {
        self.depth_write = depth_write;
        self
    }
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

    let targets = config.color_format.map(|format| {
        [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        })]
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(config.label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: config.shader,
            entry_point: Some("vs_main"),
            buffers: &vertex_buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: targets.as_ref().map(|targets| wgpu::FragmentState {
            module: config.shader,
            entry_point: Some("fs_main"),
            targets,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: config.topology,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: config.cull_mode,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: config.depth_write.then(|| wgpu::DepthStencilState {
            format: texture::Texture::DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: config.depth_bias,
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
