use std::{collections::HashMap, marker::PhantomData};

use js_sys::{Promise, Function};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader, WebGlVertexArrayObject, WebGlTexture, Window, Document, WebGlUniformLocation,
};

#[derive(Clone, PartialEq, Eq)]
pub struct ShaderFormat {
    pub attributes: Vec<VertexAttribute>,
    pub uniforms: Vec<Uniform>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct VertexAttribute {
    pub name: String,
    pub type_: ShaderType,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Uniform {
    pub name: String,
    pub type_: ShaderType,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ShaderType {
    Float,
    Vec2,
    Vec3,
    Mat3x3,
    Int,
    Sampler2D,
}

impl ShaderFormat {
    pub fn make_attribute_map(&self) -> HashMap<&str, &VertexAttribute> {
        let mut map = HashMap::new();
        for attr in &self.attributes {
            map.insert(attr.name.as_str(), attr);
        }
        map
    }

    pub fn make_uniform_map(&self) -> HashMap<&str, &Uniform> {
        let mut map = HashMap::new();
        for uniform in &self.uniforms {
            map.insert(uniform.name.as_str(), uniform);
        }
        map 
    }

    pub fn vertex_bytes(&self) -> usize {
        self.attributes
            .iter()
            .map(|attr| attr.type_.num_bytes())
            .sum()
    }
}

impl ShaderType {
    pub fn num_components(self) -> u32 {
        match self {
            ShaderType::Float => 1,
            ShaderType::Vec2 => 2,
            ShaderType::Vec3 => 3,
            ShaderType::Mat3x3 => 9,
            ShaderType::Int => 1,
            ShaderType::Sampler2D => todo!(),
        }
    }

    pub fn num_bytes(self) -> usize {
        match self {
            ShaderType::Float => 4,
            ShaderType::Vec2 => 8,
            ShaderType::Vec3 => 12,
            ShaderType::Mat3x3 => 36,
            ShaderType::Int => 4,
            ShaderType::Sampler2D => todo!(),
        }
    }

    pub fn webgl_scalar_type(self) -> u32 {
        match self {
            ShaderType::Float |
            ShaderType::Vec2 |
            ShaderType::Vec3 |
            ShaderType::Mat3x3 => WebGl2RenderingContext::FLOAT,
            ShaderType::Int => WebGl2RenderingContext::INT,
            ShaderType::Sampler2D => todo!(),
        }
    }

    pub fn webgl_type(self) -> u32 {
        match self {
            ShaderType::Float => WebGl2RenderingContext::FLOAT,
            ShaderType::Vec2 => WebGl2RenderingContext::FLOAT_VEC2,
            ShaderType::Vec3 => WebGl2RenderingContext::FLOAT_VEC3,
            ShaderType::Mat3x3 => WebGl2RenderingContext::FLOAT_MAT3,
            ShaderType::Int => WebGl2RenderingContext::INT,
            ShaderType::Sampler2D => WebGl2RenderingContext::SAMPLER_2D,
        }
    }
}

pub struct Shader {
    format: ShaderFormat,
    program: WebGlProgram,
}

impl Shader {
    pub fn compile(
        context: &WebGl2RenderingContext,
        format: ShaderFormat,
        vert_source: &str,
        frag_source: &str,
    ) -> Result<Shader, String> {
        let vert_shader = compile_shader(
            context, 
            WebGl2RenderingContext::VERTEX_SHADER, 
            vert_source)?;
        let frag_shader = compile_shader(
            context, 
            WebGl2RenderingContext::FRAGMENT_SHADER, 
            frag_source)?;
    
        let program = context
            .create_program()
            .ok_or_else(|| "Failed to create_program".to_string())?;
        context.attach_shader(&program, &vert_shader);
        context.attach_shader(&program, &frag_shader);
        for (i, attr) in format.attributes.iter().enumerate() {
            context.bind_attrib_location(&program, i as u32, &attr.name);
        }
        context.link_program(&program);
    
        if context
            .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
            .as_bool()
            != Some(true)
        {
            return Err(context
                .get_program_info_log(&program)
                .unwrap_or_else(|| "Failed to get_program_info_log".to_string()));
        }
    
        let mut attribute_map = format.make_attribute_map();
        let num_active_attributes = context
            .get_program_parameter(&program, WebGl2RenderingContext::ACTIVE_ATTRIBUTES)
            .as_f64()
            .ok_or_else(|| "Failed to retrieve active attributes".to_string())?
            as usize;
        for i in 0..num_active_attributes {
            let info = context
                .get_active_attrib(&program, i as u32)
                .ok_or_else(|| format!("Failed to retrieve active attribute {}", i))?;
    
            let attribute = attribute_map
                .remove(info.name().as_str())
                .ok_or_else(|| format!("Shader requires unknown vertex attribute {}", info.name()))?;
            
            if info.type_() != attribute.type_.webgl_type() {
                return Err(format!(
                    "Data type mismatch on attribute {} (Found {:#04X} expected {:#04X})",
                    info.name(),
                    info.type_(),
                    attribute.type_.webgl_type(),
                ))
            }
        }
    
        if !attribute_map.is_empty() {
            let mut names = attribute_map.keys().cloned().collect::<Vec<_>>();
            names.sort();
            return Err(format!(
                "Shader is missing these attributes: {}", names.join(", ")
            ))
        }
    
        let mut uniform_map = format.make_uniform_map();
        let num_active_uniforms = context
            .get_program_parameter(&program, WebGl2RenderingContext::ACTIVE_UNIFORMS)
            .as_f64()
            .ok_or_else(|| "Failed to retrieve active uniforms".to_string())?
            as usize;
        for i in 0..num_active_uniforms {
            let info = context
                .get_active_uniform(&program, i as u32)
                .ok_or_else(|| format!("Failed to retrieve active uniform {}", i))?;
    
            let uniform = uniform_map
                .remove(info.name().as_str())
                .ok_or_else(|| format!("Sahder requires unknown uniform {}", info.name()))?;
    
            if info.type_() != uniform.type_.webgl_type() {
                return Err(format!(
                    "Data type mismatch on uniform {} (Found {:#04X} expected {:#04X})",
                    info.name(),
                    info.type_(),
                    uniform.type_.webgl_type(),
                ))            
            }
        }    
    
        Ok(Shader { format, program })
    }

    pub fn format(&self) -> &ShaderFormat {
        &self.format
    }

    pub fn uniform_location<T: UniformValue>(&self, context: &WebGl2RenderingContext, name: &str) -> Result<ShaderUniform<T>, String> {
        // TODO check against our format
        let location = context.get_uniform_location(&self.program, name).expect("Failed to `get_uniform_location`");
        Ok(ShaderUniform { location, phantom: PhantomData })
    }

    pub fn set_uniform<T: UniformValue>(&self, context: &WebGl2RenderingContext, uniform: &ShaderUniform<T>, value: T) {
        context.use_program(Some(&self.program));
        value.set_uniform(context, &uniform.location);
    }
}

pub struct ShaderUniform<T: UniformValue> {
    location: WebGlUniformLocation,
    phantom: PhantomData<T>,
}

pub trait UniformValue {
    const SHADER_TYPE: ShaderType;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation);
}

impl UniformValue for f32 {
    const SHADER_TYPE: ShaderType = ShaderType::Float;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1f(Some(&loc), *self);
    }
}

impl UniformValue for glam::Vec2 {
    const SHADER_TYPE: ShaderType = ShaderType::Vec2;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform2f(Some(&loc), self.x, self.y);
    }
}

