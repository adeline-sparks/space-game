use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlShader, WebGlUniformLocation};

use super::{Attribute, Context, DataType};

pub struct Shader {
    gl: WebGl2RenderingContext,
    program: WebGlProgram,
}

impl Shader {
    pub fn compile(
        context: &Context,
        attributes: &[Attribute],
        vert_source: &str,
        frag_source: &str,
    ) -> Result<Shader, String> {
        let gl = context.0.clone();
        let vert_shader =
            compile_shader(&gl, WebGl2RenderingContext::VERTEX_SHADER, vert_source)?;
        let frag_shader = compile_shader(
            &gl,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            frag_source,
        )?;

        let program = gl
            .create_program()
            .ok_or_else(|| "Failed to create_program".to_string())?;
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
            return Err(gl
                .get_program_info_log(&program)
                .unwrap_or_else(|| "Failed to get_program_info_log".to_string()));
        }

        let num_active_attributes = gl
            .get_program_parameter(&program, WebGl2RenderingContext::ACTIVE_ATTRIBUTES)
            .as_f64()
            .ok_or_else(|| "Failed to retrieve active attributes".to_string())?
            as usize;

        let attribute_map = attributes
            .iter()
            .map(|attr| (attr.name.as_str(), attr))
            .collect::<HashMap<_, _>>();
        let mut missing_names = attributes
            .iter()
            .map(|attr| attr.name.as_str())
            .collect::<HashSet<_>>();
        for i in 0..num_active_attributes {
            let info = gl
                .get_active_attrib(&program, i as u32)
                .ok_or_else(|| format!("Failed to retrieve active attribute {}", i))?;

            let attribute = *attribute_map.get(info.name().as_str()).ok_or_else(|| {
                format!("Shader requires unknown vertex attribute {}", info.name())
            })?;

            if info.type_() != attribute.type_.webgl_type() {
                return Err(format!(
                    "Data type mismatch on attribute {} (Found {:#04X} expected {:#04X})",
                    info.name(),
                    info.type_(),
                    attribute.type_.webgl_type(),
                ));
            }

            missing_names.remove(info.name().as_str());
        }

        Ok(Shader { gl, program })
    }

    pub fn uniform_location<T: UniformValue>(&self, name: &str) -> Result<Uniform<T>, String> {
        let location = self
            .gl
            .get_uniform_location(&self.program, name)
            .expect("Failed to `get_uniform_location`");
        Ok(Uniform {
            location,
            phantom: PhantomData,
        })
    }

    pub fn set_uniform<T: UniformValue>(&self, uniform: &Uniform<T>, value: T) {
        self.gl.use_program(Some(&self.program));
        value.set_uniform(&self.gl, &uniform.location);
    }

    pub(super) fn use_(&self, gl: &WebGl2RenderingContext) {
        gl.use_program(Some(&self.program));
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
    const RENDER_TYPE: DataType;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation);
}

impl UniformValue for f32 {
    const RENDER_TYPE: DataType = DataType::Float;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1f(Some(&loc), *self);
    }
}

impl UniformValue for glam::Vec2 {
    const RENDER_TYPE: DataType = DataType::Vec2;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform2f(Some(&loc), self.x, self.y);
    }
}

impl UniformValue for glam::Vec3 {
    const RENDER_TYPE: DataType = DataType::Vec3;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform3f(Some(&loc), self.x, self.y, self.z);
    }
}

impl UniformValue for glam::Mat3 {
    const RENDER_TYPE: DataType = DataType::Mat3x3;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform_matrix3fv_with_f32_array(Some(&loc), false, self.as_ref());
    }
}

impl UniformValue for i32 {
    const RENDER_TYPE: DataType = DataType::Int;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1i(Some(&loc), *self);
    }
}

pub struct Sampler2D(pub u32);

impl UniformValue for Sampler2D {
    const RENDER_TYPE: DataType = DataType::Sampler2D;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1i(Some(&loc), self.0 as i32);
    }
}
