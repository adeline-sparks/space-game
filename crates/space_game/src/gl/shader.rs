use std::{marker::PhantomData};
use std::fmt::Write;

use async_recursion::async_recursion;
use indexmap::IndexMap;
use nalgebra::{Matrix3, Matrix4, Vector2, Vector3, Vector4};
use thiserror::Error;
use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlShader, WebGlUniformLocation};

use crate::dom::load_text;

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
    #[error(transparent)]
    Preprocessor(#[from] ShaderLoaderError)
}

impl Shader {
    pub async fn load(
        context: &Context,
        preprocessor: &mut ShaderLoader,
        vert_path: &str,
        frag_path: &str,
    ) -> Result<Shader, ShaderError> {
        preprocessor.load(vert_path).await?;
        preprocessor.load(frag_path).await?;
        Self::compile(context, preprocessor.get(vert_path).unwrap(), preprocessor.get(frag_path).unwrap())
    }

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

#[derive(Default)]
pub struct ShaderLoader {
    cache: IndexMap<String, Option<String>>,
}

#[derive(Error, Debug)]
pub enum ShaderLoaderError {
    #[error("TODO")]
    RequestFailed(String),
    #[error("TODO")]
    IncludeCycle(String),
    #[error("TODO")]
    IncludeSyntaxError(String),
    #[error("TODO")]
    VersionMismatch(String),
}

impl ShaderLoader {
    pub fn new() -> Self {
        ShaderLoader::default()
    }

    pub fn get<'s>(&'s self, path: &str) -> Option<&'s str> {
        self.cache.get(path).map(|e| e.as_ref().unwrap().as_str())
    }

    pub fn get_path(&self, source_index: usize) -> Option<&str> {
        self.cache.get_index(source_index).map(|(p, _)| p.as_str())
    }

    #[async_recursion(?Send)]
    pub async fn load(&mut self, path: &str) -> Result<(), ShaderLoaderError> {
        if let Some(entry) = self.cache.get(path) {
            if entry.is_some() {
                return Ok(());
            }

            return Err(ShaderLoaderError::IncludeCycle(path.to_string()));
        }

        let (source_index, _) = self.cache.insert_full(path.to_string(), None);

        let file = load_text(path)
            .await
            .map_err(|_| ShaderLoaderError::RequestFailed(path.to_string()))?;

        let mut result = String::new();
        let mut needs_line_directive = false;
        for (line_num, line) in file.lines().enumerate() {
            let line_trimmed = line.trim_start();
            if let Some(rest) = line_trimmed.strip_prefix("#include") {
                let include_literal = rest.trim();
                let include = 
                    (if let Some(rest) = include_literal.strip_prefix('<') {
                        rest.strip_suffix('>')
                    } else if let Some(rest) = include_literal.strip_prefix('"') {
                        rest.strip_suffix('"')
                    } else {
                        None
                    })
                    .ok_or_else(|| ShaderLoaderError::IncludeSyntaxError(path.to_string()))?;

                self.load(include).await?;
                result.push_str(self.cache[include].as_ref().unwrap());
                needs_line_directive = true;
                continue;
            }

            if line_trimmed.starts_with("#version") {
                writeln!(&mut result, "{line}").unwrap();
                needs_line_directive = true;
                continue;
            }

            if needs_line_directive {
                needs_line_directive = false;
                writeln!(&mut result, "#line {line_num} {source_index}").unwrap();
            }
            writeln!(&mut result, "{line}").unwrap();
        }

        self.cache[source_index] = Some(result);
        Ok(())
    }
}
