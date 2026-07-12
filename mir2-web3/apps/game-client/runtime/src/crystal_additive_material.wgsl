#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::view,
}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> uv_scale_offset: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var color_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv * uv_scale_offset.xy + uv_scale_offset.zw;
    let sampled = textureSample(color_texture, color_sampler, uv);
    let source_alpha = clamp(sampled.a * tint.a, 0.0, 1.0);
    let brightness = clamp(max(sampled.r, max(sampled.g, sampled.b)), 0.0, 1.0);
    let coverage = source_alpha * brightness;
    var output_color = vec4<f32>(sampled.rgb * tint.rgb * source_alpha, coverage);

#ifdef TONEMAP_IN_SHADER
    output_color = tonemapping::tone_mapping(output_color, view.color_grading);
#endif
    return output_color;
}
