struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) image_position: vec2<f32>,
};

@group(0) @binding(0) var image: texture_2d<f32>;

@vertex
fn vs_main(@location(0) position: vec2<f32>,
           @location(1) image_position: vec2<f32>,
           @location(2) color: vec4<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.image_position = image_position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Integer texel loads are nearest sampling, independent of device defaults.
    let last = vec2<i32>(textureDimensions(image)) - vec2<i32>(1);
    let texel = clamp(vec2<i32>(in.image_position), vec2<i32>(0), last);
    return textureLoad(image, texel, 0);
}
