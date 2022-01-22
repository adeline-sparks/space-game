use std::{marker::PhantomData, collections::{HashMap, HashSet}};

use web_sys::{WebGl2RenderingContext, WebGlShader, WebGlUniformLocation, WebGlProgram};

use super::{Context, AttributeFormat, RenderType};


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderFormat {
    pub attributes: Vec<AttributeFormat>,
    pub attribute_map: HashMap<String, usize>,
    pub uniform_map: HashMap<String, UniformFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniformFormat {
    pub name: String,
    pub type_: RenderType,
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
                .ok_or_else(|| format!("Shader requires unknown uniform {}", info.name()))?;
    
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
        if uniform.type_ != T::RENDER_TYPE {
            return Err(format!("Type mismatch (requested {:?} actual {:?})", T::RENDER_TYPE, uniform.type_));
        }

        let location = self.context.get_uniform_location(&self.program, name).expect("Failed to `get_uniform_location`");
        Ok(Uniform { location, phantom: PhantomData })
    }

    pub fn set_uniform<T: UniformValue>(&self, uniform: &Uniform<T>, value: T) {
        self.context.use_program(Some(&self.program));
        value.set_uniform(&self.context, &uniform.location);
    }

    pub(super) fn use_(&self, context: &WebGl2RenderingContext) {
        context.use_program(Some(&self.program));
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
    const RENDER_TYPE: RenderType;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation);
}

impl UniformValue for f32 {
    const RENDER_TYPE: RenderType = RenderType::Float;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1f(Some(&loc), *self);
    }
}

impl UniformValue for glam::Vec2 {
    const RENDER_TYPE: RenderType = RenderType::Vec2;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform2f(Some(&loc), self.x, self.y);
    }
}

impl UniformValue for glam::Vec3 {
    const RENDER_TYPE: RenderType = RenderType::Vec3;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform3f(Some(&loc), self.x, self.y, self.z);
    }
}

impl UniformValue for glam::Mat3 {
    const RENDER_TYPE: RenderType = RenderType::Mat3x3;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform_matrix3fv_with_f32_array(Some(&loc), false, self.as_ref());
    }
}

impl UniformValue for i32 {
    const RENDER_TYPE: RenderType = RenderType::Int;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1i(Some(&loc), *self);
    }
}

pub struct Sampler2D(pub u32);

impl UniformValue for Sampler2D {
    const RENDER_TYPE: RenderType = RenderType::Sampler2D;

    fn set_uniform(&self, context: &WebGl2RenderingContext, loc: &WebGlUniformLocation) {
        context.uniform1i(Some(&loc), self.0 as i32);
    }   
}
