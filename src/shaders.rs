pub const MD3_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) world_pos: vec3<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    light_pos0: vec4<f32>,
    light_color0: vec4<f32>,
    light_radius0: f32,
    _padding0_0: f32,
    _padding0_1: f32,
    _padding0_2: f32,
    light_pos1: vec4<f32>,
    light_color1: vec4<f32>,
    light_radius1: f32,
    num_lights: i32,
    ambient_light: f32,
    _padding1: f32,
    _padding2_0: f32,
    _padding2_1: f32,
    _padding2_2: f32,
    _padding2_3: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var model_texture: texture_2d<f32>;

@group(0) @binding(2)
var model_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = uniforms.model * vec4<f32>(input.position, 1.0);
    output.clip_position = uniforms.view_proj * world_pos;
    output.uv = input.uv;
    output.color = input.color;
    output.normal = normalize((uniforms.model * vec4<f32>(input.normal, 0.0)).xyz);
    output.world_pos = world_pos.xyz;
    return output;
}

fn toon_quantize(value: f32, levels: f32) -> f32 {
    return floor(value * levels) / levels;
}

fn saturate_color(color: vec3<f32>, amount: f32) -> vec3<f32> {
    let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    return mix(vec3<f32>(gray), color, amount);
}

@fragment
fn fs_main(input: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    var total_light = vec3<f32>(uniforms.ambient_light * 1.5);

    if (uniforms.num_lights > 0) {
        let light_vec0 = uniforms.light_pos0.xyz - input.world_pos;
        let light_dir0 = normalize(light_vec0);
        let dist0 = length(light_vec0);
        let attenuation0 = pow(1.0 - min(dist0 / uniforms.light_radius0, 1.0), 1.2);
        let ndotl0 = max(dot(input.normal, light_dir0), 0.0);
        
        let toon_ndotl0 = toon_quantize(ndotl0, 3.0);
        total_light += uniforms.light_color0.xyz * toon_ndotl0 * attenuation0;
    }

    if (uniforms.num_lights > 1) {
        let light_vec1 = uniforms.light_pos1.xyz - input.world_pos;
        let light_dir1 = normalize(light_vec1);
        let dist1 = length(light_vec1);
        let attenuation1 = pow(1.0 - min(dist1 / uniforms.light_radius1, 1.0), 1.2);
        let ndotl1 = max(dot(input.normal, light_dir1), 0.0);
        
        let toon_ndotl1 = toon_quantize(ndotl1, 3.0);
        total_light += uniforms.light_color1.xyz * toon_ndotl1 * attenuation1;
    }

    total_light = max(vec3<f32>(0.4), min(total_light, vec3<f32>(1.8)));
    
    // Sample texture and multiply by vertex color and lighting
    let tex_color = textureSample(model_texture, model_sampler, input.uv).rgb;
    let final_color = tex_color * input.color.rgb * total_light;
    
    if (!is_front) {
        return vec4<f32>(final_color * 0.7, input.color.a);
    }
    
    return vec4<f32>(final_color, input.color.a);
}
"#;

pub const GROUND_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) normal: vec3<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    light_pos0: vec4<f32>,
    light_color0: vec4<f32>,
    light_radius0: f32,
    light_pos1: vec4<f32>,
    light_color1: vec4<f32>,
    light_radius1: f32,
    num_lights: i32,
    ambient_light: f32,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = uniforms.model * vec4<f32>(input.position, 1.0);
    output.clip_position = uniforms.view_proj * world_pos;
    output.uv = input.uv;
    output.world_pos = world_pos.xyz;
    output.normal = normalize((uniforms.model * vec4<f32>(input.normal, 0.0)).xyz);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let checker = floor(input.world_pos.x * 1.0) + floor(input.world_pos.z * 1.0);
    let base_color = select(vec3<f32>(0.25, 0.25, 0.28), vec3<f32>(0.18, 0.18, 0.2), checker % 2.0 == 0.0);
    
    var lighting = vec3<f32>(uniforms.ambient_light);
    
    if (uniforms.num_lights > 0) {
        let light_vec0 = uniforms.light_pos0.xyz - input.world_pos;
        let light_dir0 = normalize(light_vec0);
        let dist0 = length(light_vec0);
        let attenuation0 = pow(1.0 - min(dist0 / uniforms.light_radius0, 1.0), 1.6);
        let ndotl0 = max(dot(input.normal, light_dir0), 0.0);
        lighting += uniforms.light_color0.xyz * ndotl0 * attenuation0;
    }
    
    if (uniforms.num_lights > 1) {
        let light_vec1 = uniforms.light_pos1.xyz - input.world_pos;
        let light_dir1 = normalize(light_vec1);
        let dist1 = length(light_vec1);
        let attenuation1 = pow(1.0 - min(dist1 / uniforms.light_radius1, 1.0), 1.6);
        let ndotl1 = max(dot(input.normal, light_dir1), 0.0);
        lighting += uniforms.light_color1.xyz * ndotl1 * attenuation1;
    }
    
    return vec4<f32>(base_color * lighting, 1.0);
}
"#;

pub const SHADOW_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    light_pos0: vec4<f32>,
    light_color0: vec4<f32>,
    light_radius0: f32,
    _padding0_0: f32,
    _padding0_1: f32,
    _padding0_2: f32,
    light_pos1: vec4<f32>,
    light_color1: vec4<f32>,
    light_radius1: f32,
    num_lights: i32,
    ambient_light: f32,
    _padding1: f32,
    _padding2_0: f32,
    _padding2_1: f32,
    _padding2_2: f32,
    _padding2_3: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var model_texture: texture_2d<f32>;

@group(0) @binding(2)
var model_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = uniforms.model * vec4<f32>(input.position, 1.0);
    
    let ground_y = -1.5;
    let light_pos = uniforms.light_pos0.xyz;
    let light_to_vertex = world_pos.xyz - light_pos;
    let t = (ground_y - light_pos.y) / light_to_vertex.y;
    let shadow_pos = light_pos + light_to_vertex * t;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(shadow_pos.x, ground_y + 0.005, shadow_pos.z, 1.0);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.4);
}
"#;

