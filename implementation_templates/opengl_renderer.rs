// OpenGL Hardware Renderer Architecture for Rustation-NG
// This file provides a complete architecture for hardware-accelerated rendering

use gl;
use std::collections::HashMap;
use std::sync::mpsc::{Sender, Receiver};

// ============================================================================
// Renderer Backend Abstraction
// ============================================================================

/// Trait for implementing different rendering backends
pub trait RendererBackend: Send {
    /// Initialize the renderer with given resolution
    fn initialize(&mut self, width: u32, height: u32) -> Result<()>;
    
    /// Submit a batch of drawing commands
    fn submit_commands(&mut self, commands: &[DrawCommand]) -> Result<()>;
    
    /// Present the rendered frame
    fn present(&mut self) -> Result<Frame>;
    
    /// Update renderer settings
    fn update_settings(&mut self, settings: &RendererSettings) -> Result<()>;
    
    /// Get performance statistics
    fn get_stats(&self) -> RenderStats;
    
    /// Cleanup resources
    fn shutdown(&mut self);
}

// ============================================================================
// OpenGL Renderer Implementation
// ============================================================================

pub struct OpenGLRenderer {
    // Core OpenGL objects
    context: GLContext,
    framebuffers: FramebufferCache,
    shaders: ShaderCache,
    textures: TextureCache,
    vertex_buffers: VertexBufferPool,
    
    // Rendering state
    current_framebuffer: GLuint,
    internal_resolution: Resolution,
    output_resolution: Resolution,
    vram_texture: GLuint,
    
    // Command batching
    command_buffer: Vec<DrawCommand>,
    vertex_buffer: Vec<Vertex>,
    index_buffer: Vec<u16>,
    
    // Performance tracking
    stats: RenderStats,
    frame_timer: FrameTimer,
    
    // Settings
    settings: RendererSettings,
}

impl OpenGLRenderer {
    pub fn new() -> Result<Self> {
        let context = GLContext::create()?;
        
        Ok(OpenGLRenderer {
            context,
            framebuffers: FramebufferCache::new(),
            shaders: ShaderCache::new(),
            textures: TextureCache::new(),
            vertex_buffers: VertexBufferPool::new(),
            current_framebuffer: 0,
            internal_resolution: Resolution::default(),
            output_resolution: Resolution::default(),
            vram_texture: 0,
            command_buffer: Vec::with_capacity(1024),
            vertex_buffer: Vec::with_capacity(4096),
            index_buffer: Vec::with_capacity(6144),
            stats: RenderStats::default(),
            frame_timer: FrameTimer::new(),
            settings: RendererSettings::default(),
        })
    }
    
    /// Initialize VRAM texture (1024x512 for PSX)
    fn init_vram_texture(&mut self) -> Result<()> {
        unsafe {
            gl::GenTextures(1, &mut self.vram_texture);
            gl::BindTexture(gl::TEXTURE_2D, self.vram_texture);
            
            // PSX VRAM is 1024x512 16-bit pixels
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as i32,
                1024,
                512,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
            
            // Set texture parameters
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        }
        
        Ok(())
    }
    
    /// Batch draw commands for efficient rendering
    fn batch_commands(&mut self, commands: &[DrawCommand]) {
        self.command_buffer.clear();
        
        for cmd in commands {
            // Group similar commands together
            if let Some(last) = self.command_buffer.last_mut() {
                if last.can_merge(cmd) {
                    last.merge(cmd);
                    continue;
                }
            }
            
            self.command_buffer.push(cmd.clone());
        }
    }
    
    /// Execute batched draw commands
    fn execute_commands(&mut self) -> Result<()> {
        for cmd in &self.command_buffer {
            match cmd {
                DrawCommand::Triangle(tri) => self.draw_triangle(tri)?,
                DrawCommand::Rectangle(rect) => self.draw_rectangle(rect)?,
                DrawCommand::Line(line) => self.draw_line(line)?,
                DrawCommand::Fill(fill) => self.fill_rect(fill)?,
                DrawCommand::Copy(copy) => self.copy_rect(copy)?,
            }
        }
        
        Ok(())
    }
    
