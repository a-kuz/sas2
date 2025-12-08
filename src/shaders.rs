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

struct LightData {
    position: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    lights: array<LightData, 8>,
    num_lights: i32,
    ambient_light: f32,
    _padding0: f32,
    _padding1: f32,
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

    for (var i = 0; i < uniforms.num_lights; i++) {
        let light = uniforms.lights[i];
        let light_vec = light.position.xyz - input.world_pos;
        let light_dir = normalize(light_vec);
        let dist = length(light_vec);
        let attenuation = pow(1.0 - min(dist / light.radius, 1.0), 1.2);
        let ndotl = max(dot(input.normal, light_dir), 0.0);
        
        let toon_ndotl = toon_quantize(ndotl, 3.0);
        total_light += light.color.xyz * toon_ndotl * attenuation;
    }

    total_light = max(vec3<f32>(0.4), min(total_light, vec3<f32>(1.8)));
    
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

struct LightData {
    position: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    lights: array<LightData, 8>,
    num_lights: i32,
    ambient_light: f32,
    _padding0: f32,
    _padding1: f32,
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
    
    for (var i = 0; i < uniforms.num_lights; i++) {
        let light = uniforms.lights[i];
        let light_vec = light.position.xyz - input.world_pos;
        let light_dir = normalize(light_vec);
        let dist = length(light_vec);
        let attenuation = pow(1.0 - min(dist / light.radius, 1.0), 1.6);
        let ndotl = max(dot(input.normal, light_dir), 0.0);
        lighting += light.color.xyz * ndotl * attenuation;
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
    @location(0) world_pos: vec2<f32>,
    @location(1) light_pos_2d: vec2<f32>,
    @location(2) vertex_to_center: vec2<f32>,
}

struct LightData {
    position: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    lights: array<LightData, 8>,
    num_lights: i32,
    ambient_light: f32,
    _padding0: f32,
    _padding1: f32,
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
    let light_pos = uniforms.lights[0].position.xyz;
    let light_to_vertex = world_pos.xyz - light_pos;
    let t = (ground_y - light_pos.y) / light_to_vertex.y;
    let shadow_pos_center = light_pos + light_to_vertex * t;
    
    let shadow_center_2d = vec2<f32>(light_pos.x, light_pos.z);
    let to_shadow = vec2<f32>(shadow_pos_center.x, shadow_pos_center.z) - shadow_center_2d;
    let expand_amount = 0.15;
    let shadow_pos_expanded = shadow_pos_center.xz + normalize(to_shadow) * expand_amount;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(shadow_pos_expanded.x, ground_y + 0.005, shadow_pos_expanded.y, 1.0);
    output.world_pos = shadow_pos_expanded;
    output.light_pos_2d = shadow_center_2d;
    output.vertex_to_center = to_shadow;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let dist_to_light = length(input.world_pos - input.light_pos_2d);
    let max_shadow_dist = 15.0;
    let soft_edge_width = 2.0;
    
    let distance_falloff = smoothstep(max_shadow_dist, max_shadow_dist - soft_edge_width, dist_to_light);
    
    let edge_dist = length(input.vertex_to_center);
    let edge_softness = smoothstep(0.3, 0.0, edge_dist);
    
    let shadow_alpha = 0.65 * distance_falloff * (0.6 + 0.4 * edge_softness);
    
    return vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
}
"#;

pub const WALL_SHADOW_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) light_pos_2d: vec2<f32>,
    @location(2) vertex_to_center: vec2<f32>,
}

struct LightData {
    position: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    lights: array<LightData, 8>,
    num_lights: i32,
    ambient_light: f32,
    _padding0: f32,
    _padding1: f32,
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
    
    let wall_z = -3.0;
    let light_pos = uniforms.lights[0].position.xyz;
    let light_to_vertex = world_pos.xyz - light_pos;
    
    if (abs(light_to_vertex.z) < 0.001 || light_to_vertex.z >= 0.0) {
        output.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
        output.world_pos = vec2<f32>(0.0, 0.0);
        output.light_pos_2d = vec2<f32>(0.0, 0.0);
        output.vertex_to_center = vec2<f32>(0.0, 0.0);
        return output;
    }
    
    let t = (wall_z - light_pos.z) / light_to_vertex.z;
    
    if (t < 0.0) {
        output.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
        output.world_pos = vec2<f32>(0.0, 0.0);
        output.light_pos_2d = vec2<f32>(0.0, 0.0);
        output.vertex_to_center = vec2<f32>(0.0, 0.0);
        return output;
    }
    
    let shadow_pos_center = light_pos + light_to_vertex * t;
    
    let shadow_center_2d = vec2<f32>(light_pos.x, light_pos.y);
    let to_shadow = vec2<f32>(shadow_pos_center.x, shadow_pos_center.y) - shadow_center_2d;
    let expand_amount = 0.15;
    let shadow_pos_expanded = shadow_pos_center.xy + normalize(to_shadow) * expand_amount;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(shadow_pos_expanded.x, shadow_pos_expanded.y, wall_z + 0.01, 1.0);
    output.world_pos = shadow_pos_expanded;
    output.light_pos_2d = shadow_center_2d;
    output.vertex_to_center = to_shadow;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let dist_to_light = length(input.world_pos - input.light_pos_2d);
    let max_shadow_dist = 15.0;
    let soft_edge_width = 2.0;
    
    let distance_falloff = smoothstep(max_shadow_dist, max_shadow_dist - soft_edge_width, dist_to_light);
    
    let edge_dist = length(input.vertex_to_center);
    let edge_softness = smoothstep(0.3, 0.0, edge_dist);
    
    let shadow_alpha = 0.7 * distance_falloff * (0.6 + 0.4 * edge_softness);
    
    return vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
}
"#;

pub const PARTICLE_SHADER: &str = r#"
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
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = uniforms.model * vec4<f32>(input.position, 1.0);
    
    let to_camera = normalize(uniforms.camera_pos.xyz - world_pos.xyz);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_camera));
    let up = cross(to_camera, right);
    
    let billboard_pos = world_pos.xyz + right * (input.uv.x - 0.5) * 2.0 + up * (input.uv.y - 0.5) * 2.0;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(billboard_pos, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = length(input.uv - center);
    let alpha = smoothstep(0.5, 0.2, dist) * input.color.a;
    
    let smoke_color = vec3<f32>(0.3, 0.3, 0.35);
    return vec4<f32>(smoke_color, alpha * 0.4);
}
"#;

pub const FLAME_SHADER: &str = r#"
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
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec4<f32>,
    time: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = uniforms.model * vec4<f32>(input.position, 1.0);
    
    let to_camera = normalize(uniforms.camera_pos.xyz - world_pos.xyz);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_camera));
    let up = cross(to_camera, right);
    
    let billboard_pos = world_pos.xyz + right * (input.uv.x - 0.5) * 1.5 + up * (input.uv.y - 0.5) * 1.5;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(billboard_pos, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = length(input.uv - center);
    
    let flicker = sin(uniforms.time * 30.0) * 0.1 + 0.9;
    let alpha = smoothstep(0.5, 0.0, dist) * input.color.a * flicker;
    
    let inner_color = vec3<f32>(1.0, 0.9, 0.6);
    let outer_color = vec3<f32>(1.0, 0.4, 0.1);
    let flame_color = mix(outer_color, inner_color, 1.0 - dist * 2.0);
    
    return vec4<f32>(flame_color, alpha);
}
"#;
