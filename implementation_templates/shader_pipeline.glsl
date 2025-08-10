// Complete Shader Pipeline for Rustation-NG
// Collection of GLSL shaders for various rendering effects

// ============================================================================
// CRT Simulation Shaders
// ============================================================================

// --- CRT Vertex Shader ---
#shader vertex crt_vertex
#version 330 core

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_texcoord;

out vec2 v_texcoord;
out vec2 v_screen_pos;

uniform mat4 u_projection;
uniform vec2 u_resolution;

void main() {
    gl_Position = u_projection * vec4(a_position, 0.0, 1.0);
    v_texcoord = a_texcoord;
    v_screen_pos = a_position;
}

// --- CRT Fragment Shader ---
#shader fragment crt_fragment
#version 330 core

in vec2 v_texcoord;
in vec2 v_screen_pos;

out vec4 FragColor;

uniform sampler2D u_texture;
uniform vec2 u_resolution;
uniform float u_time;

// CRT simulation parameters
uniform float u_scanline_intensity = 0.3;
uniform float u_scanline_thickness = 1.0;
uniform float u_curvature = 0.02;
uniform float u_vignette = 0.3;
uniform float u_phosphor_blur = 0.5;
uniform vec3 u_phosphor_color = vec3(1.0, 0.95, 0.85);

// Apply barrel distortion for CRT curvature
vec2 apply_curvature(vec2 uv) {
    uv = (uv - 0.5) * 2.0;
    uv *= 1.0 + dot(uv, uv) * u_curvature;
    uv = (uv / 2.0) + 0.5;
    return uv;
}

// Generate scanlines
float scanline(vec2 uv) {
    float line = sin(uv.y * u_resolution.y * 3.14159 * u_scanline_thickness);
    return 1.0 - u_scanline_intensity * (1.0 - line * line);
}

// Phosphor persistence simulation
vec3 phosphor_glow(vec3 color) {
    vec3 glow = color * u_phosphor_color;
    glow *= 1.0 + u_phosphor_blur * 0.5;
    return glow;
}

// Vignette effect
float vignette(vec2 uv) {
    uv = (uv - 0.5) * 2.0;
    float v = 1.0 - length(uv);
    return mix(1.0 - u_vignette, 1.0, smoothstep(0.0, 0.7, v));
}

void main() {
    // Apply CRT curvature
    vec2 curved_uv = apply_curvature(v_texcoord);
    
    // Check if we're outside the screen
    if (curved_uv.x < 0.0 || curved_uv.x > 1.0 || 
        curved_uv.y < 0.0 || curved_uv.y > 1.0) {
        FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    
    // Sample the texture
    vec3 color = texture(u_texture, curved_uv).rgb;
    
    // Apply phosphor glow
    color = phosphor_glow(color);
    
    // Apply scanlines
    color *= scanline(curved_uv);
    
    // Apply vignette
    color *= vignette(v_texcoord);
    
    // Add slight flicker (optional)
    color *= 0.98 + 0.02 * sin(u_time * 60.0);
    
    FragColor = vec4(color, 1.0);
}

// ============================================================================
// Anti-Aliasing Shaders (FXAA)
// ============================================================================

// --- FXAA Fragment Shader ---
#shader fragment fxaa_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_texture;
uniform vec2 u_resolution;

// FXAA settings
uniform float u_fxaa_span_max = 8.0;
uniform float u_fxaa_reduce_mul = 1.0 / 8.0;
uniform float u_fxaa_reduce_min = 1.0 / 128.0;

vec3 fxaa(sampler2D tex, vec2 uv, vec2 resolution) {
    vec2 texel = 1.0 / resolution;
    
    // Sample neighboring pixels
    vec3 rgbNW = texture(tex, uv + vec2(-texel.x, -texel.y)).rgb;
    vec3 rgbNE = texture(tex, uv + vec2( texel.x, -texel.y)).rgb;
    vec3 rgbSW = texture(tex, uv + vec2(-texel.x,  texel.y)).rgb;
    vec3 rgbSE = texture(tex, uv + vec2( texel.x,  texel.y)).rgb;
    vec3 rgbM  = texture(tex, uv).rgb;
    
    // Luminance calculation
    vec3 luma = vec3(0.299, 0.587, 0.114);
    float lumaNW = dot(rgbNW, luma);
    float lumaNE = dot(rgbNE, luma);
    float lumaSW = dot(rgbSW, luma);
    float lumaSE = dot(rgbSE, luma);
    float lumaM  = dot(rgbM,  luma);
    
    // Find edge direction
    float lumaMin = min(lumaM, min(min(lumaNW, lumaNE), min(lumaSW, lumaSE)));
    float lumaMax = max(lumaM, max(max(lumaNW, lumaNE), max(lumaSW, lumaSE)));
    float lumaRange = lumaMax - lumaMin;
    
    // Skip if no edge detected
    if (lumaRange < max(u_fxaa_reduce_min, lumaMax * u_fxaa_reduce_mul)) {
        return rgbM;
    }
    
    // Calculate blend direction
    vec2 dir;
    dir.x = -((lumaNW + lumaNE) - (lumaSW + lumaSE));
    dir.y =  ((lumaNW + lumaSW) - (lumaNE + lumaSE));
    
    float dirReduce = max((lumaNW + lumaNE + lumaSW + lumaSE) * (0.25 * u_fxaa_reduce_mul),
                          u_fxaa_reduce_min);
    
    float rcpDirMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + dirReduce);
    dir = min(vec2(u_fxaa_span_max), max(vec2(-u_fxaa_span_max), dir * rcpDirMin)) * texel;
    
    // Perform edge AA
    vec3 rgbA = 0.5 * (
        texture(tex, uv + dir * (1.0 / 3.0 - 0.5)).rgb +
        texture(tex, uv + dir * (2.0 / 3.0 - 0.5)).rgb);
    
    vec3 rgbB = rgbA * 0.5 + 0.25 * (
        texture(tex, uv + dir * -0.5).rgb +
        texture(tex, uv + dir *  0.5).rgb);
    
    float lumaB = dot(rgbB, luma);
    
    if ((lumaB < lumaMin) || (lumaB > lumaMax)) {
        return rgbA;
    } else {
        return rgbB;
    }
}

