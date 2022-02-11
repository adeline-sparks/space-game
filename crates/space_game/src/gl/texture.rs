use super::Context;
use crate::dom::{self, DomError};
use thiserror::Error;
use web_sys::{WebGl2RenderingContext, WebGlTexture};

#[derive(Clone)]
pub struct Texture {
    gl: WebGl2RenderingContext,
    texture: WebGlTexture,
}

#[derive(Error, Debug)]
#[error("Failed to create texture from \"{0}\"")]
pub struct TextureError(String);

impl Texture {
    pub async fn load(context: &Context, src: &str) -> anyhow::Result<Texture> {
        let image = dom::load_image(src).await?;
        let gl = &context.gl;
        let texture = gl
            .create_texture()
            .ok_or_else(|| TextureError(src.to_string()))?;
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        gl.tex_image_2d_with_u32_and_u32_and_html_image_element(
            WebGl2RenderingContext::TEXTURE_2D,
            0,
            WebGl2RenderingContext::RGBA as i32,
            WebGl2RenderingContext::RGBA,
            WebGl2RenderingContext::UNSIGNED_BYTE,
            &image,
        )
        .map_err(DomError::from)?;
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
        Ok(Texture {
            gl: gl.clone(),
            texture,
        })
    }

    pub fn bind(textures: &[Option<&Self>], gl: &WebGl2RenderingContext) {
        for (i, texture) in textures.iter().enumerate() {
            if let Some(texture) = texture {
                gl.active_texture(WebGl2RenderingContext::TEXTURE0 + (i as u32));
                gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture.texture));
            }
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        self.gl.delete_texture(Some(&self.texture));
    }
}
