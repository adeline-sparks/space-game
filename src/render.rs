use std::collections::HashMap;

use web_sys::{
    WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader, WebGlVertexArrayObject,
};

pub struct VertexFormat {
    pub attributes: Vec<VertexAttribute>,
}

impl VertexFormat {
    pub fn make_attribute_map(&self) -> HashMap<&str, &VertexAttribute> {
        let mut map = HashMap::new();
        for attr in &self.attributes {
            map.insert(attr.name.as_str(), attr);
        }
        map
    }
}

pub struct VertexAttribute {
    pub name: String,
    pub type_: VertexAttributeType,
}

#[derive(Copy, Clone)]
pub enum VertexAttributeType {
    Float,
    Vec2,
    Vec3,
}

impl VertexAttributeType {
    fn num_components(self) -> u32 {
        match self {
            VertexAttributeType::Float => 1,
            VertexAttributeType::Vec2 => 2,
            VertexAttributeType::Vec3 => 3,
        }
    }

    fn num_bytes(self) -> usize {
        match self {
            VertexAttributeType::Float => 4,
            VertexAttributeType::Vec2 => 8,
            VertexAttributeType::Vec3 => 12,
        }
    }

    fn webgl_scalar_type(self) -> u32 {
        match self {
            VertexAttributeType::Float |
            VertexAttributeType::Vec2 |
            VertexAttributeType::Vec3 => WebGl2RenderingContext::FLOAT,
        }
    }

    fn webgl_type(self) -> u32 {
        match self {
            VertexAttributeType::Float => WebGl2RenderingContext::FLOAT,
            VertexAttributeType::Vec2 => WebGl2RenderingContext::FLOAT_VEC2,
            VertexAttributeType::Vec3 => WebGl2RenderingContext::FLOAT_VEC3,
        }
    }
}

pub fn make_vao(
    context: &WebGl2RenderingContext,
    format: &VertexFormat,
    buffer: &WebGlBuffer,
) -> Result<WebGlVertexArrayObject, String> {
    context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(buffer));

    let vao = context
        .create_vertex_array()
        .ok_or_else(|| String::from("Failed to create vertex array object"))?;
    context.bind_vertex_array(Some(&vao));

    let stride: usize = format
        .attributes
        .iter()
        .map(|attr| attr.type_.num_bytes())
        .sum();

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

pub fn make_program(
    context: &WebGl2RenderingContext,
    vert_format: &VertexFormat,
    vert_source: &str,
    frag_source: &str,
) -> Result<WebGlProgram, String> {
    let vert_shader = compile_shader(context, WebGl2RenderingContext::VERTEX_SHADER, vert_source)?;
    let frag_shader = compile_shader(
        context,
        WebGl2RenderingContext::FRAGMENT_SHADER,
        frag_source,
    )?;

    let program = context
        .create_program()
        .ok_or_else(|| String::from("Unable to create program object"))?;
    context.attach_shader(&program, &vert_shader);
    context.attach_shader(&program, &frag_shader);
    for (i, attr) in vert_format.attributes.iter().enumerate() {
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
            .unwrap_or_else(|| String::from("Unknown error creating program object")));
    }

    let attribute_map = vert_format.make_attribute_map();

    let num_active_attributes = context
        .get_program_parameter(&program, WebGl2RenderingContext::ACTIVE_ATTRIBUTES)
        .as_f64()
        .map(|v| v as usize)
        .ok_or_else(|| String::from("Failed to retrieve active attributes"))?;
    for i in 0..num_active_attributes {
        let info = context
            .get_active_attrib(&program, i as u32)
            .ok_or_else(|| format!("Failed to retrieve active attribute {}", i))?;

        let attribute = attribute_map
            .get(info.name().as_str())
            .cloned()
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

    Ok(program)
}

fn compile_shader(
    context: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = context
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        != Some(true)
    {
        return Err(context
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader")));
    }

    Ok(shader)
}
