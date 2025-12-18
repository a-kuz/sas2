pub const MD3_ADDITIVE_SHADER: &str = r#"
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
    lights: array<LightData, 8>,
    num_lights: i32,
    ambient_light: f32,
    _padding0: f32,
    _padding1: f32,
}

struct LightData {
    position: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
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
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(model_texture, model_sampler, input.uv);
    return vec4<f32>(tex_color.rgb * input.color.rgb, tex_color.a * input.color.a);
}
"#;

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
    var total_light = vec3<f32>(uniforms.ambient_light);

    for (var i = 0; i < uniforms.num_lights; i++) {
        let light = uniforms.lights[i];
        let light_vec = light.position.xyz - input.world_pos;
        let dist_sq = dot(light_vec, light_vec);
        let radius_sq = light.radius * light.radius;
        
        if (dist_sq > radius_sq) {
            continue;
        }
        
        let dist_norm_sq = dist_sq / radius_sq;
        if (dist_norm_sq >= 1.0) {
            continue;
        }
        
        let light_dir = light_vec * inverseSqrt(max(dist_sq, 0.0001));
        let ndotl = max(dot(input.normal, light_dir), 0.0);
        
        if (ndotl < 0.01) {
            continue;
        }
        
        let falloff = 1.0 - dist_norm_sq;
        let attenuation = falloff * falloff;
        
        let toon_ndotl = toon_quantize(ndotl, 3.0);
        let contribution = light.color.xyz * toon_ndotl * attenuation;
        
        if (max(max(contribution.x, contribution.y), contribution.z) < 0.001) {
            continue;
        }
        
        total_light += contribution;
    }

    total_light = min(total_light, vec3<f32>(1.8));
    
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

@group(0) @binding(1)
var ground_texture: texture_2d<f32>;

@group(0) @binding(2)
var ground_sampler: sampler;

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
    let texture_size = 64.0;
    let scale = 1.0;
    
    let tiled_uv = vec2<f32>(
        input.world_pos.x / texture_size * scale,
        input.world_pos.y / texture_size * scale
    );
    
    let tex_color = textureSample(ground_texture, ground_sampler, tiled_uv).rgb;
    
    var lighting = vec3<f32>(uniforms.ambient_light);
    
    for (var i = 0; i < uniforms.num_lights; i++) {
        let light = uniforms.lights[i];
        let light_vec = light.position.xyz - input.world_pos;
        let dist_sq = dot(light_vec, light_vec);
        let radius_sq = light.radius * light.radius;
        
        if (dist_sq > radius_sq) {
            continue;
        }
        
        let dist_norm_sq = dist_sq / radius_sq;
        if (dist_norm_sq >= 1.0) {
            continue;
        }
        
        let light_dir = light_vec * inverseSqrt(max(dist_sq, 0.0001));
        let ndotl = max(dot(input.normal, light_dir), 0.0);
        
        if (ndotl < 0.01) {
            continue;
        }
        
        let falloff = 1.0 - dist_norm_sq;
        let attenuation = falloff * falloff * falloff;
        
        let contribution = light.color.xyz * ndotl * attenuation;
        
        if (max(max(contribution.x, contribution.y), contribution.z) < 0.001) {
            continue;
        }
        
        lighting += contribution;
    }
    
    return vec4<f32>(tex_color * lighting, 1.0);
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

    let ground_y = 0.0;
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
    
    let shadow_alpha = 0.85 * distance_falloff * (0.6 + 0.4 * edge_softness);
    
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

    let ground_y = 0.0;
    if (shadow_pos_center.y < ground_y) {
        output.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
        output.world_pos = vec2<f32>(0.0, 0.0);
        output.light_pos_2d = vec2<f32>(0.0, 0.0);
        output.vertex_to_center = vec2<f32>(0.0, 0.0);
        return output;
    }
    
    let shadow_center_2d = vec2<f32>(light_pos.x, light_pos.y);
    let to_shadow = vec2<f32>(shadow_pos_center.x, shadow_pos_center.y) - shadow_center_2d;
    let expand_amount = 0.15;
    let shadow_pos_expanded = shadow_pos_center.xy + normalize(to_shadow) * expand_amount;

    if (shadow_pos_expanded.y < ground_y) {
        output.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
        output.world_pos = vec2<f32>(0.0, 0.0);
        output.light_pos_2d = vec2<f32>(0.0, 0.0);
        output.vertex_to_center = vec2<f32>(0.0, 0.0);
        return output;
    }
    
    output.clip_position = uniforms.view_proj * vec4<f32>(shadow_pos_expanded.x, shadow_pos_expanded.y, wall_z + 0.01, 1.0);
    output.world_pos = shadow_pos_expanded;
    output.light_pos_2d = shadow_center_2d;
    output.vertex_to_center = to_shadow;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let dist_to_light = length(input.world_pos - input.light_pos_2d);
    let max_shadow_dist = 25.0;
    let soft_edge_width = 5.0;

    let distance_falloff = smoothstep(max_shadow_dist, max_shadow_dist - soft_edge_width, dist_to_light);

    let edge_dist = length(input.vertex_to_center);
    let edge_softness = smoothstep(0.3, 0.0, edge_dist);

    let shadow_alpha = 1.3 * distance_falloff * (0.6 + 0.4 * edge_softness);

    return vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
}
"#;

