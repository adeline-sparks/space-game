use std::collections::HashMap;
use std::marker::PhantomData;

use thiserror::Error;
use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlShader, WebGlUniformLocation};

use crate::dom::DomError;
use crate::gl::{webgl_type, Context};
use crate::mesh::{Attribute, AttributeName};

pub struct Shader {
    gl: WebGl2RenderingContext,
    program: WebGlProgram,
}

#[derive(Error, Debug)]
pub enum ShaderError {
    #[error("Failed to create_program")]
    CreateProgramFailed,
    #[error("Failed to create_shader")]
    CreateShaderFailed,
    #[error("Error while compiling shader: {0}")]
    CompileError(String),
    #[error("Error while linking shader: {0}")]
    LinkError(String),
    #[error("Type error for attribute `{0}` (Found {1:#04X} expected {2:#04X})")]
    AttributeTypeError(AttributeName, u32, u32),
    #[error("Shader expects unknown attribute `{0}`")]
    UnknownAttribute(AttributeName),
    #[error(transparent)]
    DomError(#[from] DomError),
}

impl Shader {
    pub fn compile(
        context: &Context,
        attributes: &[Attribute],
        vert_source: &str,
        frag_source: &str,
    ) -> Result<Shader, ShaderError> {
        let gl = context.gl.clone();
        let vert_shader = compile_shader(&gl, WebGl2RenderingContext::VERTEX_SHADER, vert_source)?;
        let frag_shader =
            compile_shader(&gl, WebGl2RenderingContext::FRAGMENT_SHADER, frag_source)?;

        let program = gl
            .create_program()
            .ok_or(ShaderError::CreateProgramFailed)?;
        gl.attach_shader(&program, &vert_shader);
        gl.attach_shader(&program, &frag_shader);
        for (i, attr) in attributes.iter().enumerate() {
            gl.bind_attrib_location(&program, i as u32, &attr.name);
        }
        gl.link_program(&program);

        if gl
            .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
            .as_bool()
            != Some(true)
        {
            return Err(ShaderError::LinkError(
                gl.get_program_info_log(&program)
                    .unwrap_or("get_program_info_log failed".into()),
            ));
        }

        let num_active_attributes = gl
            .get_program_parameter(&program, WebGl2RenderingContext::ACTIVE_ATTRIBUTES)
            .unchecked_into_f64() as usize;

        let attribute_map = attributes
            .iter()
            .map(|attr| (attr.name.as_ref(), attr))
            .collect::<HashMap<_, _>>();
        for i in 0..num_active_attributes {
            let info = gl
                .get_active_attrib(&program, i as u32)
                .expect("get_active_attrib failed");

            let attribute = *attribute_map
                .get(info.name().as_str())
                .ok_or_else(|| ShaderError::UnknownAttribute(info.name().into()))?;

            let type_ = webgl_type(attribute.type_);
            if info.type_() != type_ {
                return Err(ShaderError::AttributeTypeError(
                    attribute.name.clone(),
                    info.type_(),
                    type_,
                ));
            }
        }

        Ok(Shader { gl, program })
    }

    pub fn uniform_location<T: UniformValue>(&self, name: &str) -> Result<Uniform<T>, DomError> {
        let location = self
            .gl
            .get_uniform_location(&self.program, name)
            .ok_or(JsValue::from("Failed to `get_uniform_location`"))?;
        Ok(Uniform {
            location,
            phantom: PhantomData,
        })
    }

    pub fn set_uniform<T: UniformValue>(&self, uniform: &Uniform<T>, value: T) {
        self.gl.use_program(Some(&self.program));
        value.set_uniform(&self.gl, &uniform.location);
    }

    pub fn use_(&self) {
        self.gl.use_program(Some(&self.program));
    }
}

fn compile_shader(
    context: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, ShaderError> {
    let shader = context
        .create_shader(shader_type)
        .ok_or(ShaderError::CreateShaderFailed)?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        != Some(true)
    {
        return Err(ShaderError::CompileError(
            context
                .get_shader_info_log(&shader)
                .unwrap_or_else(|| "Failed to `get_shader_info_log`".into()),
        ));
    }

    Ok(shader)
}

impl Drop for Shader {
    fn drop(&mut self) {
        self.gl.delete_program(Some(&self.program));
    }
}

pub struct Uniform<T: UniformValue> {
    location: WebGlUniformLocation,
    phantom: PhantomData<T>,
}

pub trait UniformValue {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation);
}

impl UniformValue for f32 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform1f(Some(&loc), *self);
    }
}

impl UniformValue for glam::Vec2 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform2f(Some(&loc), self.x, self.y);
    }
}

impl UniformValue for glam::Vec3 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform3f(Some(&loc), self.x, self.y, self.z);
    }
}

impl UniformValue for glam::Vec4 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform4f(Some(&loc), self.x, self.y, self.z, self.w);
    }
}

impl UniformValue for glam::Mat3 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform_matrix3fv_with_f32_array(Some(&loc), false, self.as_ref());
    }
}

impl UniformValue for glam::Mat4 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform_matrix4fv_with_f32_array(Some(&loc), false, self.as_ref());
    }
}

impl UniformValue for i32 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform1i(Some(&loc), *self);
    }
}

pub struct Sampler2D(pub u32);

impl UniformValue for Sampler2D {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform1i(Some(&loc), self.0 as i32);
    }
}
