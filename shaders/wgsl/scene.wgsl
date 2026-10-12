struct ViewUniforms {
    selected: u32,
    offset_x: f32,
    offset_y: f32,
    padding: f32,
}

@group(0) @binding(0) var<uniform> view: ViewUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) object_id: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    let object_x = (f32(input.object_id) - 1.0) * 1.35;
    let offset_x = select(0.0, view.offset_x, input.object_id == view.selected);
    let offset_y = select(0.0, view.offset_y, input.object_id == view.selected);
    let angle = 0.48;
    let cosine = cos(angle);
    let sine = sin(angle);
    let world_x = input.position.x + object_x + offset_x;
    let world_y = input.position.y + offset_y;
    let world_z = input.position.z;
    let rotated_x = cosine * world_x + sine * world_z;
    let rotated_z = -sine * world_x + cosine * world_z;
    let depth = rotated_z + 4.2;

    var output: VertexOutput;
    output.position = vec4<f32>(rotated_x * 1.35 / depth, world_y * 1.35 / depth, (depth - 0.5) / 8.0, 1.0);
    output.color = select(input.color, min(input.color * 1.45, vec3<f32>(1.0)), input.object_id == view.selected);
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