    /// Draw a triangle with proper PSX attributes
    fn draw_triangle(&mut self, tri: &TriangleCommand) -> Result<()> {
        let shader = self.shaders.get_shader(tri.get_shader_type())?;
        shader.bind();
        
        // Setup uniforms
        shader.set_uniform_matrix4("u_projection", &self.get_projection_matrix());
        shader.set_uniform_int("u_texture", 0);
        shader.set_uniform_bool("u_textured", tri.textured);
        shader.set_uniform_bool("u_gouraud", tri.gouraud);
        shader.set_uniform_int("u_blend_mode", tri.blend_mode as i32);
        
        // Build vertex data
        self.vertex_buffer.clear();
        for vertex in &tri.vertices {
            self.vertex_buffer.push(Vertex {
                position: [vertex.x as f32, vertex.y as f32],
                color: vertex.color.to_float_rgba(),
                texcoord: [vertex.u as f32 / 256.0, vertex.v as f32 / 256.0],
                texture_page: tri.texture_page,
                clut: tri.clut,
            });
        }
        
        // Upload and draw
        let vbo = self.vertex_buffers.get_buffer(self.vertex_buffer.len())?;
        vbo.upload(&self.vertex_buffer);
        vbo.draw(gl::TRIANGLES, self.vertex_buffer.len());
        
        self.stats.triangles_drawn += 1;
        Ok(())
    }
    
    /// Internal resolution scaling support
    fn create_scaled_framebuffer(&mut self, scale: u32) -> Result<GLuint> {
        let width = 1024 * scale;
        let height = 512 * scale;
        
        let fbo = self.framebuffers.create_framebuffer(width, height)?;
        
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            
            // Create color attachment
            let mut color_texture = 0;
            gl::GenTextures(1, &mut color_texture);
            gl::BindTexture(gl::TEXTURE_2D, color_texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
            
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                color_texture,
                0,
            );
            
            // Check completeness
            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                return Err(PsxError::OpenGLError("Framebuffer incomplete".into()));
            }
        }
        
        Ok(fbo)
    }
}

impl RendererBackend for OpenGLRenderer {
    fn initialize(&mut self, width: u32, height: u32) -> Result<()> {
        self.output_resolution = Resolution { width, height };
        
        // Initialize OpenGL state
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::SCISSOR_TEST);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        }
        
        // Initialize resources
        self.init_vram_texture()?;
        self.shaders.load_all_shaders()?;
        
        // Create scaled framebuffer based on settings
        let scale = self.settings.internal_resolution_scale;
        self.current_framebuffer = self.create_scaled_framebuffer(scale)?;
        
        Ok(())
    }
    
    fn submit_commands(&mut self, commands: &[DrawCommand]) -> Result<()> {
        self.frame_timer.start_frame();
        
        // Batch commands for efficiency
        self.batch_commands(commands);
        
        // Bind framebuffer for internal resolution
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.current_framebuffer);
            gl::Viewport(
                0, 
                0, 
                self.internal_resolution.width as i32,
                self.internal_resolution.height as i32
            );
        }
        
        // Execute draw commands
        self.execute_commands()?;
        
        self.frame_timer.end_frame();
        self.stats.frame_time = self.frame_timer.average_frame_time();
        
        Ok(())
    }
    
    fn present(&mut self) -> Result<Frame> {
        // Resolve internal resolution to output resolution
        self.resolve_framebuffer()?;
        
        // Read pixels for libretro
        let mut pixels = vec![0u8; (self.output_resolution.width * self.output_resolution.height * 4) as usize];
        
        unsafe {
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.current_framebuffer);
            gl::ReadPixels(
                0,
                0,
                self.output_resolution.width as i32,
                self.output_resolution.height as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                pixels.as_mut_ptr() as *mut _,
            );
        }
        
        Ok(Frame {
            width: self.output_resolution.width,
            height: self.output_resolution.height,
            pixels,
        })
    }
    
    fn update_settings(&mut self, settings: &RendererSettings) -> Result<()> {
        // Update internal resolution if changed
        if settings.internal_resolution_scale != self.settings.internal_resolution_scale {
            self.current_framebuffer = self.create_scaled_framebuffer(settings.internal_resolution_scale)?;
        }
        
        // Update texture filtering
        if settings.texture_filtering != self.settings.texture_filtering {
            self.update_texture_filtering(settings.texture_filtering)?;
        }
        
        self.settings = settings.clone();
        Ok(())
    }
    
    fn get_stats(&self) -> RenderStats {
        self.stats.clone()
    }
    
    fn shutdown(&mut self) {
        self.framebuffers.cleanup();
        self.shaders.cleanup();
        self.textures.cleanup();
        self.vertex_buffers.cleanup();
        
        unsafe {
            if self.vram_texture != 0 {
                gl::DeleteTextures(1, &self.vram_texture);
            }
        }
    }
}