impl UniformValue for glam::Vec3 {
    const SHADER_TYPE: ShaderType = ShaderType::Vec3;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform3f(Some(&loc), self.x, self.y, self.z);
    }
}

impl UniformValue for glam::Mat3 {
    const SHADER_TYPE: ShaderType = ShaderType::Mat3x3;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform_matrix3fv_with_f32_array(Some(&loc), false, self.as_ref());
    }
}

impl UniformValue for i32 {
    const SHADER_TYPE: ShaderType = ShaderType::Int;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1i(Some(&loc), *self);
    }
}

pub fn make_vao(
    context: &WebGl2RenderingContext,
    format: &ShaderFormat,
    buffer: &WebGlBuffer,
) -> Result<WebGlVertexArrayObject, String> {
    context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(buffer));

    let vao = context
        .create_vertex_array()
        .ok_or_else(|| "Failed to create_vertex_array".to_string())?;
    context.bind_vertex_array(Some(&vao));

    let stride: usize = format.vertex_bytes();
    let mut offset = 0;
    for (i, attr) in format.attributes.iter().enumerate() {
        context.enable_vertex_attrib_array(i as u32);
        context.vertex_attrib_pointer_with_i32(
            i as u32,
            attr.type_.num_components() as i32,
            attr.type_.webgl_scalar_type(),
            false,
            stride as i32,
            offset as i32,
        );
        offset += attr.type_.num_bytes();
    }

    Ok(vao)
}

