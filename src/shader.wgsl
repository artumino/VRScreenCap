// Vertex shader
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

struct ScreenParams {
    x_curvature: f32,
    y_curvature: f32,
    eye_offset: f32,
    y_offset: f32,
    x_offset: f32
};

@group(1) @binding(0)
var<uniform> camera: array<CameraUniform, 2>;
@group(1) @binding(1)
var<uniform> screen_params: ScreenParams;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};


@vertex
fn vs_main(
    model: VertexInput,
    @builtin(view_index) view_index: i32
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    let x_diff = (model.tex_coords.x - 0.5) * 2.0;
    let y_diff = (model.tex_coords.y - 0.5) * 2.0;
    let z_x_curvature = (1.0 - x_diff * x_diff) * screen_params.x_curvature;
    let z_y_curvature = (1.0 - y_diff * y_diff) * screen_params.y_curvature;
    out.clip_position = camera[view_index].view_proj * vec4<f32>(model.position.xy, model.position.z - z_x_curvature - z_y_curvature, 1.0);
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput, @builtin(view_index) view_index: i32) -> @location(0) vec4<f32> {
    let x_offset = abs(f32(view_index) - screen_params.eye_offset) / 2.0;
    return textureSample(t_diffuse, s_diffuse, vec2<f32>(abs(in.tex_coords.x - screen_params.x_offset) / 2.0 + x_offset, abs(in.tex_coords.y - screen_params.y_offset)));
}