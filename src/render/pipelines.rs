use wgpu::*;

pub fn create_depth_stencil_state(depth_write_enabled: bool) -> DepthStencilState {
    DepthStencilState {
        format: TextureFormat::Depth24PlusStencil8,
        depth_write_enabled,
        depth_compare: CompareFunction::Less,
        stencil: StencilState::default(),
        bias: DepthBiasState::default(),
    }
}

pub fn create_primitive_state(cull_mode: Option<Face>) -> PrimitiveState {
    PrimitiveState {
        topology: PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: FrontFace::Cw,
        cull_mode,
        polygon_mode: PolygonMode::Fill,
        unclipped_depth: false,
        conservative: false,
    }
}

pub fn create_multisample_state() -> MultisampleState {
    MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    }
}

pub fn create_color_target_state(surface_format: TextureFormat) -> ColorTargetState {
    ColorTargetState {
        format: surface_format,
        blend: Some(BlendState::ALPHA_BLENDING),
        write_mask: ColorWrites::ALL,
    }
}