// ============================================================================
// Shader Management
// ============================================================================

pub struct ShaderCache {
    shaders: HashMap<ShaderType, Shader>,
}

impl ShaderCache {
    pub fn new() -> Self {
        ShaderCache {
            shaders: HashMap::new(),
        }
    }
    
    pub fn load_all_shaders(&mut self) -> Result<()> {
        // Load and compile all shader variants
        self.load_shader(ShaderType::Flat, FLAT_VERTEX_SHADER, FLAT_FRAGMENT_SHADER)?;
        self.load_shader(ShaderType::Gouraud, GOURAUD_VERTEX_SHADER, GOURAUD_FRAGMENT_SHADER)?;
        self.load_shader(ShaderType::Textured, TEXTURED_VERTEX_SHADER, TEXTURED_FRAGMENT_SHADER)?;
        self.load_shader(ShaderType::TexturedGouraud, TEXTURED_GOURAUD_VERTEX, TEXTURED_GOURAUD_FRAGMENT)?;
        
        Ok(())
    }
    
    fn load_shader(&mut self, shader_type: ShaderType, vertex_src: &str, fragment_src: &str) -> Result<()> {
        let shader = Shader::compile(vertex_src, fragment_src)?;
        self.shaders.insert(shader_type, shader);
        Ok(())
    }
    
    pub fn get_shader(&self, shader_type: ShaderType) -> Result<&Shader> {
        self.shaders.get(&shader_type)
            .ok_or_else(|| PsxError::OpenGLError(format!("Shader {:?} not loaded", shader_type)))
    }
    
    pub fn cleanup(&mut self) {
        for shader in self.shaders.values_mut() {
            shader.delete();
        }
        self.shaders.clear();
    }
}

// ============================================================================
// Shader Source Code
// ============================================================================

const FLAT_VERTEX_SHADER: &str = r#"
#version 330 core

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec4 a_color;

uniform mat4 u_projection;

out vec4 v_color;

void main() {
    gl_Position = u_projection * vec4(a_position, 0.0, 1.0);
    v_color = a_color;
}
"#;

const FLAT_FRAGMENT_SHADER: &str = r#"
#version 330 core

in vec4 v_color;
out vec4 FragColor;

uniform int u_blend_mode;

void main() {
    FragColor = v_color;
    
    // PSX blend modes
    if (u_blend_mode == 1) {
        // Additive blending
        FragColor.rgb *= FragColor.a;
    } else if (u_blend_mode == 2) {
        // Subtractive blending
        FragColor.rgb = vec3(1.0) - FragColor.rgb;
        FragColor.rgb *= FragColor.a;
    }
}
"#;

