#![allow(dead_code)]
use std::f64::consts::PI;

use dom::{open_websocket, spawn, InputEventListener, Key};
use futures::FutureExt;
use gl::{Context, Sampler2D, Shader, Texture, DrawPrimitives, PrimitiveBuffer, ShaderLoader};
use log::info;
use nalgebra::{Isometry3, Matrix4, Point3, Translation3, UnitQuaternion, Vector3};

pub mod dom;
pub mod gl;
pub mod mesh;
pub mod voxel;
use voxel::{marching_cubes, SignedDistanceFunction};

use crate::dom::DomError;

pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();

    use anyhow::Context;
    spawn(main_render().map(|r| r.context("main_render failed")));
    spawn(main_net().map(|r| r.context("main_net failed")));
}

struct Sphere(Vector3<f64>, f64);

impl SignedDistanceFunction for Sphere {
    fn value(&self, pos: Vector3<f64>) -> f64 {
        (pos - self.0).norm() - self.1
    }

    fn grad(&self, pos: Vector3<f64>) -> Vector3<f64> {
        2.0 * (pos - self.0)
    }
}

impl<A: SignedDistanceFunction, B: SignedDistanceFunction> SignedDistanceFunction for (A, B) {
    fn value(&self, pos: Vector3<f64>) -> f64 {
        let a = self.0.value(pos);
        let b = self.1.value(pos);
        if a < b {
            a
        } else {
            b
        }
    }

    fn grad(&self, pos: Vector3<f64>) -> Vector3<f64> {
        let a = self.0.value(pos);
        let b = self.1.value(pos);
        if a < b {
            self.0.grad(pos)
        } else {
            self.1.grad(pos)
        }
    }
}

async fn main_render() -> anyhow::Result<()> {
    dom::content_loaded().await?;
    let input = InputEventListener::from_canvas("space_game")?;
    let context = Context::from_canvas("space_game")?;

    let color_texture = Texture::load(&context, "res/ground_0010_base_color_2k.jpg").await?;
    let normal_texture = Texture::load(&context, "res/ground_0010_normal_2k.jpg").await?;

    let mesh = marching_cubes(
        &(
            Sphere(Vector3::new(0.0, 0.0, 0.0), 50.0),
            Sphere(Vector3::new(50.0, 10.0, 0.0), 30.0),
        ),
        (
            Vector3::new(-128.0, -128.0, -128.0),
            Vector3::new(128.0, 128.0, 128.0),
        ),
        Vector3::new(32, 32, 32),
    );
    let vbo = PrimitiveBuffer::build(&context, &mesh)?;

    let mut shader_loader = ShaderLoader::new();
    let shader = Shader::load(
        &context,
        &mut shader_loader,
        "res/test.vert.glsl",
        "res/test.frag.glsl",
    ).await?;

    let vao = DrawPrimitives::build(&context, &shader, &vbo)?;

    let model_view_projection_loc =
        shader.uniform::<Matrix4<f32>>("model_view_projection")?;
    let model_matrix_loc = shader.uniform::<Matrix4<f32>>("model_matrix")?;
    let normal_matrix_loc = shader.uniform::<Matrix4<f32>>("normal_matrix")?;
    let tex_color = shader.uniform::<Sampler2D>("tex_color")?;
    let tex_normal = shader.uniform::<Sampler2D>("tex_normal")?;
    let tex_scale_loc = shader.uniform::<f32>("tex_scale")?;
    let tex_blend_sharpness_loc = shader.uniform::<f32>("tex_blend_sharpness")?;
    let light_dir_loc = shader.uniform::<Vector3<f32>>("light_dir").ok();

    tex_scale_loc.set(&0.1);
    tex_blend_sharpness_loc.set(&4.0);
    tex_color.set(&Sampler2D(0));
    tex_normal.set(&Sampler2D(1));

    let canvas = context.canvas();
    let aspect_ratio = (canvas.width() as f64) / (canvas.height() as f64);
    let projection = Matrix4::new_perspective((75.0f64).to_radians(), aspect_ratio, 1.0, 1000.0);

    let mut view = Isometry3::<f64>::look_at_rh(
        &Point3::new(0.0, 0.0, 100.0),
        &Point3::origin(),
        &Vector3::y_axis(),
    );
    let mut prev_time = animation_frame_seconds().await?;
    let mut prev_mouse_pos = input.mouse_pos();
    loop {
        let time = animation_frame_seconds().await?;
        let dt = time - prev_time;
        prev_time = time;

        let mouse_pos = input.mouse_pos();
        let mouse_delta = (mouse_pos - prev_mouse_pos).cast() * dt;
        prev_mouse_pos = mouse_pos;

        let mut rot = UnitQuaternion::from_scaled_axis(Vector3::new(
            mouse_delta.y / 20.0,
            mouse_delta.x / 20.0,
            0.0,
        ));

        let speed = PI / 4.0;
        if input.is_key_down(&Key::ch('q')) {
            rot *= UnitQuaternion::from_axis_angle(&Vector3::z_axis(), speed * dt);
        } else if input.is_key_down(&Key::ch('e')) {
            rot *= UnitQuaternion::from_axis_angle(&Vector3::z_axis(), -speed * dt);
        }
        view.append_rotation_mut(&rot);

        let mut translate = Translation3::<f64>::new(0.0, 0.0, 0.0);
        let speed = 50.0;
        if input.is_key_down(&Key::ch('w')) {
            translate.z += speed * dt;
        } else if input.is_key_down(&Key::ch('s')) {
            translate.z -= speed * dt;
        }

        if input.is_key_down(&Key::ch('a')) {
            translate.x += speed * dt;
        } else if input.is_key_down(&Key::ch('d')) {
            translate.x -= speed * dt;
        }
        view.append_translation_mut(&translate);

        let light_dir = Vector3::new((time / 2.0).cos(), 0.0, (time / 2.0).sin());

        context.clear();
        let model = Matrix4::identity();
        let model_view = view.to_matrix() * model;
        let model_view_projection = projection * model_view;
        model_view_projection_loc.set(&model_view_projection.cast());
        model_matrix_loc.set(&model.cast());
        normal_matrix_loc.set(&model.cast());
        if let Some(loc) = &light_dir_loc {
            loc.set(&light_dir.cast());
        }
        context.draw(
            &[&color_texture, &normal_texture],
            &vao,
        );
    }
}

async fn animation_frame_seconds() -> Result<f64, DomError> {
    Ok(dom::animation_frame().await? / 1e3)
}

async fn main_net() -> anyhow::Result<()> {
    info!("Creating websocket");
    let ws = open_websocket("api/v1/ws").await?;
    info!("Websocket connected");
    ws.send_with_str("Hello World").map_err(DomError::from)?;
    Ok(())
}
