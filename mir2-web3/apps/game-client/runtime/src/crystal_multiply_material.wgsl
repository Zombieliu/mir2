#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::view,
}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var light_buffer: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var light_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> uv_scale_offset: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> border_darkness: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Crystal first clears an offscreen buffer to the map darkness colour,
    // adds every procedural light into that buffer, then multiplies the
    // completed RGB value with the already-rendered scene.
    // The world-space composite follows the same camera pose as the offscreen
    // light pass. Its centre maps to the original target at 1:1 pixel density;
    // only the normally-offscreen virtual guard falls outside 0..1 and receives
    // the exact light-buffer clear colour.
    let source_uv = mesh.uv * uv_scale_offset.xy + uv_scale_offset.zw;
    let inside_x = source_uv.x >= 0.0 && source_uv.x <= 1.0;
    let inside_y = source_uv.y >= 0.0 && source_uv.y <= 1.0;
    var output_color = border_darkness;
    if inside_x && inside_y {
        let sampled = textureSample(light_buffer, light_sampler, source_uv);
        output_color = vec4<f32>(sampled.rgb, 1.0);
    }

#ifdef TONEMAP_IN_SHADER
    output_color = tonemapping::tone_mapping(output_color, view.color_grading);
#endif
    return output_color;
}