const TEXTURED_VERTEX_SHADER: &str = r#"
#version 330 core

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec4 a_color;
layout(location = 2) in vec2 a_texcoord;
layout(location = 3) in int a_texture_page;
layout(location = 4) in int a_clut;

uniform mat4 u_projection;

out vec4 v_color;
out vec2 v_texcoord;
flat out int v_texture_page;
flat out int v_clut;

void main() {
    gl_Position = u_projection * vec4(a_position, 0.0, 1.0);
    v_color = a_color;
    v_texcoord = a_texcoord;
    v_texture_page = a_texture_page;
    v_clut = a_clut;
}
"#;

const TEXTURED_FRAGMENT_SHADER: &str = r#"
#version 330 core

in vec4 v_color;
in vec2 v_texcoord;
flat in int v_texture_page;
flat in int v_clut;

out vec4 FragColor;

uniform sampler2D u_vram;
uniform int u_blend_mode;
uniform bool u_texture_window;
uniform ivec4 u_texture_window_params; // x, y, width, height

vec4 sample_texture(vec2 coord) {
    // Calculate VRAM coordinates based on texture page
    int page_x = (v_texture_page & 0xF) * 64;
    int page_y = ((v_texture_page >> 4) & 1) * 256;
    
    // Apply texture window if enabled
    if (u_texture_window) {
        coord.x = mod(coord.x, u_texture_window_params.z) + u_texture_window_params.x;
        coord.y = mod(coord.y, u_texture_window_params.w) + u_texture_window_params.y;
    }
    
    vec2 vram_coord = vec2(page_x, page_y) + coord * 256.0;
    vram_coord /= vec2(1024.0, 512.0); // Normalize to VRAM dimensions
    
    vec4 texel = texture(u_vram, vram_coord);
    
    // Handle transparency (black = transparent in PSX)
    if (texel.rgb == vec3(0.0)) {
        discard;
    }
    
    return texel;
}

void main() {
    vec4 texel = sample_texture(v_texcoord);
    
    // Modulate with vertex color
    FragColor = texel * v_color;
    
    // Apply blend mode
    if (u_blend_mode == 1) {
        FragColor.rgb *= FragColor.a;
    } else if (u_blend_mode == 2) {
        FragColor.rgb = vec3(1.0) - FragColor.rgb;
        FragColor.rgb *= FragColor.a;
    }
}
"#;

const GOURAUD_VERTEX_SHADER: &str = r#"
#version 330 core

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec4 a_color;

uniform mat4 u_projection;

out vec4 v_color;

void main() {
    gl_Position = u_projection * vec4(a_position, 0.0, 1.0);
    v_color = a_color; // Color will be interpolated across triangle
}
"#;

const GOURAUD_FRAGMENT_SHADER: &str = r#"
#version 330 core

in vec4 v_color; // Interpolated color from vertices
out vec4 FragColor;

uniform int u_blend_mode;
uniform bool u_dithering;

// PSX dithering pattern (4x4 Bayer matrix)
const float dither_matrix[16] = float[16](
     0.0,  8.0,  2.0, 10.0,
    12.0,  4.0, 14.0,  6.0,
     3.0, 11.0,  1.0,  9.0,
    15.0,  7.0, 13.0,  5.0
);

void main() {
    vec4 color = v_color;
    
    // Apply dithering if enabled
    if (u_dithering) {
        ivec2 pos = ivec2(gl_FragCoord.xy) % 4;
        int index = pos.y * 4 + pos.x;
        float dither = (dither_matrix[index] / 16.0 - 0.5) / 32.0;
        color.rgb += dither;
    }
    
    FragColor = color;
    
    // Apply blend mode
    if (u_blend_mode == 1) {
        FragColor.rgb *= FragColor.a;
    } else if (u_blend_mode == 2) {
        FragColor.rgb = vec3(1.0) - FragColor.rgb;
        FragColor.rgb *= FragColor.a;
    }
}
"#;

