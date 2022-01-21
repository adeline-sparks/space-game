use std::{collections::{HashMap, HashSet}, marker::PhantomData};

use js_sys::{Promise, Function, Uint8Array, Uint16Array};
use wasm_bindgen::{JsValue, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader, WebGlVertexArrayObject, WebGlTexture, Window, Document, WebGlUniformLocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderFormat {
    pub attributes: Vec<AttributeFormat>,
    pub attribute_map: HashMap<String, usize>,
    pub uniform_map: HashMap<String, UniformFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeFormat {
    pub name: String,
    pub type_: ShaderType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniformFormat {
    pub name: String,
    pub type_: ShaderType,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ShaderType {
    Float,
    Vec2,
    Vec3,
    Mat3x3,
    Int,
    Sampler2D,
}

impl ShaderFormat {
    pub fn new(attributes: Vec<AttributeFormat>, uniforms: Vec<UniformFormat>) -> Self {
        let attribute_map = attributes
            .iter()
            .enumerate()
            .map(|(i, attr)| (attr.name.clone(), i))
            .collect();
        let uniform_map = uniforms
            .into_iter()
            .map(|uniform| (uniform.name.clone(), uniform))
            .collect();
        ShaderFormat { attributes, attribute_map, uniform_map }
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
    context: WebGl2RenderingContext,
    format: ShaderFormat,
    program: WebGlProgram,
}

impl Shader {
    pub fn compile(
        context: &Context,
        format: ShaderFormat,
        vert_source: &str,
        frag_source: &str,
    ) -> Result<Shader, String> {
        let context = context.0.clone();
        let vert_shader = compile_shader(
            &context, 
            WebGl2RenderingContext::VERTEX_SHADER, 
            vert_source)?;
        let frag_shader = compile_shader(
            &context, 
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
    
        let num_active_attributes = context
            .get_program_parameter(&program, WebGl2RenderingContext::ACTIVE_ATTRIBUTES)
            .as_f64()
            .ok_or_else(|| "Failed to retrieve active attributes".to_string())?
            as usize;

        let mut missing_names = format.attribute_map.keys().map(|s| s.as_str()).collect::<HashSet<_>>();
        for i in 0..num_active_attributes {
            let info = context
                .get_active_attrib(&program, i as u32)
                .ok_or_else(|| format!("Failed to retrieve active attribute {}", i))?;
    
            let attribute_pos = *format.attribute_map
                .get(info.name().as_str())
                .ok_or_else(|| format!("Shader requires unknown vertex attribute {}", info.name()))?;
            let attribute = &format.attributes[attribute_pos];
            
            if info.type_() != attribute.type_.webgl_type() {
                return Err(format!(
                    "Data type mismatch on attribute {} (Found {:#04X} expected {:#04X})",
                    info.name(),
                    info.type_(),
                    attribute.type_.webgl_type(),
                ))
            }

            missing_names.remove(info.name().as_str());
        }
    
        if !missing_names.is_empty() {
            let mut missing_names = missing_names.into_iter().collect::<Vec<_>>();
            missing_names.sort();
            return Err(format!(
                "Shader is missing these attributes: {}", missing_names.join(", ")
            ))
        }
    
        let num_active_uniforms = context
            .get_program_parameter(&program, WebGl2RenderingContext::ACTIVE_UNIFORMS)
            .as_f64()
            .ok_or_else(|| "Failed to retrieve active uniforms".to_string())?
            as usize;
        let mut missing_names = format.uniform_map.keys().map(|s| s.as_str()).collect::<HashSet<_>>();
        for i in 0..num_active_uniforms {
            let info = context
                .get_active_uniform(&program, i as u32)
                .ok_or_else(|| format!("Failed to retrieve active uniform {}", i))?;
    
            let uniform = format.uniform_map
                .get(info.name().as_str())
                .ok_or_else(|| format!("Sahder requires unknown uniform {}", info.name()))?;
    
            if info.type_() != uniform.type_.webgl_type() {
                return Err(format!(
                    "Data type mismatch on uniform {} (Found {:#04X} expected {:#04X})",
                    info.name(),
                    info.type_(),
                    uniform.type_.webgl_type(),
                ))            
            }

            missing_names.remove(info.name().as_str());
        }   
        
        if !missing_names.is_empty() {
            let mut missing_names = missing_names.into_iter().collect::<Vec<_>>();
            missing_names.sort();
            return Err(format!(
                "Shader is missing these uniforms: {}", missing_names.join(", ")
            ))
        }
    
        Ok(Shader { context, format, program })
    }

    pub fn format(&self) -> &ShaderFormat {
        &self.format
    }

    pub fn uniform_location<T: UniformValue>(&self, name: &str) -> Result<Uniform<T>, String> {
        let uniform = self.format.uniform_map.get(name)
            .ok_or_else(|| format!("Unknown uniform `{}`", name))?;
        if uniform.type_ != T::SHADER_TYPE {
            return Err(format!("Type mismatch (requested {:?} actual {:?})", T::SHADER_TYPE, uniform.type_));
        }

        let location = self.context.get_uniform_location(&self.program, name).expect("Failed to `get_uniform_location`");
        Ok(Uniform { location, phantom: PhantomData })
    }

    pub fn set_uniform<T: UniformValue>(&self, uniform: &Uniform<T>, value: T) {
        self.context.use_program(Some(&self.program));
        value.set_uniform(&self.context, &uniform.location);
    }

    pub fn render(&self, mesh: &Mesh, textures: &[&Texture]) {
        self.context.use_program(Some(&self.program));
        self.context.bind_vertex_array(Some(&mesh.vao));
        for (i, texture) in textures.iter().enumerate() {
            self.context.active_texture(WebGl2RenderingContext::TEXTURE0 + (i as u32));
            self.context.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture.0));
        }

        self.context.draw_elements_with_i32(WebGl2RenderingContext::TRIANGLE_STRIP, mesh.vert_count, WebGl2RenderingContext::UNSIGNED_SHORT, 0);
    }
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

pub struct Uniform<T: UniformValue> {
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

pub struct Sampler2D(pub u32);

impl UniformValue for Sampler2D {
    const SHADER_TYPE: ShaderType = ShaderType::Sampler2D;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1i(Some(&loc), self.0 as i32);
    }   
}

pub struct MeshBuilder<'a> {
    attributes: &'a [AttributeFormat],
    bytes: Vec<u8>,
    indices: Vec<u16>,
    attribute_num: usize,
    vertex_num: usize,
}

impl<'a> MeshBuilder<'a> {
    pub fn new(attributes: &'a [AttributeFormat]) -> Self {
        MeshBuilder { 
            attributes, 
            bytes: Vec::new(), 
            indices: Vec::new(), 
            attribute_num: 0, 
            vertex_num: 0,
        }
    }

    pub fn push<V: MeshBuilderValue>(&mut self, val: V) {
        assert!(self.attributes[self.attribute_num].type_ == V::SHADER_TYPE);
        self.attribute_num += 1;
        val.push(&mut self.bytes);
    }
    
    pub fn end_vert(&mut self) -> u16 {
        assert!(self.attribute_num == self.attributes.len());
        let result: u16 = self.vertex_num.try_into().unwrap();
        self.vertex_num += 1;
        self.attribute_num = 0;
        self.indices.push(result);
        result
    }

    pub fn dup_vert(&mut self, id: u16) {
        self.indices.push(id);
    }

    pub fn build(&self, context: &Context) -> Result<Mesh, String> {
        let context = context.0.clone();
        assert!(self.attribute_num == 0);

        let vao = context
            .create_vertex_array()
            .ok_or_else(|| "Failed to create_vertex_array".to_string())?;
        context.bind_vertex_array(Some(&vao));

        let vert_buffer = context.create_buffer()
            .ok_or_else(|| "failed to create vertex buffer".to_string())?;
        context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vert_buffer));
        context.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &Uint8Array::from(self.bytes.as_slice()),
            WebGl2RenderingContext::STATIC_DRAW,
        );
    
        let stride: usize = self.attributes.iter().map(|a|a.type_.num_bytes()).sum();
        let mut offset = 0;
        for (i, attr) in self.attributes.iter().enumerate() {
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
    
        let index_buffer = context.create_buffer()
            .ok_or_else(|| "failed to create index buffer".to_string())?;
        context.bind_buffer(WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, Some(&index_buffer));
        context.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            &Uint16Array::from(self.indices.as_slice()),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        Ok(Mesh { context, vao, vert_count: self.indices.len() as i32 })
    }
}


pub trait MeshBuilderValue : UniformValue {
    fn push(&self, bytes: &mut Vec<u8>);
}

impl MeshBuilderValue for f32 {
    fn push(&self, bytes: &mut Vec<u8>) {
        bytes.extend(self.to_ne_bytes().iter());
    }
}

impl MeshBuilderValue for glam::Vec2 {
    fn push(&self, bytes: &mut Vec<u8>) {
        self.x.push(bytes);
        self.y.push(bytes);
    }
}

pub struct Mesh {
    context: WebGl2RenderingContext,
    vao: WebGlVertexArrayObject,
    vert_count: i32,
}

#[derive(Clone)]
pub struct Texture(WebGlTexture);

impl Texture {
    pub async fn load(context: &Context, src: &str) -> Result<Texture, String> {
        let context = &context.0;
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
            
        Ok(Texture(texture))
    }
}

pub struct Context(WebGl2RenderingContext);

impl Context {
    pub fn from_canvas(element_id: &str) -> Result<Self, String> {
        Ok(Context(expect_document()
            .get_element_by_id(element_id)
            .ok_or_else(|| format!("get_element_by_id failed for `{}`", element_id))?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .map_err(|_| format!("`{}` is not a canvas", element_id))?
            .get_context("webgl2")
            .ok()
            .flatten()
            .and_then(|o| o.dyn_into::<WebGl2RenderingContext>().ok())
            .ok_or_else(|| "Failed to get webgl2 context".to_string())?
        ))
    }

    pub fn begin(&self, clear_color: &glam::Vec4) {
        self.0.clear_color(0.0, 0.0, 0.0, 1.0);
        self.0.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
        let (width, height) = self.size();
        self.0.viewport(0, 0, width as i32, height as i32);
    }

    pub fn size(&self) -> (u32, u32) {
        let canvas = self.0
            .canvas()
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        (canvas.width(), canvas.height())
    }
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