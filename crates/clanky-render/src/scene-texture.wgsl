struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
};
@group(0) @binding(0) var pixels: texture_2d<f32>;
@group(0) @binding(1) var glyph_sampler: sampler;
@vertex fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>, @location(3) clip: vec4<f32>) -> Out {
    var out: Out;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    out.clip = clip;
    return out;
}
fn clipped(point: vec2<f32>, clip: vec4<f32>) -> bool {
    return any(point < clip.xy) || any(point >= clip.xy + clip.zw);
}
@fragment fn fs_glyph(in: Out) -> @location(0) vec4<f32> {
    if clipped(in.position.xy, in.clip) { discard; }
    let alpha = textureSampleLevel(pixels, glyph_sampler, in.uv / vec2<f32>(textureDimensions(pixels)), 0.0).r;
    return in.color * alpha;
}
@fragment fn fs_image(in: Out) -> @location(0) vec4<f32> {
    if clipped(in.position.xy, in.clip) { discard; }
    let last = vec2<i32>(textureDimensions(pixels)) - vec2<i32>(1);
    let texel = clamp(vec2<i32>(in.uv), vec2<i32>(0), last);
    let rgba = textureLoad(pixels, texel, 0);
    return vec4<f32>(rgba.rgb * rgba.a, rgba.a) * in.color;
}
