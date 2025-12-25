// Image Adjustments Compute Shader
// Processes all adjustments in parallel on the GPU

struct Params {
    exposure: f32,
    contrast: f32,
    temperature: f32,
    tint: f32,
    highlights: f32,
    shadows: f32,
    whites: f32,
    blacks: f32,
    clarity: f32,
    vibrance: f32,
    saturation: f32,
    // Tone curve parametric zones
    tone_curve_shadows: f32,
    tone_curve_darks: f32,
    tone_curve_lights: f32,
    tone_curve_highlights: f32,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    
    // Bounds check
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    
    // Load pixel (values in 0.0-1.0 range)
    let pixel = textureLoad(input_texture, vec2<i32>(global_id.xy), 0);
    var r = pixel.r * 255.0;
    var g = pixel.g * 255.0;
    var b = pixel.b * 255.0;
    let a = pixel.a;
    
    // 1. Exposure (brightness adjustment)
    if (params.exposure != 0.0) {
        let factor = pow(2.0, params.exposure);
        r *= factor;
        g *= factor;
        b *= factor;
    }
    
    // 2. Contrast
    if (params.contrast != 1.0) {
        r = (r - 128.0) * params.contrast + 128.0;
        g = (g - 128.0) * params.contrast + 128.0;
        b = (b - 128.0) * params.contrast + 128.0;
    }
    
    // 3. Temperature (warm/cool balance)
    if (params.temperature != 0.0) {
        r += params.temperature * 10.0;
        b -= params.temperature * 10.0;
    }
    
    // 4. Tint (green/magenta balance)
    if (params.tint != 0.0) {
        if (params.tint > 0.0) {
            r += params.tint * 5.0;
            b += params.tint * 5.0;
            g -= params.tint * 5.0;
        } else {
            g -= params.tint * 5.0;
            r += params.tint * 5.0;
            b += params.tint * 5.0;
        }
    }
    
    // Calculate luminance for selective adjustments
    let luminance = (r + g + b) / 3.0;
    
    // 5. Highlights (adjust bright areas)
    if (params.highlights != 0.0 && luminance > 128.0) {
        let factor = 1.0 + (params.highlights * 0.01);
        r *= factor;
        g *= factor;
        b *= factor;
    }
    
    // 6. Shadows (adjust dark areas)
    if (params.shadows != 0.0 && luminance < 128.0) {
        let factor = 1.0 + (params.shadows * 0.01);
        r *= factor;
        g *= factor;
        b *= factor;
    }
    
    // 7. Whites (adjust brightest areas)
    if (params.whites != 0.0 && luminance > 192.0) {
        let factor = 1.0 + (params.whites * 0.01);
        r *= factor;
        g *= factor;
        b *= factor;
    }
    
    // 8. Blacks (adjust darkest areas)
    if (params.blacks != 0.0 && luminance < 64.0) {
        let factor = 1.0 + (params.blacks * 0.01);
        r *= factor;
        g *= factor;
        b *= factor;
    }
    
    // Recalculate luminance after tonal adjustments
    let lum2 = (r + g + b) / 3.0;
    
    // 9. Saturation (overall color intensity)
    if (params.saturation != 0.0) {
        let factor = 1.0 + params.saturation;
        r = lum2 + (r - lum2) * factor;
        g = lum2 + (g - lum2) * factor;
        b = lum2 + (b - lum2) * factor;
    }
    
    // 10. Vibrance (intelligent saturation - affects muted colors more)
    if (params.vibrance != 0.0) {
        let max_diff = max(max(abs(r - lum2), abs(g - lum2)), abs(b - lum2));
        if (max_diff < 64.0) {
            let factor = 1.0 + params.vibrance * 2.0;
            r = lum2 + (r - lum2) * factor;
            g = lum2 + (g - lum2) * factor;
            b = lum2 + (b - lum2) * factor;
        }
    }
    
    // 11. Clarity (local contrast - simplified)
    if (params.clarity != 0.0) {
        let factor = 1.0 + (params.clarity * 0.5);
        let lum3 = (r + g + b) / 3.0;
        r = lum3 + (r - lum3) * factor;
        g = lum3 + (g - lum3) * factor;
        b = lum3 + (b - lum3) * factor;
    }
    
    // 12-15. Tone Curve Parametric Zones
    let lum_final = (r + g + b) / 3.0;
    let norm_lum = lum_final / 255.0;
    
    // Zone 1: Shadows (0.0 - 0.25 range)
    if (params.tone_curve_shadows != 0.0) {
        let zone_center = 0.125;
        let zone_width = 0.25;
        let dist = abs(norm_lum - zone_center);
        if (dist < zone_width) {
            let weight = 1.0 - (dist / zone_width);
            let adjustment = params.tone_curve_shadows * 0.01 * weight;
            r += r * adjustment;
            g += g * adjustment;
            b += b * adjustment;
        }
    }
    
    // Zone 2: Darks (0.25 - 0.5 range)
    if (params.tone_curve_darks != 0.0) {
        let zone_center = 0.375;
        let zone_width = 0.25;
        let dist = abs(norm_lum - zone_center);
        if (dist < zone_width) {
            let weight = 1.0 - (dist / zone_width);
            let adjustment = params.tone_curve_darks * 0.01 * weight;
            r += r * adjustment;
            g += g * adjustment;
            b += b * adjustment;
        }
    }
    
    // Zone 3: Lights (0.5 - 0.75 range)
    if (params.tone_curve_lights != 0.0) {
        let zone_center = 0.625;
        let zone_width = 0.25;
        let dist = abs(norm_lum - zone_center);
        if (dist < zone_width) {
            let weight = 1.0 - (dist / zone_width);
            let adjustment = params.tone_curve_lights * 0.01 * weight;
            r += r * adjustment;
            g += g * adjustment;
            b += b * adjustment;
        }
    }
    
    // Zone 4: Highlights (0.75 - 1.0 range)
    if (params.tone_curve_highlights != 0.0) {
        let zone_center = 0.875;
        let zone_width = 0.25;
        let dist = abs(norm_lum - zone_center);
        if (dist < zone_width) {
            let weight = 1.0 - (dist / zone_width);
            let adjustment = params.tone_curve_highlights * 0.01 * weight;
            r += r * adjustment;
            g += g * adjustment;
            b += b * adjustment;
        }
    }
    
    // Clamp values to 0-255 and convert back to 0.0-1.0
    r = clamp(r, 0.0, 255.0) / 255.0;
    g = clamp(g, 0.0, 255.0) / 255.0;
    b = clamp(b, 0.0, 255.0) / 255.0;
    
    // Write output
    textureStore(output_texture, vec2<i32>(global_id.xy), vec4<f32>(r, g, b, a));
}

