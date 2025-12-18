struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

struct Uniforms {
    resolution: vec2<f32>,
    position: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    return output;
}

fn get_crosshair(coord: vec2<f32>) -> vec4<f32> {
    let arm_length = 5.0;
    let gap = 2.5;
    
    let ipos = round(coord);
    
    let inner_h = (abs(ipos.y) == 0.0) && (abs(ipos.x) >= gap) && (abs(ipos.x) <= arm_length);
    let inner_v = (abs(ipos.x) == 0.0) && (abs(ipos.y) >= gap) && (abs(ipos.y) <= arm_length);
    
    if (inner_h || inner_v) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    let fat_h = (abs(ipos.y) <= 1.0) && (abs(ipos.x) >= gap - 1.0) && (abs(ipos.x) <= arm_length + 1.0);
    let fat_v = (abs(ipos.x) <= 1.0) && (abs(ipos.y) >= gap - 1.0) && (abs(ipos.y) <= arm_length + 1.0);
    let in_center_block = abs(ipos.x) < gap - 1.0 && abs(ipos.y) < gap - 1.0;
    
    if ((fat_h || fat_v) && !in_center_block) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let frag_coord = vec2<f32>(
        input.position.x,
        input.position.y
    );
    
    let coord = frag_coord - uniforms.position;
    
    let crosshair = get_crosshair(coord / 2.0);
    
    return crosshair;
}
