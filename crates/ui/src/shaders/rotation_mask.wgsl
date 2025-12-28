// Rotation Mask Generation Compute Shader
// Gera uma máscara binária indicando quais pixels estão fora dos bounds
// da imagem rotacionada e precisam ser preenchidos.

struct MaskParams {
    angle_rad: f32,           // Ângulo de rotação em radianos
    crop_x: f32,              // Região de crop (normalizado 0-1)
    crop_y: f32,
    crop_width: f32,
    crop_height: f32,
    image_width: f32,         // Dimensões da imagem
    image_height: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> params: MaskParams;
@group(0) @binding(1) var output_mask: texture_storage_2d<r8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(output_mask);

    // Verificar bounds
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    // Normalizar posição do pixel para 0-1
    let x = f32(global_id.x) / f32(dims.x);
    let y = f32(global_id.y) / f32(dims.y);

    // Verificar se está dentro do crop
    let in_crop = x >= params.crop_x &&
                  x <= params.crop_x + params.crop_width &&
                  y >= params.crop_y &&
                  y <= params.crop_y + params.crop_height;

    if (!in_crop) {
        // Fora do crop - não precisa de fill (será cortado)
        textureStore(output_mask, vec2<i32>(global_id.xy), vec4<f32>(0.0, 0.0, 0.0, 1.0));
        return;
    }

    // Centro da imagem
    let center = vec2<f32>(0.5, 0.5);
    let pos = vec2<f32>(x, y) - center;

    // Correção de aspect ratio para rotação correta
    let aspect = params.image_width / params.image_height;
    let pos_corrected = vec2<f32>(pos.x * aspect, pos.y);

    // Rotação inversa para encontrar posição fonte
    let sin_a = sin(-params.angle_rad);
    let cos_a = cos(-params.angle_rad);
    let rotated = vec2<f32>(
        pos_corrected.x * cos_a - pos_corrected.y * sin_a,
        pos_corrected.x * sin_a + pos_corrected.y * cos_a
    );

    // Restaurar aspect ratio
    let source_pos = vec2<f32>(rotated.x / aspect, rotated.y) + center;

    // Verificar se posição fonte está dentro dos bounds da imagem [0,1]
    let is_valid = source_pos.x >= 0.0 && source_pos.x <= 1.0 &&
                   source_pos.y >= 0.0 && source_pos.y <= 1.0;

    // Valor da máscara: 1.0 = precisa de fill, 0.0 = tem dados válidos
    let mask_value = select(1.0, 0.0, is_valid);

    textureStore(output_mask, vec2<i32>(global_id.xy), vec4<f32>(mask_value, 0.0, 0.0, 1.0));
}
