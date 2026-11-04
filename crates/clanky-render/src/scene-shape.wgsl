struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) @interpolate(flat) rect: vec4<f32>,
    @location(2) @interpolate(flat) metrics: vec4<f32>,
    @location(3) @interpolate(flat) top: vec4<f32>,
    @location(4) @interpolate(flat) bottom: vec4<f32>,
    @location(5) @interpolate(flat) border: vec4<f32>,
    @location(6) @interpolate(flat) clip: vec4<f32>,
};
@vertex fn vs_main(@location(0) position: vec2<f32>, @location(1) point: vec2<f32>,
    @location(2) rect: vec4<f32>, @location(3) metrics: vec4<f32>,
    @location(4) top: vec4<f32>, @location(5) bottom: vec4<f32>,
    @location(6) border: vec4<f32>, @location(7) clip: vec4<f32>) -> Out {
    var out: Out;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.point = point;
    out.rect = rect;
    out.metrics = metrics;
    out.top = top;
    out.bottom = bottom;
    out.border = border;
    out.clip = clip;
    return out;
}
fn distance_to_rect(point: vec2<f32>, rect: vec4<f32>, radius: f32) -> f32 {
    let half_size = rect.zw * 0.5;
    let r = clamp(radius, 0.0, min(half_size.x, half_size.y));
    let q = abs(point - rect.xy - half_size) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}
fn coverage(distance: f32) -> f32 { return clamp(0.5 - distance, 0.0, 1.0); }
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> {
    let d = distance_to_rect(in.point, in.rect, in.metrics.x);
    if in.metrics.z > 0.5 {
        // The clip field describes the original shape for shadow cutout.
        let outside = 1.0 - coverage(distance_to_rect(in.point, in.clip, in.metrics.y));
        var falloff = coverage(d);
        if in.metrics.w > 0.0 {
            let normalized = max(d, 0.0) / in.metrics.w;
            falloff = exp(-0.5 * normalized * normalized)
                * (1.0 - smoothstep(0.5, 1.0, normalized));
        }
        return in.top * (outside * falloff);
    }
    if any(in.point < in.clip.xy) || any(in.point >= in.clip.xy + in.clip.zw) { discard; }
    let t = clamp((in.point.y - in.rect.y) / max(in.rect.w, 0.0001), 0.0, 1.0);
    let fill = mix(in.top, in.bottom, t);
    let outer = coverage(d);
    let inner = coverage(d + in.metrics.y);
    let stroke = in.border + fill * (1.0 - in.border.a);
    return fill * inner + stroke * (outer - inner);
}
