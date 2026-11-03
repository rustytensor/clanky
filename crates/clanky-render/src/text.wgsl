struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) atlas_position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0) var atlas: texture_2d<f32>;

@vertex
fn vs_main(@location(0) position: vec2<f32>,
           @location(1) atlas_position: vec2<f32>,
           @location(2) color: vec4<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.atlas_position = atlas_position;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = textureLoad(atlas, vec2<i32>(in.atlas_position), 0).r;
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
