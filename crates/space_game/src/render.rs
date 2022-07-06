mod galaxy;
mod queue;
use std::mem::size_of;
use std::num::NonZeroU32;
use std::slice;

use bytemuck::cast_slice;
pub use galaxy::*;

mod histogram;
pub use histogram::*;

mod tonemap;
use nalgebra::{Isometry3, Matrix4, Perspective3, Vector2};
use once_cell::sync::Lazy;
pub use tonemap::*;
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, Device, Extent3d, Queue, TextureAspect,
    TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    TextureViewDimension,
};

use crate::Camera;

pub struct Renderer {
    camera_buffer: Buffer,
    hdr_view: TextureView,
    target_size: Vector2<u32>,
    galaxy: GalaxyBox,
    histogram: Histogram,
    tonemap: Tonemap,
}

impl Renderer {
    pub async fn new(
        device: &Device,
        queue: &Queue,
        target_size: Vector2<u32>,
        target_format: TextureFormat,
    ) -> anyhow::Result<Self> {
        let hdr_format = TextureFormat::Rgba16Float;

        let hdr_tex = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: target_size.x,
                height: target_size.y,
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

        let galaxy = GalaxyBox::new(device, queue, &camera_buffer, hdr_format).await?;

        let histogram = Histogram::new(device, &hdr_view, target_size, 256, 0.0001, 1.0);

        let tonemap = Tonemap::new(device, &hdr_view, histogram.buckets_buffer(), target_format);

        Ok(Renderer {
            camera_buffer,
            hdr_view,
            target_size,
            galaxy,
            histogram,
            tonemap,
        })
    }

    pub fn draw(
        &mut self,
        device: &Device,
        queue: &Queue,
        target: &TextureView,
        view: &Isometry3<f64>,
    ) {
        self.histogram.with_buckets(|_| {
            // TODO
        });

        let projection = Perspective3::new(
            self.target_size.x as f64 / self.target_size.y as f64,
            (60.0f64).to_radians(),
            1.0,
            10.0,
        );
        let camera = Camera {
            viewport: Vector2::new(self.target_size.x as f32, self.target_size.y as f32),
            near: projection.znear() as f32,
            far: projection.zfar() as f32,
            inv_view_projection: {
                (view.inverse().to_matrix() * projection.inverse() * *WGPU_TO_OPENGL_MATRIX).cast()
            },
        };
        queue.write_buffer(&self.camera_buffer, 0, cast_slice(slice::from_ref(&camera)));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.galaxy.draw(&mut encoder, &self.hdr_view);
        self.histogram.encode(&mut encoder);
        self.tonemap.draw(&mut encoder, target);

        queue.submit([encoder.finish()]);
        self.histogram.map_buffers();
    }
}

#[rustfmt::skip]
static OPENGL_TO_WGPU_MATRIX: Matrix4<f64> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0, 
    0.0, 1.0, 0.0, 0.0, 
    0.0, 0.0, 0.5, 0.0, 
    0.0, 0.0, 0.5, 1.0,
);

static WGPU_TO_OPENGL_MATRIX: Lazy<Matrix4<f64>> =
    Lazy::new(|| OPENGL_TO_WGPU_MATRIX.try_inverse().unwrap());