fn compile_shader(
    context: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = context
        .create_shader(shader_type)
        .ok_or_else(|| "Failed to create_shader".to_string())?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        != Some(true)
    {
        return Err(context
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| "Failed to `get_shader_info_log`".to_string()));
    }

    Ok(shader)
}

pub async fn load_texture(context: &WebGl2RenderingContext, src: &str) -> Result<WebGlTexture, String> {
    let image = web_sys::HtmlImageElement::new()
        .expect("Failed to create HtmlImageElement");
    image.set_src(src);
    future_from_callback(|cb| {
        image.add_event_listener_with_callback("load", &cb)
            .expect("Failed to register for image load event");
        image.add_event_listener_with_callback("error", &cb)
            .expect("Failed to register for image error event"); 
    }).await;
    
    if !image.complete() || image.natural_height() == 0 {
        return Err("Failed to load image".to_string());
    }

    let texture = context.create_texture()
        .ok_or_else(|| "Failed to `create_texture`".to_string())?;
    context.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
    context.tex_image_2d_with_u32_and_u32_and_html_image_element(
        WebGl2RenderingContext::TEXTURE_2D,
        0,
        WebGl2RenderingContext::RGBA as i32,
        WebGl2RenderingContext::RGBA,
        WebGl2RenderingContext::UNSIGNED_BYTE,
        &image,
    ).map_err(|_| "Failed to `tex_image_2d`".to_string())?;
    context.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D, 
        WebGl2RenderingContext::TEXTURE_MIN_FILTER, 
        WebGl2RenderingContext::NEAREST as i32);
    context.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D, 
        WebGl2RenderingContext::TEXTURE_MAG_FILTER, 
        WebGl2RenderingContext::NEAREST as i32);
        
    Ok(texture)
}

pub async fn dom_content_loaded() {
    if expect_document().ready_state() != "loading" {
        return;
    }

    future_from_callback(|resolve| {
        expect_window()
            .add_event_listener_with_callback("DOMContentLoaded", &resolve)
            .expect("Failed to add DOMContentLoaded event handler");
    }).await;
}

pub async fn animation_frame() -> f64 {
    future_from_callback(|cb| {
        expect_window()
            .request_animation_frame(&cb)
            .expect("Failed to `request_animation_frame`");
    }).await
        .as_f64()
        .expect("request_animation_frame did not provide a float")
        / 1e3
}

async fn future_from_callback(mut setup: impl FnMut(Function)) -> JsValue {
    JsFuture::from(Promise::new(&mut |resolve, _reject| setup(resolve)))
        .await
        .expect("Promise did not resolve")
}

fn expect_window() -> Window {
    web_sys::window().expect("Global `window` does not exist")
}

fn expect_document() -> Document {
    expect_window().document().expect("Global `document` does not exist")
}