pub const WALL_SHADER: &str = r#"
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

@group(0) @binding(1)
var wall_texture: texture_2d<f32>;

@group(0) @binding(2)
var wall_sampler: sampler;

@group(0) @binding(3)
var curb_texture: texture_2d<f32>;

@group(0) @binding(4)
var curb_sampler: sampler;

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
    let wall_bottom = 0.0;
    let wall_height = 50.0;
    let curb_height = 10.0;
    let curb_start = wall_bottom;
    let curb_end = wall_bottom + curb_height;
    
    let world_y = input.world_pos.y;
    let is_curb = world_y >= curb_start && world_y <= curb_end;
    
    let texture_size = 64.0;
    let scale = 1.0;
    
    let tiled_uv = vec2<f32>(
        input.world_pos.x / texture_size * scale,
        input.world_pos.y / texture_size * scale
    );
    
    var base_color: vec3<f32>;
    
    if (is_curb) {
        let curb_uv = vec2<f32>(
            input.world_pos.x / texture_size * scale * 2.0,
            (world_y - curb_start) / texture_size * scale * 2.0
        );
        base_color = textureSample(curb_texture, curb_sampler, curb_uv).rgb;
        
        let transition = smoothstep(0.0, 0.1, abs(world_y - curb_end));
        let wall_color = textureSample(wall_texture, wall_sampler, tiled_uv).rgb;
        base_color = mix(base_color, wall_color, transition);
    } else {
        base_color = textureSample(wall_texture, wall_sampler, tiled_uv).rgb;
        
        let transition = smoothstep(0.0, 0.1, abs(world_y - curb_end));
        let curb_uv = vec2<f32>(
            input.world_pos.x / texture_size * scale * 2.0,
            (curb_end - curb_start) / texture_size * scale * 2.0
        );
        let curb_color = textureSample(curb_texture, curb_sampler, curb_uv).rgb;
        base_color = mix(curb_color, base_color, transition);
    }
    
    var lighting = vec3<f32>(uniforms.ambient_light);
    
    for (var i = 0; i < uniforms.num_lights; i++) {
        let light = uniforms.lights[i];
        let light_vec = light.position.xyz - input.world_pos;
        let dist_sq = dot(light_vec, light_vec);
        let radius_sq = light.radius * light.radius;
        
        if (dist_sq > radius_sq) {
            continue;
        }
        
        let dist_norm_sq = dist_sq / radius_sq;
        if (dist_norm_sq >= 1.0) {
            continue;
        }
        
        let light_dir = light_vec * inverseSqrt(max(dist_sq, 0.0001));
        let ndotl = max(dot(input.normal, light_dir), 0.0);
        
        if (ndotl < 0.01) {
            continue;
        }
        
        let falloff = 1.0 - dist_norm_sq;
        let attenuation = falloff * falloff * falloff;
        
        let contribution = light.color.xyz * ndotl * attenuation;
        
        if (max(max(contribution.x, contribution.y), contribution.z) < 0.001) {
            continue;
        }
        
        lighting += contribution;
    }
    
    return vec4<f32>(base_color * lighting, 1.0);
}
"#;

pub const TILE_SHADER: &str = r#"
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

@group(0) @binding(1)
var tile_texture: texture_2d<f32>;

@group(0) @binding(2)
var tile_sampler: sampler;

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
    let texture_size = 64.0;
    let scale = 1.0;
    
    let tiled_uv = vec2<f32>(
        input.world_pos.x / texture_size * scale,
        input.world_pos.z / texture_size * scale
    );
    
    let tex_color = textureSample(tile_texture, tile_sampler, tiled_uv).rgb;
    
    var lighting = vec3<f32>(uniforms.ambient_light);
    
    for (var i = 0; i < uniforms.num_lights; i++) {
        let light = uniforms.lights[i];
        let light_vec = light.position.xyz - input.world_pos;
        let dist_sq = dot(light_vec, light_vec);
        let radius_sq = light.radius * light.radius;
        
        if (dist_sq > radius_sq) {
            continue;
        }
        
        let dist_norm_sq = dist_sq / radius_sq;
        if (dist_norm_sq >= 1.0) {
            continue;
        }
        
        let light_dir = light_vec * inverseSqrt(max(dist_sq, 0.0001));
        let ndotl = max(dot(input.normal, light_dir), 0.0);
        
        if (ndotl < 0.01) {
            continue;
        }
        
        let falloff = 1.0 - dist_norm_sq;
        let attenuation = falloff * falloff * falloff;
        
        let contribution = light.color.xyz * ndotl * attenuation;
        
        if (max(max(contribution.x, contribution.y), contribution.z) < 0.001) {
            continue;
        }
        
        lighting += contribution;
    }
    
    return vec4<f32>(tex_color * lighting, 1.0);
}
"#;

