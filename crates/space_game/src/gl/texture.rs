use super::Context;
use crate::dom::{self, DomError};
use thiserror::Error;
use web_sys::{WebGl2RenderingContext, WebGlTexture};

#[derive(Clone)]
pub struct Texture {
    pub(super) gl: WebGl2RenderingContext,
    pub(super) texture: WebGlTexture,
}

#[derive(Error, Debug)]
pub enum TextureError {
    #[error("Failed to create_texture")]
    CreateTextureFailed,
    #[error(transparent)]
    DomError(#[from] DomError),
}

impl Texture {
    pub async fn load(context: &Context, src: &str) -> Result<Texture, TextureError> {
        let image = dom::load_image(src).await?;
        let gl = &context.gl;
        let texture = gl
            .create_texture().ok_or(TextureError::CreateTextureFailed)?;
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
        gl.generate_mipmap(WebGl2RenderingContext::TEXTURE_2D);

        Ok(Texture {
            gl: gl.clone(),
            texture,
        })
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        self.gl.delete_texture(Some(&self.texture));
    }
}
