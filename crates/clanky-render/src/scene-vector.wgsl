struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) clip: vec4<f32>,
};
@vertex fn vs_main(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>,
    @location(2) clip: vec4<f32>) -> Out {
    var out: Out;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.clip = clip;
    return out;
}
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> {
    if any(in.position.xy < in.clip.xy) || any(in.position.xy >= in.clip.xy + in.clip.zw) { discard; }
    return in.color;
}