pub const PARTICLE_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct InstanceInput {
    @location(4) position_size: vec4<f32>,
    @location(5) alpha: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var smoke_texture: texture_2d<f32>;

@group(0) @binding(2)
var smoke_sampler: sampler;

@vertex
fn vs_main(input: VertexInput, instance: InstanceInput) -> VertexOutput {
    var output: VertexOutput;
    let instance_pos = instance.position_size.xyz;
    let instance_size = instance.position_size.w;
    
    let world_pos = vec4<f32>(instance_pos, 1.0);
    
    let to_camera = normalize(uniforms.camera_pos.xyz - world_pos.xyz);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_camera));
    let up = cross(to_camera, right);
    
    let billboard_pos = world_pos.xyz + right * (input.uv.x - 0.5) * 2.0 * instance_size + up * (input.uv.y - 0.5) * 2.0 * instance_size;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(billboard_pos, 1.0);
    output.uv = input.uv;
    output.alpha = instance.alpha;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(smoke_texture, smoke_sampler, input.uv);
    let dist = distance(input.uv, vec2<f32>(0.5, 0.5));
    let edge = smoothstep(0.5, 0.2, dist);
    let alpha = tex_color.a * input.alpha * edge;
    let color = tex_color.rgb;
    return vec4<f32>(color, alpha);
}
"#;

pub const FLAME_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct InstanceInput {
    @location(4) position_size: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var flame_texture: texture_2d<f32>;

@group(0) @binding(2)
var flame_sampler: sampler;

@vertex
fn vs_main(input: VertexInput, instance: InstanceInput) -> VertexOutput {
    var output: VertexOutput;
    let instance_pos = instance.position_size.xyz;
    let instance_size = instance.position_size.w;
    
    let world_pos = vec4<f32>(instance_pos, 1.0);
    
    let to_camera = normalize(uniforms.camera_pos.xyz - world_pos.xyz);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_camera));
    let up = cross(to_camera, right);
    
    let uv_x = input.uv.x - 0.5;
    let uv_y = input.uv.y - 0.5;
    
    let billboard_pos = world_pos.xyz + right * uv_x * instance_size + up * uv_y * instance_size;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(billboard_pos, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(flame_texture, flame_sampler, input.uv);
    return vec4<f32>(tex_color.rgb, tex_color.a);
}
"#;

pub const DEBUG_LIGHT_SPHERE_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct InstanceInput {
    @location(4) position_radius: vec4<f32>,
    @location(5) light_color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) light_color: vec4<f32>,
    @location(2) radius: f32,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput, instance: InstanceInput) -> VertexOutput {
    var output: VertexOutput;
    let instance_pos = instance.position_radius.xyz;
    let instance_radius = instance.position_radius.w;
    
    let world_pos = instance_pos + input.position * instance_radius;
    
    output.clip_position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    output.world_pos = world_pos;
    output.light_color = instance.light_color;
    output.radius = instance_radius;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.light_color.rgb, 0.6);
}
"#;

pub const DEBUG_LIGHT_RAY_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

pub const SHADOW_VOLUME_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) extrude: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    light_pos: vec4<f32>,
    extrude_distance: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    var world_pos: vec3<f32>;
    if (input.extrude > 0.5) {
        let light_to_vertex = input.position - uniforms.light_pos.xyz;
        let extruded_pos = input.position + normalize(light_to_vertex) * input.extrude;
        world_pos = extruded_pos;
    } else {
        world_pos = input.position;
    }
    
    output.clip_position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
"#;

pub const SHADOW_APPLY_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(input.position, 0.0, 1.0);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.75);
}
"#;

pub const SHADOW_PLANAR_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    light_pos: vec4<f32>,
    extrude_distance: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    return output;
}

@fragment
fn fs_main(_input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.75);
}
"#;

pub const COORDINATE_GRID_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = uniforms.model * vec4<f32>(input.position, 1.0);
    output.clip_position = uniforms.view_proj * world_pos;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;
