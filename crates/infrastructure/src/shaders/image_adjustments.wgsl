// Image Adjustments Compute Shader
// Processes all adjustments in parallel on the GPU

// 🚨 A ordem aqui é a do `Ajustes` do Rust, campo a campo.
//
// O `uniform` viaja como bytes crus e casa por **posição**, não por nome. Este
// bloco declarava 28 campos para os 46 que a CPU manda: a partir da posição 23
// o shader lia o campo do vizinho (o matiz do vermelho virava redução de ruído)
// e da 28 em diante não lia nada — os 4 controles de Detalhe e os 3 de Lente
// não chegavam. Nada falhava: o buffer é maior que o mínimo do binding, então o
// wgpu aceita e ignora a sobra, e a duplicata de `nr_luminance` o naga aceitava.
//
// Quem confere os dois lados é `o_wgsl_declara_os_mesmos_46_campos_na_mesma_ordem`,
// que lê este arquivo e compara com a struct.
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
    // HSL color channel saturations (-100 to +100)
    hsl_red_sat: f32,
    hsl_orange_sat: f32,
    hsl_yellow_sat: f32,
    hsl_green_sat: f32,
    hsl_aqua_sat: f32,
    hsl_blue_sat: f32,
    hsl_purple_sat: f32,
    hsl_magenta_sat: f32,
    // HSL hue rotation (-180 to +180) — declarados, ainda sem código no corpo
    hsl_red_hue: f32,
    hsl_orange_hue: f32,
    hsl_yellow_hue: f32,
    hsl_green_hue: f32,
    hsl_aqua_hue: f32,
    hsl_blue_hue: f32,
    hsl_purple_hue: f32,
    hsl_magenta_hue: f32,
    // HSL luminance (-100 to +100) — idem
    hsl_red_lum: f32,
    hsl_orange_lum: f32,
    hsl_yellow_lum: f32,
    hsl_green_lum: f32,
    hsl_aqua_lum: f32,
    hsl_blue_lum: f32,
    hsl_purple_lum: f32,
    hsl_magenta_lum: f32,
    // Lens corrections — idem
    lens_distortion: f32,
    lens_vignette_amount: f32,
    lens_vignette_midpoint: f32,
    // Detail: noise reduction and sharpening
    nr_luminance: f32,
    nr_color: f32,
    sharpen_amount: f32,
    sharpen_radius: f32,
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
    
    // 0. Noise Reduction & Sharpening
    // We combine NR and Sharpening (USM) in a single neighborhood loop for efficiency
    let do_nr_lum = params.nr_luminance > 0.0;
    let do_nr_col = params.nr_color > 0.0;
    let do_sharpen = params.sharpen_amount > 0.0;
    
    if (do_nr_lum || do_nr_col || do_sharpen) {
        var sum_r_lum = 0.0;
        var sum_g_lum = 0.0;
        var sum_b_lum = 0.0;
        var sum_weight_lum = 0.0;
        
        var sum_u_col = 0.0;
        var sum_v_col = 0.0;
        var sum_weight_col = 0.0;
        
        var sum_y_sharpen = 0.0;
        var sum_weight_sharpen = 0.0;
        
        // Calculate center luminance and YUV
        let center_y = 0.299 * r + 0.587 * g + 0.114 * b;
        let center_lum_norm = center_y / 255.0; 
        
        // Params
        let sigma_s = 2.0; // NR Spatial
        let sigma_r = max(params.nr_luminance * 0.002, 0.0001); // NR Range
        let sigma_color = max(params.nr_color * 0.05, 0.1); // Color NR
        let sigma_sharpen = max(params.sharpen_radius, 0.5); // Sharpen Radius
        
        // 5x5 Kernel
        for (var dy: i32 = -2; dy <= 2; dy++) {
            for (var dx: i32 = -2; dx <= 2; dx++) {
                let nx = i32(global_id.x) + dx;
                let ny = i32(global_id.y) + dy;
                
                if (nx >= 0 && ny >= 0 && nx < i32(dims.x) && ny < i32(dims.y)) {
                    let neighbor = textureLoad(input_texture, vec2<i32>(nx, ny), 0);
                    let nr = neighbor.r * 255.0;
                    let ng = neighbor.g * 255.0;
                    let nb = neighbor.b * 255.0;
                    
                    let dist_sq = f32(dx*dx + dy*dy);
                    let n_lum = (0.299 * nr + 0.587 * ng + 0.114 * nb) / 255.0;
                    
                    // --- Luminance NR (Bilateral) ---
                    if (do_nr_lum) {
                        let diff = center_lum_norm - n_lum;
                        let weight = exp(-dist_sq / (2.0 * sigma_s * sigma_s)) * 
                                     exp(-(diff * diff) / (2.0 * sigma_r * sigma_r));
                        sum_r_lum += nr * weight;
                        sum_g_lum += ng * weight;
                        sum_b_lum += nb * weight;
                        sum_weight_lum += weight;
                    }
                    
                    // --- Color NR (Gaussian on U/V) ---
                    if (do_nr_col) {
                         let nu = -0.147 * nr - 0.289 * ng + 0.436 * nb;
                         let nv = 0.615 * nr - 0.515 * ng - 0.100 * nb;
                         let weight = exp(-dist_sq / (2.0 * sigma_color * sigma_color));
                         sum_u_col += nu * weight;
                         sum_v_col += nv * weight;
                         sum_weight_col += weight;
                    }
                    
                    // --- Sharpening (Gaussian on Y) ---
                    if (do_sharpen) {
                        // Gaussian blur for USM
                        let weight = exp(-dist_sq / (2.0 * sigma_sharpen * sigma_sharpen));
                        let ny_val = 0.299 * nr + 0.587 * ng + 0.114 * nb;
                        sum_y_sharpen += ny_val * weight;
                        sum_weight_sharpen += weight;
                    }
                }
            }
        }
        
        // --- Recombine NR Results ---
        var final_y = center_y;
        
        if (do_nr_lum && sum_weight_lum > 0.0) {
            let fr = sum_r_lum / sum_weight_lum;
            let fg = sum_g_lum / sum_weight_lum;
            let fb = sum_b_lum / sum_weight_lum;
            final_y = 0.299 * fr + 0.587 * fg + 0.114 * fb;
        }
        
        var final_u = -0.147 * r - 0.289 * g + 0.436 * b;
        var final_v = 0.615 * r - 0.515 * g - 0.100 * b;
        
        if (do_nr_col && sum_weight_col > 0.0) {
            final_u = sum_u_col / sum_weight_col;
            final_v = sum_v_col / sum_weight_col;
        }
        
        // Apply NR changes to r,g,b
        if (do_nr_lum || do_nr_col) {
            r = final_y + 1.140 * final_v;
            g = final_y - 0.395 * final_u - 0.581 * final_v;
            b = final_y + 2.032 * final_u;
        }
        
        // --- Apply Sharpening (USM) ---
        if (do_sharpen && sum_weight_sharpen > 0.0) {
            let blurred_y = sum_y_sharpen / sum_weight_sharpen;
            let detail = final_y - blurred_y;
            let amount = params.sharpen_amount * 0.05; // Scale 0-100 to approx 0-5
            
            // Add detail back to RGB channels
            r += detail * amount;
            g += detail * amount;
            b += detail * amount;
        }
        
    }
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
    
    // HSL Color Channel Saturation Adjustments
    let has_hsl = params.hsl_red_sat != 0.0 || params.hsl_orange_sat != 0.0 
        || params.hsl_yellow_sat != 0.0 || params.hsl_green_sat != 0.0
        || params.hsl_aqua_sat != 0.0 || params.hsl_blue_sat != 0.0
        || params.hsl_purple_sat != 0.0 || params.hsl_magenta_sat != 0.0;
    
    if (has_hsl) {
        // Normalize RGB to 0-1 range
        let r_norm = r / 255.0;
        let g_norm = g / 255.0;
        let b_norm = b / 255.0;
        
        let max_c = max(max(r_norm, g_norm), b_norm);
        let min_c = min(min(r_norm, g_norm), b_norm);
        let delta = max_c - min_c;
        
        // Calculate hue (0-360 degrees)
        var hue: f32 = 0.0;
        if (delta != 0.0) {
            if (max_c == r_norm) {
                hue = 60.0 * (((g_norm - b_norm) / delta) % 6.0);
            } else if (max_c == g_norm) {
                hue = 60.0 * (((b_norm - r_norm) / delta) + 2.0);
            } else {
                hue = 60.0 * (((r_norm - g_norm) / delta) + 4.0);
            }
        }
        if (hue < 0.0) { hue += 360.0; }
        
        // Calculate lightness and saturation
        let lightness = (max_c + min_c) / 2.0;
        var sat: f32 = 0.0;
        if (delta != 0.0) {
            sat = delta / (1.0 - abs(2.0 * lightness - 1.0));
        }
        
        // Determine HSL adjustment based on hue range
        var sat_adjustment: f32 = 0.0;
        
        // Red (wraps around 0): 345-360, 0-15
        if (hue >= 345.0 || hue < 15.0) {
            var dist_hue: f32;
            if (hue >= 345.0) { dist_hue = hue - 360.0; } else { dist_hue = hue; }
            let weight = 1.0 - min(abs(dist_hue) / 15.0, 1.0);
            sat_adjustment += params.hsl_red_sat * weight;
        }
        // Orange: 15-45
        if (hue >= 15.0 && hue < 45.0) {
            let weight = 1.0 - min(abs(hue - 30.0) / 15.0, 1.0);
            sat_adjustment += params.hsl_orange_sat * weight;
        }
        // Yellow: 45-75
        if (hue >= 45.0 && hue < 75.0) {
            let weight = 1.0 - min(abs(hue - 60.0) / 15.0, 1.0);
            sat_adjustment += params.hsl_yellow_sat * weight;
        }
        // Green: 75-165
        if (hue >= 75.0 && hue < 165.0) {
            let weight = 1.0 - min(abs(hue - 120.0) / 45.0, 1.0);
            sat_adjustment += params.hsl_green_sat * weight;
        }
        // Aqua: 165-210
        if (hue >= 165.0 && hue < 210.0) {
            let weight = 1.0 - min(abs(hue - 187.5) / 22.5, 1.0);
            sat_adjustment += params.hsl_aqua_sat * weight;
        }
        // Blue: 210-270
        if (hue >= 210.0 && hue < 270.0) {
            let weight = 1.0 - min(abs(hue - 240.0) / 30.0, 1.0);
            sat_adjustment += params.hsl_blue_sat * weight;
        }
        // Purple: 270-310
        if (hue >= 270.0 && hue < 310.0) {
            let weight = 1.0 - min(abs(hue - 290.0) / 20.0, 1.0);
            sat_adjustment += params.hsl_purple_sat * weight;
        }
        // Magenta: 310-345
        if (hue >= 310.0 && hue < 345.0) {
            let weight = 1.0 - min(abs(hue - 327.5) / 17.5, 1.0);
            sat_adjustment += params.hsl_magenta_sat * weight;
        }
        
        sat_adjustment *= 0.01; // Convert from -100..100 to -1..1
        
        // Apply saturation adjustment and convert back to RGB
        if (sat_adjustment != 0.0) {
            let new_sat = clamp(sat + sat_adjustment * sat, 0.0, 1.0);
            
            // HSL to RGB conversion
            let c = (1.0 - abs(2.0 * lightness - 1.0)) * new_sat;
            let x = c * (1.0 - abs((hue / 60.0) % 2.0 - 1.0));
            let m = lightness - c / 2.0;
            
            var r1: f32 = 0.0;
            var g1: f32 = 0.0;
            var b1: f32 = 0.0;
            
            if (hue < 60.0) {
                r1 = c; g1 = x; b1 = 0.0;
            } else if (hue < 120.0) {
                r1 = x; g1 = c; b1 = 0.0;
            } else if (hue < 180.0) {
                r1 = 0.0; g1 = c; b1 = x;
            } else if (hue < 240.0) {
                r1 = 0.0; g1 = x; b1 = c;
            } else if (hue < 300.0) {
                r1 = x; g1 = 0.0; b1 = c;
            } else {
                r1 = c; g1 = 0.0; b1 = x;
            }
            
            r = (r1 + m) * 255.0;
            g = (g1 + m) * 255.0;
            b = (b1 + m) * 255.0;
        }
    }
    
    // Clamp values to 0-255 and convert back to 0.0-1.0
    r = clamp(r, 0.0, 255.0) / 255.0;
    g = clamp(g, 0.0, 255.0) / 255.0;
    b = clamp(b, 0.0, 255.0) / 255.0;
    
    // Write output
    textureStore(output_texture, vec2<i32>(global_id.xy), vec4<f32>(r, g, b, a));
}

