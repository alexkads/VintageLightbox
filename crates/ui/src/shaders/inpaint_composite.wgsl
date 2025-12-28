// Inpaint Composite Shader
// Combina a imagem original com a imagem inpainted usando a máscara,
// aplicando blend suave nas bordas para transição seamless.

struct CompositeParams {
    blend_radius: f32,        // Raio do blend nas bordas (em pixels)
    image_width: f32,
    image_height: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> params: CompositeParams;
@group(0) @binding(1) var original_texture: texture_2d<f32>;
@group(0) @binding(2) var inpainted_texture: texture_2d<f32>;
@group(0) @binding(3) var mask_texture: texture_2d<f32>;
@group(0) @binding(4) var output_texture: texture_storage_2d<rgba8unorm, write>;

// Calcula distância até a borda da máscara para blend suave
fn calculate_blend_weight(coord: vec2<i32>, mask_value: f32) -> f32 {
    if (mask_value < 0.5) {
        // Pixel original - não precisa de blend
        return 0.0;
    }

    // Buscar distância até pixel válido (mask = 0)
    var min_dist: f32 = params.blend_radius + 1.0;
    let radius = i32(params.blend_radius);

    for (var dy: i32 = -radius; dy <= radius; dy = dy + 1) {
        for (var dx: i32 = -radius; dx <= radius; dx = dx + 1) {
            let sample_coord = coord + vec2<i32>(dx, dy);

            // Verificar bounds
            if (sample_coord.x < 0 || sample_coord.x >= i32(params.image_width) ||
                sample_coord.y < 0 || sample_coord.y >= i32(params.image_height)) {
                continue;
            }

            let sample_mask = textureLoad(mask_texture, sample_coord, 0).r;
            if (sample_mask < 0.5) {
                // Pixel válido encontrado
                let dist = sqrt(f32(dx * dx + dy * dy));
                min_dist = min(min_dist, dist);
            }
        }
    }

    // Calcular peso de blend baseado na distância
    // 0.0 = usar original, 1.0 = usar inpainted
    if (min_dist > params.blend_radius) {
        return 1.0; // Totalmente inpainted (longe da borda)
    }

    // Blend suave usando smoothstep
    return smoothstep(0.0, params.blend_radius, min_dist);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(output_texture);

    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    let coord = vec2<i32>(global_id.xy);

    // Ler valores das texturas
    let original = textureLoad(original_texture, coord, 0);
    let inpainted = textureLoad(inpainted_texture, coord, 0);
    let mask = textureLoad(mask_texture, coord, 0).r;

    // Calcular peso de blend
    let blend_weight = calculate_blend_weight(coord, mask);

    // Misturar original com inpainted
    let result = mix(original, inpainted, blend_weight);

    textureStore(output_texture, coord, result);
}

// Versão simplificada sem blur de borda (mais rápida)
@compute @workgroup_size(16, 16)
fn main_simple(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(output_texture);

    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    let coord = vec2<i32>(global_id.xy);

    let original = textureLoad(original_texture, coord, 0);
    let inpainted = textureLoad(inpainted_texture, coord, 0);
    let mask = textureLoad(mask_texture, coord, 0).r;

    // Seleção direta baseada na máscara
    let result = select(original, inpainted, mask > 0.5);

    textureStore(output_texture, coord, result);
}
