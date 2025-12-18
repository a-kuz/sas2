struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.tex_coords = input.tex_coords;
    return output;
}

fn tanh_approx(x: vec4<f32>) -> vec4<f32> {
    let e2x = exp(2.0 * x);
    return (e2x - 1.0) / (e2x + 1.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.tex_coords;
    let I = uv * uniforms.resolution;
    var O = vec4<f32>(0.0);
    
    let t = uniforms.time / 2.0;
    
    var i = 0.0;
    var z = 0.0;
    var d = 0.0;
    
    for (var j = 0; j < 50; j++) {
        i += 1.0;
        
        O += (sin(z / 3.0 + vec4<f32>(7.0, 2.0, 3.0, 0.0)) + 1.1) / d;
        
        var p = z * normalize(vec3<f32>(I.x + I.x, I.y + I.y, 0.0) - vec3<f32>(uniforms.resolution.x, uniforms.resolution.y, 0.0));
        p.z += 5.0 + cos(t);
        
        let c = cos(p.y * 0.5);
        let s = sin(p.y * 0.5);
        let old_x = p.x;
        p.x = c * p.x - s * p.z;
        p.z = s * old_x + c * p.z;
        
        p /= max(p.y * 0.1 + 1.0, 0.1);
        
        d = 2.0;
        for (var k = 0; k < 5; k++) {
            if (d >= 15.0) {
                break;
            }
            
            let pyzx = vec3<f32>(p.y, p.z, p.x);
            p += cos((pyzx - vec3<f32>(t / 0.1, t, d)) * d) / d;
            
            d /= 0.6;
        }
        
        let dist = 0.01 + abs(length(p.xz) + p.y * 0.3 - 0.5) / 7.0;
        d = dist;
        z += dist;
        
        if (i >= 50.0) {
            break;
        }
    }
    
    O = tanh_approx(O / 1000.0);
    O.a = 1.0;
    
    return O;
}


