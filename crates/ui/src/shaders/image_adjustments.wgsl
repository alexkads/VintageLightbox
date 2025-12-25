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
    // HSL color channel saturations (-100 to +100)
    hsl_red_sat: f32,
    hsl_orange_sat: f32,
    hsl_yellow_sat: f32,
    hsl_green_sat: f32,
    hsl_aqua_sat: f32,
    hsl_blue_sat: f32,
    hsl_purple_sat: f32,
    hsl_magenta_sat: f32,
    nr_luminance: f32,
    nr_color: f32,
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
    
    // 0. Noise Reduction (Bilateral Filter on Luminance)
    // Applied before other adjustments to avoid amplifying noise
    if (params.nr_luminance > 0.0) {
        var sum_r = 0.0;
        var sum_g = 0.0;
        var sum_b = 0.0;
        var sum_weight = 0.0;
        
        // Calculate center luminance (0-1 range from loaded pixel)
        let center_lum = (pixel.r + pixel.g + pixel.b) / 3.0;
        
        let sigma_s = 2.0; // Spatial sigma (fixed spatial weighted window)
        // Map 0-100 UI range to useful sigma_r range (e.g., 0.0 to 0.2 intensity difference)
        // If nr_luminance is 100, sigma_r = 0.1?
        // Let's try 0.2 at max.
        let sigma_r = params.nr_luminance * 0.002; 
        
        // 5x5 Kernel
        for (var dy: i32 = -2; dy <= 2; dy++) {
            for (var dx: i32 = -2; dx <= 2; dx++) {
                let nx = i32(global_id.x) + dx;
                let ny = i32(global_id.y) + dy;
                
                // Bounds check
                if (nx >= 0 && ny >= 0 && nx < i32(dims.x) && ny < i32(dims.y)) {
                    let neighbor = textureLoad(input_texture, vec2<i32>(nx, ny), 0);
                    let neighbor_lum = (neighbor.r + neighbor.g + neighbor.b) / 3.0;
                    
                    let diff = center_lum - neighbor_lum;
                    let dist_sq = f32(dx*dx + dy*dy);
                    
                    let weight = exp(-dist_sq / (2.0 * sigma_s * sigma_s)) * 
                                 exp(-(diff * diff) / (2.0 * sigma_r * sigma_r + 0.00001));
                                 
                    sum_r += neighbor.r * weight;
                    sum_g += neighbor.g * weight;
                    sum_b += neighbor.b * weight;
                    sum_weight += weight;
                }
            }
        }
        
        if (sum_weight > 0.0) {
            r = (sum_r / sum_weight) * 255.0;
            g = (sum_g / sum_weight) * 255.0;
            b = (sum_b / sum_weight) * 255.0;
        } else {
             // Fallback to original
             r = pixel.r * 255.0;
             g = pixel.g * 255.0;
             b = pixel.b * 255.0;
        }
    } else {
        r = pixel.r * 255.0;
        g = pixel.g * 255.0;
        b = pixel.b * 255.0;
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

