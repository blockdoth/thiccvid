extern crate ffmpeg_next as ffmpeg;

use std::f32::consts::PI;
use std::ffi::c_void;
use std::time::{Duration, Instant};

use raylib::prelude::*;

use crate::ffmpeg::format::{Pixel, input};
use crate::ffmpeg::media::Type;
use crate::ffmpeg::software::scaling::context::Context;
use crate::ffmpeg::software::scaling::flag::Flags;
use crate::ffmpeg::util::frame::video::Video;

const FPS: u32 = 60;
const FPS_SMOOTHING: f64 = 0.8;

fn main() -> Result<(), ffmpeg::Error> {
    let video = dump_video("assets/office.mp4")?;

    println!("Frame count {}", video.frames.len());
    println!("Dims: {}x{}", video.width, video.height);

    let (mut rl, thread) = raylib::init().log_level(TraceLogLevel::LOG_ERROR).size(1920, 1080).build();
    unsafe {
        raylib::ffi::rlDisableBackfaceCulling();
        raylib::ffi::rlDisableDepthMask();
    }
    rl.set_target_fps(FPS);

    let mut state = State {
        cam_angle_h: PI / 4.0,
        cam_angle_v: PI / 4.0,
        cam_radius: 10.0,
        camera: Camera3D::perspective(
            Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            45.0,
        ),
        frame_transparency: 3L,
        frames_distance: 0.02,

        frames_visible: video.frames.len(),
        frames_opaque: 1,
        frames_total: video.frames.len(),
    };

    let scale = 0.005;

    let mesh = Mesh::gen_mesh_plane(&thread, video.width as f32 * scale, video.height as f32 * scale, 10, 10);

    let mut shader = rl.load_shader(&thread, None, Some("src/blur.fs"));

    let (textures, materials_opaque, materials_transparant) = render_texture(&state, &thread, &mut rl, &video, &mut shader);

    let direction = Matrix::rotate(Vector3 { x: 1.0, y: 0.0, z: 0.0 }, PI / 2.0);
    let time_per_frame = Duration::from_secs_f64(1.0 / FPS as f64);
    let mut last_frame = Instant::now();
    let mut fps = FPS as f64;

    while !rl.window_should_close() {
        let now = Instant::now();

        if now.duration_since(last_frame) >= time_per_frame {
            handle_input(&mut rl, &mut state);
            update_camera(&mut state);

            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::BLACK);
            {
                let mut d3 = d.begin_mode3D(state.camera);
                let mut d3 = d3.begin_blend_mode(BlendMode::BLEND_ALPHA);

                let offset = state.frames_visible as f32 / -2.0 * state.frames_distance;
                for (idx, (material_opague, material_transparant)) in materials_opaque
                    .iter()
                    .zip(&materials_transparant)
                    // .rev()
                    .take(state.frames_visible)
                    .enumerate()
                {
                    let material = if idx >= state.frames_opaque {
                        material_transparant
                    } else {
                        material_opague
                    };
                    d3.draw_mesh(
                        &mesh,
                        material.clone(),
                        Matrix::translate(0.0, offset + idx as f32 * state.frames_distance, 0.0) * direction,
                    );
                }
            }

            let delta = now.duration_since(last_frame).as_secs_f64();
            let instant_fps = 1.0 / delta;
            fps = FPS_SMOOTHING * fps + (1.0 - FPS_SMOOTHING) * instant_fps;

            d.draw_text(&format!("Frames: {}", state.frames_visible), 10, 10, 10, Color::WHITE);
            d.draw_text(&format!("Opaque frames: {}", state.frames_opaque), 10, 20, 10, Color::WHITE);
            d.draw_text(&format!("FPS: {:.1}", fps), 10, 30, 10, Color::WHITE);

            last_frame = Instant::now();
        }
    }
    Ok(())
}

