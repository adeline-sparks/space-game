use web_sys::WebGl2RenderingContext;
use super::dom;
use super::Context;
use web_sys::WebGlTexture;

#[derive(Clone)]
pub struct Texture(WebGlTexture);

impl Texture {
    pub async fn load(context: &Context, src: &str) -> Result<Texture, String> {
        let image = dom::load_image(src).await?;
        let context = &context.0;
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

    pub fn bind(textures: &[Option<&Self>], context: &WebGl2RenderingContext) {
        for (i, texture) in textures.iter().enumerate() {
            if let Some(texture) = texture {
                context.active_texture(WebGl2RenderingContext::TEXTURE0 + (i as u32));
                context.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture.0));
            }
        }
    }
}