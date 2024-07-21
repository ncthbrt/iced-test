use bevy::{prelude::*, render::camera::RenderTarget};
use iced::widget::shader::{
    wgpu::{
        core::command::{PassChannel, RenderPassColorAttachment, RenderPassDescriptor},
        hal::TextureDescriptor,
        util::DeviceExt,
        TextureView,
    },
    Primitive,
};

struct BevyComponent {
    app: Option<bevy::app::App>,
    input: Option<std::sync::mpsc::Sender<iced::widget::shader::Event>>,
    output: Option<std::sync::mpsc::Receiver<TextureView>>,
}

#[derive(Default)]
struct BevyComponentState {
    app: Option<bevy::app::App>,
    input: Option<std::sync::mpsc::Sender<iced::widget::shader::Event>>,
    output: Option<std::sync::mpsc::Receiver<Image>>,
}

#[derive(Debug)]
struct BevyPrimitive {
    texture_view: TextureView,
}

mod pipeline {
    use crate::wgpu;
    use crate::wgpu::util::DeviceExt;

    use iced::{
        widget::shader::wgpu::{RenderPass, Texture},
        Rectangle, Size,
    };
    use wgpu::util::RenderEncoder;

    pub struct Pipeline {
        pipeline: wgpu::RenderPipeline,
        uniform_bind_group: wgpu::BindGroup,
    }

    impl Pipeline {
        pub fn new(
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
            target_size: Size<u32>,
        ) -> Self {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("blit_shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "./blit.wgsl"
                ))),
            });

            let blit_texture: Texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("blit texture"),
                    size: wgpu::Extent2d {
                        width: target_size,
                        height: target_size,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &skybox_data,
            );

            let blit_view = blit_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("blit texture view"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                ..Default::default()
            });

            let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("blit sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });

            let uniform_bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("blit uniform bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

            let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("blit uniform bind group"),
                layout: &uniform_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&blit_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&blit_sampler),
                    },
                ],
            });

            let layout: wgpu::PipelineLayout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("blit pipeline layout"),
                    bind_group_layouts: &[&uniform_bind_group_layout],
                    push_constant_ranges: &[],
                });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("blit_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Max,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview: None,
            });

            Self {
                pipeline,
                uniform_bind_group,
            }
        }

        pub fn render(
            &self,
            target: &wgpu::TextureView,
            encoder: &mut wgpu::CommandEncoder,
            viewport: Rectangle<u32>,
        ) {
            {
                let mut pass: RenderPass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("blit.pipeline.pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_scissor_rect(viewport.x, viewport.y, viewport.width, viewport.height);
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.uniform_bind_group, &[0]);
                pass.draw(0..3, 0..1);
            }
        }
    }
}

impl Primitive for BevyPrimitive {
    fn prepare(
        &self,
        device: &iced::widget::shader::wgpu::Device,
        queue: &iced::widget::shader::wgpu::Queue,
        format: iced::widget::shader::wgpu::TextureFormat,
        storage: &mut iced::widget::shader::Storage,
        bounds: &iced::Rectangle,
        viewport: &iced::widget::shader::Viewport,
    ) {
        if !storage.has::<pipeline::Pipeline>() {
            storage.store(pipeline::Pipeline::new(
                device,
                queue,
                format,
                viewport.physical_size(),
            ));
        }

        let pipeline = storage.get_mut::<pipeline::Pipeline>().unwrap();
    }

    fn render(
        &self,
        encoder: &mut iced::widget::shader::wgpu::CommandEncoder,
        storage: &iced::widget::shader::Storage,
        target: &iced::widget::shader::wgpu::TextureView,
        clip_bounds: &iced::Rectangle<u32>,
    ) {
        // At this point our pipeline should always be initialized
        let pipeline = storage.get::<pipeline::Pipeline>().unwrap();

        // Render primitive
        pipeline.render(target, encoder, *clip_bounds);
    }
}

impl<Message> iced::widget::shader::Program<Message> for BevyComponent {
    type State = BevyComponentState;

    type Primitive = BevyPrimitive;

    fn draw(
        &self,
        state: &Self::State,
        cursor: iced::advanced::mouse::Cursor,
        bounds: iced::Rectangle,
    ) -> Self::Primitive {
        BevyPrimitive {
            texture_view: self.output.take().unwrap().recv(),
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: iced::widget::shader::Event,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
        shell: &mut iced::advanced::Shell<'_, Message>,
    ) -> (
        iced::advanced::graphics::core::event::Status,
        Option<Message>,
    ) {
        match event {
            iced::widget::shader::Event::RedrawRequested(_) => {
                if let Some(mut app) = state.app.take() {
                    app.update();
                    state.app.replace(app);
                };

                shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                (
                    iced::advanced::graphics::core::event::Status::Captured,
                    None,
                )
            }
            iced::widget::shader::Event::Mouse(_) => (
                iced::advanced::graphics::core::event::Status::Captured,
                None,
            ),
            iced::widget::shader::Event::Touch(_) => (
                iced::advanced::graphics::core::event::Status::Captured,
                None,
            ),
            iced::widget::shader::Event::Keyboard(_) => (
                iced::advanced::graphics::core::event::Status::Captured,
                None,
            ),
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> iced::advanced::mouse::Interaction {
        iced::advanced::mouse::Interaction::Pointer
    }
}
