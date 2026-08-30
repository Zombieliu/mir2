#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::view,
}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var light_buffer: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var light_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Crystal first clears an offscreen buffer to the map darkness colour,
    // adds every procedural light into that buffer, then multiplies the
    // completed RGB value with the already-rendered scene.
    let sampled = textureSample(light_buffer, light_sampler, mesh.uv);
    var output_color = vec4<f32>(sampled.rgb, 1.0);

#ifdef TONEMAP_IN_SHADER
    output_color = tonemapping::tone_mapping(output_color, view.color_grading);
#endif
    return output_color;
}
