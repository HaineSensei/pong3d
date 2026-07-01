use std::sync::Arc;

use glam::Vec3;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::game::{self, Game, Winner};
use crate::geometry::{self, Vertex3D, VertexHud};
use crate::hud;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad: f32,
}

pub struct Graphics {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,

    scene_pipeline_opaque: wgpu::RenderPipeline,
    scene_pipeline_translucent: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,

    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    room_vbuf: wgpu::Buffer,
    room_vcount: u32,

    ball_vbuf: wgpu::Buffer,
    ball_vcount: u32,

    player_bat_vbuf: wgpu::Buffer,
    ai_bat_vbuf: wgpu::Buffer,

    hud_vbuf: wgpu::Buffer,
    hud_capacity: u64,
}

const HUD_MAX_VERTS: u64 = 4096;

impl Graphics {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("pong3d device"),
                ..Default::default()
            })
            .await
            .expect("failed to create device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let depth_view = create_depth_view(&device, config.width, config.height);

        // --- uniform buffer + bind group ---
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform bg"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // --- scene pipeline (room, ball, paddles) ---
        let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/scene.wgsl").into()),
        });
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&uniform_bind_group_layout)],
            immediate_size: 0,
        });

        let make_scene_pipeline = |label: &str, blend: Option<wgpu::BlendState>, depth_write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState {
                    module: &scene_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex3D::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &scene_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let scene_pipeline_opaque = make_scene_pipeline("scene opaque", None, true);
        let scene_pipeline_translucent =
            make_scene_pipeline("scene translucent", Some(wgpu::BlendState::ALPHA_BLENDING), false);

        // --- HUD pipeline (orthographic overlay, no depth test) ---
        let hud_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/hud.wgsl").into()),
        });
        let hud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hud pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud pipeline"),
            layout: Some(&hud_layout),
            vertex: wgpu::VertexState {
                module: &hud_shader,
                entry_point: Some("vs_main"),
                buffers: &[VertexHud::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // --- static room mesh ---
        let room_verts = geometry::room_mesh(game::HALF_W, game::HALF_H, -2.0, game::DEPTH + 2.0);
        let room_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("room vbuf"),
            contents: bytemuck::cast_slice(&room_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let room_vcount = room_verts.len() as u32;

        // --- dynamic meshes (rewritten every frame) ---
        let ball_verts = geometry::sphere_mesh(
            game::BALL_RADIUS,
            Vec3::ZERO,
            10,
            16,
            [1.0, 0.82, 0.35, 1.0],
        );
        let ball_vcount = ball_verts.len() as u32;
        let ball_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ball vbuf"),
            contents: bytemuck::cast_slice(&ball_verts),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let player_bat_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("player bat vbuf"),
            size: (6 * std::mem::size_of::<Vertex3D>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ai_bat_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ai bat vbuf"),
            size: (6 * std::mem::size_of::<Vertex3D>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let hud_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud vbuf"),
            size: HUD_MAX_VERTS * std::mem::size_of::<VertexHud>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            surface,
            device,
            queue,
            config,
            depth_view,
            scene_pipeline_opaque,
            scene_pipeline_translucent,
            hud_pipeline,
            uniform_buffer,
            uniform_bind_group,
            room_vbuf,
            room_vcount,
            ball_vbuf,
            ball_vcount,
            player_bat_vbuf,
            ai_bat_vbuf,
            hud_vbuf,
            hud_capacity: HUD_MAX_VERTS,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, width, height);
    }

    /// Renders one frame. Returns `false` if the caller should skip
    /// requesting another redraw right away (e.g. window occluded/minimized).
    pub fn render(&mut self, game: &Game, flash: Option<[f32; 4]>) -> bool {
        let acquired = self.surface.get_current_texture();
        let frame = match acquired {
            wgpu::CurrentSurfaceTexture::Success(tex) => tex,
            wgpu::CurrentSurfaceTexture::Suboptimal(tex) => {
                // Still usable this frame; reconfigure before the next one.
                self.surface.configure(&self.device, &self.config);
                tex
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return true;
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return true;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                eprintln!("surface validation error acquiring frame");
                return true;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let width = self.config.width as f32;
        let height = self.config.height as f32;
        let aspect = width / height.max(1.0);

        // --- camera: embedded at the player's paddle, looking down +Z ---
        let camera_z = game::PADDLE_Z_PLAYER - 0.9;
        let eye = Vec3::new(game.player_pos.x, game.player_pos.y, camera_z);
        let view_mat = glam::camera::rh::view::look_to_mat4(eye, Vec3::Z, Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective(70f32.to_radians(), aspect, 0.05, 100.0);
        let view_proj = proj * view_mat;

        let uniforms = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            light_dir: [0.35, 0.85, 0.4],
            _pad: 0.0,
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // --- rebuild dynamic meshes at current world-space positions ---
        let ball_verts = geometry::sphere_mesh(
            game::BALL_RADIUS,
            game.ball_pos,
            10,
            16,
            [1.0, 0.82, 0.35, 1.0],
        );
        self.queue
            .write_buffer(&self.ball_vbuf, 0, bytemuck::cast_slice(&ball_verts));

        let player_bat = geometry::paddle_mesh(
            game::PADDLE_HALF,
            Vec3::new(game.player_pos.x, game.player_pos.y, game::PADDLE_Z_PLAYER),
            true,
            [0.6, 0.85, 1.0, 0.32],
        );
        self.queue
            .write_buffer(&self.player_bat_vbuf, 0, bytemuck::cast_slice(&player_bat));

        let ai_bat = geometry::paddle_mesh(
            game::PADDLE_HALF,
            Vec3::new(game.ai_pos.x, game.ai_pos.y, game::PADDLE_Z_AI),
            false,
            [1.0, 0.5, 0.5, 1.0],
        );
        self.queue
            .write_buffer(&self.ai_bat_vbuf, 0, bytemuck::cast_slice(&ai_bat));

        // --- HUD ---
        let mut hud_verts: Vec<VertexHud> = Vec::new();
        hud::push_score(&mut hud_verts, game.player_score, width / 2.0 - 90.0, 24.0, width, height, [0.85, 0.9, 1.0, 1.0]);
        hud::push_score(&mut hud_verts, game.ai_score, width / 2.0 + 64.0, 24.0, width, height, [1.0, 0.75, 0.75, 1.0]);
        hud::push_crosshair(&mut hud_verts, width, height, [1.0, 1.0, 1.0, 0.55]);
        if let Some(color) = flash {
            hud::push_full_screen_tint(&mut hud_verts, width, height, color);
        }
        if (hud_verts.len() as u64) > self.hud_capacity {
            hud_verts.truncate(self.hud_capacity as usize);
        }
        self.queue
            .write_buffer(&self.hud_vbuf, 0, bytemuck::cast_slice(&hud_verts));
        let hud_vcount = hud_verts.len() as u32;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.035,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // opaque geometry first (depth write on)
            pass.set_pipeline(&self.scene_pipeline_opaque);
            pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            pass.set_vertex_buffer(0, self.room_vbuf.slice(..));
            pass.draw(0..self.room_vcount, 0..1);

            pass.set_vertex_buffer(0, self.ball_vbuf.slice(..));
            pass.draw(0..self.ball_vcount, 0..1);

            pass.set_vertex_buffer(0, self.ai_bat_vbuf.slice(..));
            pass.draw(0..6, 0..1);

            // translucent player bat last (depth write off, alpha blend)
            pass.set_pipeline(&self.scene_pipeline_translucent);
            pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, self.player_bat_vbuf.slice(..));
            pass.draw(0..6, 0..1);

            // HUD overlay on top, no depth test
            if hud_vcount > 0 {
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_vertex_buffer(0, self.hud_vbuf.slice(..));
                pass.draw(0..hud_vcount, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        true
    }
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Picks a flash tint for the win/loss screen effect, if the game has ended.
pub fn winner_flash(winner: Option<Winner>, pulse: f32) -> Option<[f32; 4]> {
    winner.map(|w| match w {
        Winner::Player => [0.15, 0.9, 0.35, 0.12 + 0.08 * pulse],
        Winner::Ai => [0.9, 0.15, 0.15, 0.12 + 0.08 * pulse],
    })
}
