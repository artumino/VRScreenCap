struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

struct TemporalBlurParams {
    jitter: vec2<f32>,
    scale: vec2<f32>,
    resolution: vec2<f32>,
    history_decay: f32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var out: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let tc = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    out.clip_position = vec4<f32>(
        tc.x * 2.0 - 1.0,
        1.0 - tc.y * 2.0,
        0.0, 1.0
    );
    out.tex_coords = tc;
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(1) @binding(0)
var t_history: texture_2d<f32>;
@group(1) @binding(1)
var s_history: sampler;
@group(2) @binding(0)
var<uniform> blur_params: TemporalBlurParams;

struct TemporalOutput {
    @location(0) color: vec4<f32>,
    @location(1) history: vec4<f32>,
}
@fragment
fn temporal_fs_main(in: VertexOutput) -> TemporalOutput {
    var out: TemporalOutput;
    let current_color = textureSample(t_diffuse, s_diffuse, in.tex_coords + blur_params.jitter * blur_params.scale);
    let history_color = textureSample(t_history, s_history, in.tex_coords);
    let mixed_color = mix(current_color, history_color, blur_params.history_decay).rgb;
    let brightness = smoothstep(0.05, 0.35, dot(mixed_color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)));
    out.color = vec4<f32>(mixed_color * brightness, 1.0);
    out.history = vec4<f32>(mixed_color.rgb, 1.0);
    return out;
}