const TEXTURED_GOURAUD_VERTEX: &str = TEXTURED_VERTEX_SHADER;

const TEXTURED_GOURAUD_FRAGMENT: &str = r#"
#version 330 core

in vec4 v_color;
in vec2 v_texcoord;
flat in int v_texture_page;
flat in int v_clut;

out vec4 FragColor;

uniform sampler2D u_vram;
uniform int u_blend_mode;
uniform bool u_dithering;

// Include texture sampling function from textured shader
vec4 sample_texture(vec2 coord) {
    int page_x = (v_texture_page & 0xF) * 64;
    int page_y = ((v_texture_page >> 4) & 1) * 256;
    
    vec2 vram_coord = vec2(page_x, page_y) + coord * 256.0;
    vram_coord /= vec2(1024.0, 512.0);
    
    vec4 texel = texture(u_vram, vram_coord);
    
    if (texel.rgb == vec3(0.0)) {
        discard;
    }
    
    return texel;
}

const float dither_matrix[16] = float[16](
     0.0,  8.0,  2.0, 10.0,
    12.0,  4.0, 14.0,  6.0,
     3.0, 11.0,  1.0,  9.0,
    15.0,  7.0, 13.0,  5.0
);

void main() {
    vec4 texel = sample_texture(v_texcoord);
    vec4 color = texel * v_color; // Gouraud shading
    
    if (u_dithering) {
        ivec2 pos = ivec2(gl_FragCoord.xy) % 4;
        int index = pos.y * 4 + pos.x;
        float dither = (dither_matrix[index] / 16.0 - 0.5) / 32.0;
        color.rgb += dither;
    }
    
    FragColor = color;
    
    if (u_blend_mode == 1) {
        FragColor.rgb *= FragColor.a;
    } else if (u_blend_mode == 2) {
        FragColor.rgb = vec3(1.0) - FragColor.rgb;
        FragColor.rgb *= FragColor.a;
    }
}
"#;

// ============================================================================
// Supporting Types and Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct RendererSettings {
    pub internal_resolution_scale: u32,  // 1x, 2x, 4x, 8x, 16x
    pub texture_filtering: TextureFilter,
    pub anti_aliasing: AntiAliasing,
    pub vsync: bool,
    pub dithering: bool,
    pub true_color: bool,  // 24-bit color mode
    pub widescreen_hack: bool,
    pub pgxp_enabled: bool,  // Perspective-correct texturing
}