fn render_texture(state: &State, thread: &RaylibThread, rl: &mut RaylibHandle,  video: &VideoContainer, shader: &mut Shader) -> (Vec<Texture2D>, Vec<WeakMaterial>, Vec<WeakMaterial>) {
    let mut textures = Vec::new();
    let mut materials_opaque = Vec::new();
    let mut materials_transparant = Vec::new();

    let blur_radius = shader.get_shader_location("blurRadius");
    let alpha = shader.get_shader_location("alpha");

    shader.set_shader_value(blur_radius, 3);
    shader.set_shader_value(alpha, state.frame_transparency as f32);

    for image_bytes in video.frames.iter() {
        let image_raw = ffi::Image {
            data: image_bytes.as_ptr() as *mut c_void,
            width: video.width,
            height: video.height,
            mipmaps: 1,
            format: ffi::PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8 as i32,
        };
        let image = unsafe { Image::from_raw(image_raw) };
        let texture = rl.load_texture_from_image(thread, &image).unwrap();
        std::mem::forget(image);

        let mut opaque = rl.load_material_default(thread);
        opaque.set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &texture);

        let mut transparent = rl.load_material_default(thread);
        transparent.set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &texture);
        transparent.set_shader(&shader);

        materials_opaque.push(opaque);
        materials_transparant.push(transparent);
        textures.push(texture);
    }
    (textures, materials_opaque, materials_transparant)
}

struct State {
    cam_radius: f32,
    cam_angle_h: f32,
    cam_angle_v: f32,
    camera: Camera3D,
    frames_opaque: usize,
    frame_transparency: usize,
    frames_visible: usize,

    frames_distance: f32,
    frames_total: usize,
}

fn update_camera(view: &mut State) {
    let horizontal = view.cam_radius * view.cam_angle_v.cos();

    view.camera.position.x = view.camera.target.x + horizontal * view.cam_angle_h.cos();
    view.camera.position.z = view.camera.target.z + horizontal * view.cam_angle_h.sin();
    view.camera.position.y = view.camera.target.y + view.cam_radius * view.cam_angle_v.sin();
}

fn handle_input(h: &mut RaylibHandle, state: &mut State) {
    if h.is_key_down(KeyboardKey::KEY_ENTER) {
        state.cam_radius += 0.5;
    }

    if h.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT) {
        state.cam_radius -= 0.5;
    }

    if h.is_key_down(KeyboardKey::KEY_LEFT) {
        state.cam_angle_h += 0.05;
    }
    if h.is_key_down(KeyboardKey::KEY_RIGHT) {
        state.cam_angle_h -= 0.05;
    }
    if h.is_key_down(KeyboardKey::KEY_UP) {
        state.cam_angle_v += 0.05;
    }
    if h.is_key_down(KeyboardKey::KEY_DOWN) {
        state.cam_angle_v -= 0.05;
    }

    if h.is_key_down(KeyboardKey::KEY_EQUAL) && state.frames_visible < state.frames_total {
        state.frames_visible += 1;
    }
    if h.is_key_down(KeyboardKey::KEY_MINUS) && state.frames_visible > 0 {
        state.frames_visible -= 1;
    }
    if h.is_key_down(KeyboardKey::KEY_RIGHT_BRACKET) && state.frames_opaque < state.frames_visible {
        state.frames_opaque += 1;
    }
    if h.is_key_down(KeyboardKey::KEY_LEFT_BRACKET) && state.frames_opaque > 0 {
        state.frames_opaque -= 1;
    }
}

struct VideoContainer {
    frames: Vec<Vec<u8>>,
    width: i32,
    height: i32,
}

fn dump_video(path: &str) -> Result<VideoContainer, ffmpeg::Error> {
    ffmpeg::init().unwrap();
    let mut frames = vec![];

    let mut ictx = input(path)?;
    let input = ictx.streams().best(Type::Video).ok_or(ffmpeg::Error::StreamNotFound)?;
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?.decoder().video()?;

    let mut scaler = Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )?;

    let mut frame_index = 0;

    let mut receive_and_process_decoded_frames = |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
        let mut decoded = Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgb_frame = Video::empty();
            scaler.run(&decoded, &mut rgb_frame)?;

            frames.push(rgb_frame.data(0).to_vec());

            frame_index += 1;
        }
        Ok(())
    };

    let video_stream_index = input.index();
    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;
            receive_and_process_decoded_frames(&mut decoder)?;
        }
    }
    decoder.send_eof()?;
    receive_and_process_decoded_frames(&mut decoder)?;

    let video = VideoContainer {
        frames,
        width: decoder.width() as i32,
        height: decoder.height() as i32,
    };

    Ok(video)
}
