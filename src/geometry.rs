use glam::Vec3;

/// Vertex used by the 3D scene pipeline (room walls, ball, paddles).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

impl Vertex3D {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem::size_of;
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex3D>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: (size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Vertex used by the orthographic HUD overlay pipeline. Positions are
/// supplied directly in clip space (NDC), so no camera/projection matrix
/// is needed for the HUD pass.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexHud {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl VertexHud {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem::size_of;
        wgpu::VertexBufferLayout {
            array_stride: size_of::<VertexHud>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Emits a flat quad (two triangles, 6 vertices) with an explicit normal
/// and per-vertex color. Corners should be given in order (a,b,c,d)
/// walking around the quad's perimeter.
pub fn push_quad(
    out: &mut Vec<Vertex3D>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    d: Vec3,
    normal: Vec3,
    color: [f32; 4],
) {
    let n = normal.to_array();
    for p in [a, b, c, a, c, d] {
        out.push(Vertex3D {
            position: p.to_array(),
            normal: n,
            color,
        });
    }
}

/// Builds the four walls (left, right, floor, ceiling) of an open-ended
/// cuboid room. The room has no wall at the near (player) or far (AI) end
/// -- those are the "goal planes" the ball can fly through to score.
pub fn room_mesh(half_w: f32, half_h: f32, z_min: f32, z_max: f32) -> Vec<Vertex3D> {
    let mut v = Vec::with_capacity(24);

    // Left wall (x = -half_w), inward normal +X.
    push_quad(
        &mut v,
        Vec3::new(-half_w, -half_h, z_min),
        Vec3::new(-half_w, -half_h, z_max),
        Vec3::new(-half_w, half_h, z_max),
        Vec3::new(-half_w, half_h, z_min),
        Vec3::new(1.0, 0.0, 0.0),
        [0.20, 0.30, 0.50, 1.0],
    );

    // Right wall (x = +half_w), inward normal -X.
    push_quad(
        &mut v,
        Vec3::new(half_w, -half_h, z_max),
        Vec3::new(half_w, -half_h, z_min),
        Vec3::new(half_w, half_h, z_min),
        Vec3::new(half_w, half_h, z_max),
        Vec3::new(-1.0, 0.0, 0.0),
        [0.20, 0.30, 0.50, 1.0],
    );

    // Floor (y = -half_h), inward normal +Y.
    push_quad(
        &mut v,
        Vec3::new(-half_w, -half_h, z_min),
        Vec3::new(half_w, -half_h, z_min),
        Vec3::new(half_w, -half_h, z_max),
        Vec3::new(-half_w, -half_h, z_max),
        Vec3::new(0.0, 1.0, 0.0),
        [0.18, 0.24, 0.20, 1.0],
    );

    // Ceiling (y = +half_h), inward normal -Y.
    push_quad(
        &mut v,
        Vec3::new(-half_w, half_h, z_max),
        Vec3::new(half_w, half_h, z_max),
        Vec3::new(half_w, half_h, z_min),
        Vec3::new(-half_w, half_h, z_min),
        Vec3::new(0.0, -1.0, 0.0),
        [0.45, 0.45, 0.50, 1.0],
    );

    v
}

/// Builds a flat square paddle/bat mesh centered at the origin, facing
/// along +/-Z (lies in the XY plane). Caller translates it into world
/// space by rebuilding this each frame at the paddle's current position.
pub fn paddle_mesh(half_extent: f32, center: Vec3, facing_neg_z: bool, color: [f32; 4]) -> Vec<Vertex3D> {
    let mut v = Vec::with_capacity(6);
    let normal = if facing_neg_z {
        Vec3::new(0.0, 0.0, -1.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let a = center + Vec3::new(-half_extent, -half_extent, 0.0);
    let b = center + Vec3::new(half_extent, -half_extent, 0.0);
    let c = center + Vec3::new(half_extent, half_extent, 0.0);
    let d = center + Vec3::new(-half_extent, half_extent, 0.0);
    if facing_neg_z {
        push_quad(&mut v, a, d, c, b, normal, color);
    } else {
        push_quad(&mut v, a, b, c, d, normal, color);
    }
    v
}

/// Builds a UV sphere of the given radius, centered at `center`.
pub fn sphere_mesh(
    radius: f32,
    center: Vec3,
    lat_segments: u32,
    lon_segments: u32,
    color: [f32; 4],
) -> Vec<Vertex3D> {
    let mut v = Vec::with_capacity((lat_segments * lon_segments * 6) as usize);

    let point = |lat: f32, lon: f32| -> Vec3 {
        Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin())
    };

    for i in 0..lat_segments {
        let lat0 = std::f32::consts::PI * (-0.5 + i as f32 / lat_segments as f32);
        let lat1 = std::f32::consts::PI * (-0.5 + (i + 1) as f32 / lat_segments as f32);
        for j in 0..lon_segments {
            let lon0 = 2.0 * std::f32::consts::PI * j as f32 / lon_segments as f32;
            let lon1 = 2.0 * std::f32::consts::PI * (j + 1) as f32 / lon_segments as f32;

            let n00 = point(lat0, lon0);
            let n01 = point(lat0, lon1);
            let n10 = point(lat1, lon0);
            let n11 = point(lat1, lon1);

            for (n_a, n_b, n_c) in [(n00, n10, n11), (n00, n11, n01)] {
                for n in [n_a, n_b, n_c] {
                    v.push(Vertex3D {
                        position: (center + n * radius).to_array(),
                        normal: n.to_array(),
                        color,
                    });
                }
            }
        }
    }

    v
}