void main() {
    FragColor = vec4(fxaa(u_texture, v_texcoord, u_resolution), 1.0);
}

// ============================================================================
// xBR Texture Filtering Shader
// ============================================================================

// --- xBR Fragment Shader ---
#shader fragment xbr_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_texture;
uniform vec2 u_texture_size;
uniform float u_scale = 2.0;

// xBR constants
const float XBR_Y_WEIGHT = 48.0;
const float XBR_EQ_THRESHOLD = 15.0;
const vec3 Y_WEIGHT = vec3(0.299, 0.587, 0.114);

float rgb_to_y(vec3 color) {
    return dot(color, Y_WEIGHT);
}

float df(float A, float B) {
    return abs(A - B);
}

bool eq(float A, float B) {
    return df(A, B) < XBR_EQ_THRESHOLD;
}

vec4 xbr_filter(sampler2D tex, vec2 uv, vec2 texture_size) {
    vec2 texel = 1.0 / texture_size;
    vec2 fp = fract(uv * texture_size);
    
    // Sample 3x3 neighborhood
    vec3 A = texture(tex, uv + texel * vec2(-1, -1)).rgb;
    vec3 B = texture(tex, uv + texel * vec2( 0, -1)).rgb;
    vec3 C = texture(tex, uv + texel * vec2( 1, -1)).rgb;
    vec3 D = texture(tex, uv + texel * vec2(-1,  0)).rgb;
    vec3 E = texture(tex, uv).rgb;
    vec3 F = texture(tex, uv + texel * vec2( 1,  0)).rgb;
    vec3 G = texture(tex, uv + texel * vec2(-1,  1)).rgb;
    vec3 H = texture(tex, uv + texel * vec2( 0,  1)).rgb;
    vec3 I = texture(tex, uv + texel * vec2( 1,  1)).rgb;
    
    // Convert to luminance
    float a = rgb_to_y(A), b = rgb_to_y(B), c = rgb_to_y(C);
    float d = rgb_to_y(D), e = rgb_to_y(E), f = rgb_to_y(F);
    float g = rgb_to_y(G), h = rgb_to_y(H), i = rgb_to_y(I);
    
    // Detect edges
    float d1 = df(d, h) + df(h, f) + df(f, b) + df(b, d);
    float d2 = df(b, f) + df(f, h) + df(h, d) + df(d, b);
    
    vec3 color;
    if (d1 < d2) {
        // Diagonal edge pattern 1
        if (fp.x < fp.y) {
            color = mix(E, mix(D, B, 0.5), 0.5);
        } else {
            color = mix(E, mix(F, H, 0.5), 0.5);
        }
    } else {
        // Diagonal edge pattern 2
        if (fp.x + fp.y < 1.0) {
            color = mix(E, mix(D, H, 0.5), 0.5);
        } else {
            color = mix(E, mix(B, F, 0.5), 0.5);
        }
    }
    
    return vec4(color, 1.0);
}

void main() {
    FragColor = xbr_filter(u_texture, v_texcoord, u_texture_size);
}

// ============================================================================
// Motion Blur and Frame Blending
// ============================================================================

// --- Motion Blur Fragment Shader ---
#shader fragment motion_blur_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_current_frame;
uniform sampler2D u_previous_frame;
uniform float u_blur_strength = 0.5;
uniform int u_blur_samples = 4;

