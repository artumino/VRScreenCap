// Vertex shader
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

struct ModelUniform {
    model_matrix: mat4x4<f32>,
};

struct ScreenParams {
    x_curvature: f32,
    y_curvature: f32,
    eye_offset: f32,
    y_offset: f32,
    x_offset: f32,
    aspect_ratio: f32,
    screen_width: u32,
    ambient_width: u32,
    stereo_x: f32, // 0.0 = disabled, 1.0 = enabled
    stereo_y: f32, // 0.0 = disabled, 1.0 = enabled
};

@group(1) @binding(0)
var<uniform> camera: array<CameraUniform, 2>;
@group(1) @binding(1)
var<uniform> screen_params: ScreenParams;
//Could be a push constant but we only have one entity
@group(1) @binding(2)
var<uniform> model_uniform: ModelUniform;

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
    out.clip_position = camera[view_index].view_proj * model_uniform.model_matrix * vec4<f32>(model.position.xy, model.position.z - z_x_curvature - z_y_curvature, 1.0);
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

fn uv_to_stereo_uv(view_index: i32, uv: vec2<f32>) -> vec2<f32> {
    let x_divider = 2.0 - (1.0 - screen_params.stereo_x);
    let y_divider = 2.0 - (1.0 - screen_params.stereo_y); 
    let x_offset = (abs(f32(view_index) - screen_params.eye_offset) / 2.0) * screen_params.stereo_x;
    let y_offset = (abs(f32(view_index) - screen_params.eye_offset) / 2.0) * screen_params.stereo_y;
    return vec2<f32>((abs(uv.x - screen_params.x_offset) / x_divider) + x_offset, (abs(uv.y - screen_params.y_offset) / y_divider) + y_offset);
}

@fragment
fn fs_main(in: VertexOutput, @builtin(view_index) view_index: i32) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, uv_to_stereo_uv(view_index, in.tex_coords));
}

@vertex
fn mv_vs_main(
    model: VertexInput,
    @builtin(view_index) view_index: i32
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.clip_position = camera[view_index].view_proj * model_uniform.model_matrix * vec4<f32>(model.position.xyz, 1.0);
    return out;
}

fn weighted9_sample(vstep: f32, hstep: f32,
        w_ul: f32, w_u: f32,  w_ur: f32,
        w_l: f32,  w_c: f32,  w_r: f32,
        w_dl: f32, w_d: f32, w_dr: f32,
        t_texture: texture_2d<f32>, s_texture: sampler, uv: vec2<f32>) -> vec4<f32> {
    let center_color = textureSample(t_texture, s_texture, uv);
    let center_up_color = textureSample(t_texture, s_texture, uv + vec2<f32>(0.0, vstep));
    let center_down_color = textureSample(t_texture, s_texture, uv - vec2<f32>(0.0, vstep));
    let center_left_color = textureSample(t_texture, s_texture, uv + vec2<f32>(hstep, 0.0));
    let center_right_color = textureSample(t_texture, s_texture, uv - vec2<f32>(hstep, 0.0));
    let center_up_left_color = textureSample(t_texture, s_texture, uv + vec2<f32>(hstep, vstep));
    let center_up_right_color = textureSample(t_texture, s_texture, uv + vec2<f32>(-hstep, vstep));
    let center_down_right_color = textureSample(t_texture, s_texture, uv - vec2<f32>(hstep, vstep));
    let center_down_left_color = textureSample(t_texture, s_texture, uv - vec2<f32>(-hstep, vstep));
    return center_color * w_c
      + center_up_color * w_u
      + center_down_color * w_d
      + center_left_color * w_l
      + center_right_color * w_r
      + center_up_left_color * w_ul
      + center_up_right_color * w_ur
      + center_down_right_color * w_dr
      + center_down_left_color * w_dl;
}

@fragment
fn vignette_fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let clamped_text_coords = clamp(in.tex_coords, vec2<f32>(0.0), vec2<f32>(1.0)) - vec2<f32>(0.5);
    let dist = length(clamped_text_coords);
    let vig = 1.0 - smoothstep(0.35, 0.5, dist);
    let hstep = 1.0 / f32(screen_params.ambient_width);
    let vstep = 1.0 / (f32(screen_params.ambient_width) * screen_params.aspect_ratio);
    return vec4<f32>(weighted9_sample(hstep, vstep, 
        0.0625, 0.125, 0.0625,
        0.125,  0.25,  0.125,
        0.0625, 0.125, 0.0625,
        //0.1111111111111111, 0.1111111111111111, 0.1111111111111111,
        //0.1111111111111111, 0.1111111111111111, 0.1111111111111111,
        //0.1111111111111111, 0.1111111111111111, 0.1111111111111111,
        t_diffuse, s_diffuse, uv_to_stereo_uv(0, in.tex_coords)).rgb * vig, 1.0);
                            // ^ forced mono to left eye
}