impl Default for RendererSettings {
    fn default() -> Self {
        RendererSettings {
            internal_resolution_scale: 1,
            texture_filtering: TextureFilter::Nearest,
            anti_aliasing: AntiAliasing::None,
            vsync: true,
            dithering: true,
            true_color: false,
            widescreen_hack: false,
            pgxp_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureFilter {
    Nearest,
    Bilinear,
    xBR,
    SABR,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AntiAliasing {
    None,
    FXAA,
    SMAA,
    MSAA2x,
    MSAA4x,
    MSAA8x,
}

#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub frame_time: f32,
    pub draw_calls: u32,
    pub triangles_drawn: u32,
    pub pixels_rendered: u64,
    pub texture_cache_hits: u32,
    pub texture_cache_misses: u32,
    pub vram_reads: u32,
    pub vram_writes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderType {
    Flat,
    Gouraud,
    Textured,
    TexturedGouraud,
}

#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub texcoord: [f32; 2],
    pub texture_page: u16,
    pub clut: u16,
}

#[derive(Debug, Clone)]
pub enum DrawCommand {
    Triangle(TriangleCommand),
    Rectangle(RectangleCommand),
    Line(LineCommand),
    Fill(FillCommand),
    Copy(CopyCommand),
}

#[derive(Debug, Clone)]
pub struct TriangleCommand {
    pub vertices: [PSXVertex; 3],
    pub textured: bool,
    pub gouraud: bool,
    pub blend_mode: BlendMode,
    pub texture_page: u16,
    pub clut: u16,
}

#[derive(Debug, Clone)]
pub struct PSXVertex {
    pub x: i16,
    pub y: i16,
    pub color: Color,
    pub u: u8,
    pub v: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn to_float_rgba(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendMode {
    None = 0,
    Additive = 1,
    Subtractive = 2,
    Quarter = 3,
}

#[derive(Debug, Clone)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Resolution {
            width: 320,
            height: 240,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

// Placeholder types that would need full implementation
type Result<T> = std::result::Result<T, PsxError>;
type GLuint = u32;
type GLContext = u32;

struct FramebufferCache;
struct TextureCache;
struct VertexBufferPool;
struct Shader;
struct FrameTimer;
struct RectangleCommand;
struct LineCommand;
struct FillCommand;
struct CopyCommand;

#[derive(Debug)]
enum PsxError {
    OpenGLError(String),
}

// Implementation stubs for the placeholder types
impl FramebufferCache {
    fn new() -> Self { FramebufferCache }
    fn create_framebuffer(&mut self, width: u32, height: u32) -> Result<GLuint> { Ok(0) }
    fn cleanup(&mut self) {}
}

impl TextureCache {
    fn new() -> Self { TextureCache }
    fn cleanup(&mut self) {}
}

impl VertexBufferPool {
    fn new() -> Self { VertexBufferPool }
    fn get_buffer(&mut self, size: usize) -> Result<VertexBuffer> { Ok(VertexBuffer) }
    fn cleanup(&mut self) {}
}

struct VertexBuffer;
impl VertexBuffer {
    fn upload(&self, vertices: &[Vertex]) {}
    fn draw(&self, mode: u32, count: usize) {}
}

impl Shader {
    fn compile(vertex: &str, fragment: &str) -> Result<Self> { Ok(Shader) }
    fn bind(&self) {}
    fn set_uniform_matrix4(&self, name: &str, matrix: &[[f32; 4]; 4]) {}
    fn set_uniform_int(&self, name: &str, value: i32) {}
    fn set_uniform_bool(&self, name: &str, value: bool) {}
    fn delete(&mut self) {}
}

impl FrameTimer {
    fn new() -> Self { FrameTimer }
    fn start_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn average_frame_time(&self) -> f32 { 16.67 }
}

impl GLContext {
    fn create() -> Result<Self> { Ok(0) }
}

impl OpenGLRenderer {
    fn get_projection_matrix(&self) -> [[f32; 4]; 4] {
        // Orthographic projection for 2D PSX rendering
        [[1.0, 0.0, 0.0, 0.0],
         [0.0, 1.0, 0.0, 0.0],
         [0.0, 0.0, 1.0, 0.0],
         [0.0, 0.0, 0.0, 1.0]]
    }
    
    fn draw_rectangle(&mut self, rect: &RectangleCommand) -> Result<()> { Ok(()) }
    fn draw_line(&mut self, line: &LineCommand) -> Result<()> { Ok(()) }
    fn fill_rect(&mut self, fill: &FillCommand) -> Result<()> { Ok(()) }
    fn copy_rect(&mut self, copy: &CopyCommand) -> Result<()> { Ok(()) }
    fn resolve_framebuffer(&mut self) -> Result<()> { Ok(()) }
    fn update_texture_filtering(&mut self, filter: TextureFilter) -> Result<()> { Ok(()) }
}

impl DrawCommand {
    fn can_merge(&self, other: &DrawCommand) -> bool { false }
    fn merge(&mut self, other: &DrawCommand) {}
}

impl TriangleCommand {
    fn get_shader_type(&self) -> ShaderType {
        match (self.textured, self.gouraud) {
            (false, false) => ShaderType::Flat,
            (false, true) => ShaderType::Gouraud,
            (true, false) => ShaderType::Textured,
            (true, true) => ShaderType::TexturedGouraud,
        }
    }
}