void main() {
    vec3 color = texture(u_current_frame, v_texcoord).rgb;
    vec3 prev_color = texture(u_previous_frame, v_texcoord).rgb;
    
    // Simple frame blending
    color = mix(color, prev_color, u_blur_strength);
    
    // Add velocity-based blur if velocity buffer available
    // This would require per-pixel velocity information
    
    FragColor = vec4(color, 1.0);
}

// ============================================================================
// Color Correction and Enhancement
// ============================================================================

// --- Color Correction Fragment Shader ---
#shader fragment color_correction_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_texture;
uniform float u_brightness = 0.0;
uniform float u_contrast = 1.0;
uniform float u_saturation = 1.0;
uniform float u_gamma = 1.0;
uniform vec3 u_color_balance = vec3(1.0, 1.0, 1.0);

vec3 adjust_brightness_contrast(vec3 color, float brightness, float contrast) {
    return (color - 0.5) * contrast + 0.5 + brightness;
}

vec3 adjust_saturation(vec3 color, float saturation) {
    float gray = dot(color, vec3(0.299, 0.587, 0.114));
    return mix(vec3(gray), color, saturation);
}

vec3 apply_gamma(vec3 color, float gamma) {
    return pow(color, vec3(1.0 / gamma));
}

void main() {
    vec3 color = texture(u_texture, v_texcoord).rgb;
    
    // Apply color balance
    color *= u_color_balance;
    
    // Brightness and contrast
    color = adjust_brightness_contrast(color, u_brightness, u_contrast);
    
    // Saturation
    color = adjust_saturation(color, u_saturation);
    
    // Gamma correction
    color = apply_gamma(color, u_gamma);
    
    // Clamp to valid range
    color = clamp(color, 0.0, 1.0);
    
    FragColor = vec4(color, 1.0);
}

// ============================================================================
// Bloom/Glow Effect
// ============================================================================

// --- Bloom Threshold Fragment Shader ---
#shader fragment bloom_threshold_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_texture;
uniform float u_threshold = 0.8;

void main() {
    vec3 color = texture(u_texture, v_texcoord).rgb;
    
    // Extract bright pixels
    float brightness = dot(color, vec3(0.299, 0.587, 0.114));
    
    if (brightness > u_threshold) {
        FragColor = vec4(color, 1.0);
    } else {
        FragColor = vec4(0.0, 0.0, 0.0, 1.0);
    }
}

// --- Gaussian Blur Fragment Shader ---
#shader fragment gaussian_blur_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_texture;
uniform vec2 u_direction; // (1,0) for horizontal, (0,1) for vertical
uniform vec2 u_resolution;

// 9-tap Gaussian kernel
const float kernel[9] = float[](
    0.0162, 0.0540, 0.1216, 0.1945, 0.2270,
    0.1945, 0.1216, 0.0540, 0.0162
);

void main() {
    vec2 texel = 1.0 / u_resolution;
    vec3 color = vec3(0.0);
    
    for (int i = -4; i <= 4; i++) {
        vec2 offset = float(i) * texel * u_direction;
        color += texture(u_texture, v_texcoord + offset).rgb * kernel[i + 4];
    }
    
    FragColor = vec4(color, 1.0);
}

// --- Bloom Combine Fragment Shader ---
#shader fragment bloom_combine_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_original;
uniform sampler2D u_bloom;
uniform float u_bloom_intensity = 0.5;

void main() {
    vec3 original = texture(u_original, v_texcoord).rgb;
    vec3 bloom = texture(u_bloom, v_texcoord).rgb;
    
    // Additive blending
    vec3 color = original + bloom * u_bloom_intensity;
    
    // Tone mapping to prevent over-brightness
    color = color / (1.0 + color);
    
    FragColor = vec4(color, 1.0);
}

// ============================================================================
// Retro/Vintage Effects
// ============================================================================

// --- Retro TV Fragment Shader ---
#shader fragment retro_tv_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_texture;
uniform vec2 u_resolution;
uniform float u_time;

// Retro TV parameters
uniform float u_noise_strength = 0.1;
uniform float u_interference_speed = 5.0;
uniform float u_color_shift = 0.002;
uniform float u_barrel_distortion = 0.1;

// Noise generation
float random(vec2 co) {
    return fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 43758.5453);
}

// Barrel distortion
vec2 barrel_distort(vec2 uv, float amount) {
    vec2 cc = uv - 0.5;
    float dist = length(cc);
    return uv + cc * dist * dist * amount;
}

// TV interference lines
float interference(vec2 uv, float time) {
    return sin(uv.y * 300.0 + time * u_interference_speed) * 0.01;
}

