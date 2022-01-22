use super::Context;
use crate::dom;
use web_sys::{WebGl2RenderingContext, WebGlTexture};

#[derive(Clone)]
pub struct Texture(WebGlTexture);

impl Texture {
    pub async fn load(context: &Context, src: &str) -> Result<Texture, String> {
        let image = dom::load_image(src).await?;
        let gl = &context.gl;
        let texture = gl
            .create_texture()
            .ok_or_else(|| "Failed to `create_texture`".to_string())?;
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        gl.tex_image_2d_with_u32_and_u32_and_html_image_element(
            WebGl2RenderingContext::TEXTURE_2D,
            0,
            WebGl2RenderingContext::RGBA as i32,
            WebGl2RenderingContext::RGBA,
            WebGl2RenderingContext::UNSIGNED_BYTE,
            &image,
        )
        .map_err(|_| "Failed to `tex_image_2d`".to_string())?;
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );

        Ok(Texture(texture))
    }

    pub fn bind(textures: &[Option<&Self>], gl: &WebGl2RenderingContext) {
        for (i, texture) in textures.iter().enumerate() {
            if let Some(texture) = texture {
                gl.active_texture(WebGl2RenderingContext::TEXTURE0 + (i as u32));
                gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture.0));
            }
        }
    }
}
