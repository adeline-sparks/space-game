mod galaxy;
use std::{mem::size_of, num::NonZeroU32};

pub use galaxy::*;

mod histogram;
pub use histogram::*;

mod tonemap;
use nalgebra::Vector2;
pub use tonemap::*;
use wgpu::{Device, Queue, TextureFormat, BufferDescriptor, BufferUsages, SurfaceConfiguration, TextureDescriptor, Extent3d, TextureUsages, TextureViewDescriptor, TextureViewDimension, TextureAspect, Buffer, Texture, TextureView, CommandEncoder};

use crate::Camera;

pub struct Renderer {
    pub camera_buffer: Buffer, // TODO
    hdr_view: TextureView,
    galaxy: GalaxyBox,
    histogram: Histogram,
    tonemap: Tonemap,
}

impl Renderer {
    pub async fn new(
        device: &Device,
        queue: &Queue,
        surface_config: &SurfaceConfiguration,
    ) -> anyhow::Result<Self> {
        let hdr_format = TextureFormat::Rgba16Float;
        let hdr_tex_size = Vector2::new(surface_config.width, surface_config.height);

        let hdr_tex = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: hdr_tex_size.x,
                height: hdr_tex_size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
        });

        let hdr_view = hdr_tex.create_view(&TextureViewDescriptor {
            label: None,
            format: Some(hdr_format),
            dimension: Some(TextureViewDimension::D2),
            aspect: TextureAspect::default(),
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(1),
        });

        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: size_of::<Camera>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let galaxy = GalaxyBox::new(
            device,
            queue,
            &camera_buffer,
            hdr_format,
        ).await?;

        let histogram = Histogram::new(
            device,
            &hdr_view,
            hdr_tex_size,
        );

        let tonemap = Tonemap::new(
            device,
            &hdr_view,
            histogram.buffer(),
            surface_config.format,
        );
        
        Ok(Renderer { camera_buffer, hdr_view, galaxy, histogram, tonemap })
    }

    pub fn draw(&self, encoder: &mut CommandEncoder, target: &TextureView) {
        self.galaxy.draw(encoder, &self.hdr_view);
        self.histogram.dispatch(encoder);
        self.tonemap.draw(encoder, target);
    }
}