void main() {
    vec2 uv = v_texcoord;
    
    // Apply barrel distortion
    uv = barrel_distort(uv, u_barrel_distortion);
    
    // Add interference
    uv.x += interference(uv, u_time);
    
    // Color channel separation (chromatic aberration)
    float r = texture(u_texture, uv + vec2(u_color_shift, 0.0)).r;
    float g = texture(u_texture, uv).g;
    float b = texture(u_texture, uv - vec2(u_color_shift, 0.0)).b;
    
    vec3 color = vec3(r, g, b);
    
    // Add noise
    color += vec3(random(uv + u_time)) * u_noise_strength;
    
    // Vignette
    float vignette = 1.0 - length((uv - 0.5) * 1.5);
    color *= vignette;
    
    FragColor = vec4(color, 1.0);
}

// ============================================================================
// Sharp Bilinear Filtering (for pixel art)
// ============================================================================

// --- Sharp Bilinear Fragment Shader ---
#shader fragment sharp_bilinear_fragment
#version 330 core

in vec2 v_texcoord;
out vec4 FragColor;

uniform sampler2D u_texture;
uniform vec2 u_texture_size;
uniform float u_sharpness = 0.5;

void main() {
    vec2 texel = 1.0 / u_texture_size;
    vec2 pixel = v_texcoord * u_texture_size;
    vec2 fp = fract(pixel);
    
    // Sharpness curve
    vec2 w = clamp(fp / u_sharpness, 0.0, 1.0);
    w = w * w * (3.0 - 2.0 * w); // Smoothstep
    
    // Sample four corners
    vec2 tc = (floor(pixel) + 0.5) * texel;
    vec4 c00 = texture(u_texture, tc);
    vec4 c10 = texture(u_texture, tc + vec2(texel.x, 0.0));
    vec4 c01 = texture(u_texture, tc + vec2(0.0, texel.y));
    vec4 c11 = texture(u_texture, tc + texel);
    
    // Bilinear interpolation with sharpness
    vec4 color = mix(
        mix(c00, c10, w.x),
        mix(c01, c11, w.x),
        w.y
    );
    
    FragColor = color;
}

// ============================================================================
// Shader Utility Functions
// ============================================================================

// --- Common utility functions that can be included ---
#shader utility common_utils

// SRGB conversion
vec3 srgb_to_linear(vec3 color) {
    return pow(color, vec3(2.2));
}

vec3 linear_to_srgb(vec3 color) {
    return pow(color, vec3(1.0 / 2.2));
}

// Dithering
float dither(vec2 position, float value) {
    int x = int(position.x) % 4;
    int y = int(position.y) % 4;
    
    const float dither_matrix[16] = float[](
         0.0,  8.0,  2.0, 10.0,
        12.0,  4.0, 14.0,  6.0,
         3.0, 11.0,  1.0,  9.0,
        15.0,  7.0, 13.0,  5.0
    );
    
    int index = y * 4 + x;
    return value + (dither_matrix[index] / 16.0 - 0.5) / 32.0;
}

// Color space conversion
vec3 rgb_to_hsv(vec3 color) {
    float cmax = max(color.r, max(color.g, color.b));
    float cmin = min(color.r, min(color.g, color.b));
    float delta = cmax - cmin;
    
    vec3 hsv;
    hsv.z = cmax; // Value
    
    if (cmax != 0.0) {
        hsv.y = delta / cmax; // Saturation
    } else {
        hsv.y = 0.0;
    }
    
    if (delta == 0.0) {
        hsv.x = 0.0; // Hue
    } else if (cmax == color.r) {
        hsv.x = mod((color.g - color.b) / delta, 6.0) / 6.0;
    } else if (cmax == color.g) {
        hsv.x = ((color.b - color.r) / delta + 2.0) / 6.0;
    } else {
        hsv.x = ((color.r - color.g) / delta + 4.0) / 6.0;
    }
    
    return hsv;
}

vec3 hsv_to_rgb(vec3 hsv) {
    float c = hsv.y * hsv.z;
    float x = c * (1.0 - abs(mod(hsv.x * 6.0, 2.0) - 1.0));
    float m = hsv.z - c;
    
    vec3 rgb;
    if (hsv.x < 1.0/6.0) {
        rgb = vec3(c, x, 0.0);
    } else if (hsv.x < 2.0/6.0) {
        rgb = vec3(x, c, 0.0);
    } else if (hsv.x < 3.0/6.0) {
        rgb = vec3(0.0, c, x);
    } else if (hsv.x < 4.0/6.0) {
        rgb = vec3(0.0, x, c);
    } else if (hsv.x < 5.0/6.0) {
        rgb = vec3(x, 0.0, c);
    } else {
        rgb = vec3(c, 0.0, x);
    }
    
    return rgb + m;
}