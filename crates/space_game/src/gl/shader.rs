use std::marker::PhantomData;

use nalgebra::{Matrix3, Matrix4, Vector2, Vector3, Vector4};
use thiserror::Error;
use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlShader, WebGlUniformLocation};

use super::Context;

pub struct Shader {
    pub(super) gl: WebGl2RenderingContext,
    pub(super) program: WebGlProgram,
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
    #[error("Shader does not have uniform `{0}`")]
    MissingUniform(String),
}

impl Shader {
    pub fn compile(
        context: &Context,
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
        gl.link_program(&program);
        gl.delete_shader(Some(&vert_shader));
        gl.delete_shader(Some(&frag_shader));

        if gl
            .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
            .as_bool()
            != Some(true)
        {
            return Err(ShaderError::LinkError(
                gl.get_program_info_log(&program)
                    .unwrap_or_else(|| "get_program_info_log failed".into()),
            ));
        }

        Ok(Shader { gl, program })
    }

    pub fn uniform<T: UniformValue>(&self, name: &str) -> Result<Uniform<T>, ShaderError> {
        let location = self.gl.get_uniform_location(&self.program, name)
            .ok_or_else(|| ShaderError::MissingUniform(name.into()))?;

        Ok(Uniform {
            gl: self.gl.clone(),
            program: self.program.clone(),
            location,
            phantom: PhantomData,
        })  
    }
}

fn compile_shader(
    gl: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, ShaderError> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or(ShaderError::CreateShaderFailed)?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        != Some(true)
    {
        return Err(ShaderError::CompileError(
            gl
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

pub struct Uniform<T> {
    gl: WebGl2RenderingContext,
    program: WebGlProgram,
    location: WebGlUniformLocation,
    phantom: PhantomData<T>,
}

impl<T: UniformValue> Uniform<T> {
    pub fn set(&self, value: &T) {
        self.gl.use_program(Some(&self.program));
        value.set_uniform(&self.gl, &self.location);
    }
}

pub trait UniformValue {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation);
}

impl UniformValue for f32 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform1f(Some(loc), *self);
    }
}

impl UniformValue for Vector2<f32> {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform2f(Some(loc), self.x, self.y);
    }
}

impl UniformValue for Vector3<f32> {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform3f(Some(loc), self.x, self.y, self.z);
    }
}

impl UniformValue for Vector4<f32> {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform4f(Some(loc), self.x, self.y, self.z, self.w);
    }
}

impl UniformValue for Matrix3<f32> {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform_matrix3fv_with_f32_array(Some(loc), false, self.as_slice());
    }
}

impl UniformValue for Matrix4<f32> {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform_matrix4fv_with_f32_array(Some(loc), false, self.as_slice());
    }
}

impl UniformValue for i32 {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform1i(Some(loc), *self);
    }
}

pub struct Sampler2D(pub u32);

impl UniformValue for Sampler2D {
    fn set_uniform(&self, gl: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        gl.uniform1i(Some(loc), self.0 as i32);
    }
}
