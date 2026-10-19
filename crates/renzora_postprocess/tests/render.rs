//! The effect pass draws: a camera clearing to a dark colour, with an invert
//! effect file on it, reads back bright.
//!
//! Needs a GPU, so it only runs with `RENZORA_GPU_TESTS=1` (the CI gpu lane
//! sets it); otherwise it returns at once. Everything a parse test cannot
//! reach is in this path: the shader loading through the asset server, the
//! settings buffer matching the layout the pipeline was built with, and the
//! pass actually running in the frame.

use std::sync::{Arc, Mutex};

use bevy::camera::RenderTarget;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use renzora::{EffectRouting, PostEffect, PostEffects};

const SIZE: u32 = 16;
const CLEAR: Color = Color::srgb(0.2, 0.2, 0.2);

fn app() -> App {
    use bevy::window::{ExitCondition, WindowPlugin};
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::winit::WinitPlugin>()
            // Pipelined rendering hands frames to a render thread the app's
            // runner sets up; a test pumping `update()` by hand has no runner.
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin {
                file_path: "tests/fixtures".into(),
                ..default()
            }),
    );
    app.add_plugins(renzora_postprocess::PostProcessPlugin);
    app.init_resource::<EffectRouting>();
    app
}

/// Run the camera with `effects` until its centre pixel satisfies `done`, and
/// return that pixel, or the last one read if it never does. `None` when no
/// GPU run was asked for.
///
/// Waits on the picture rather than counting frames: loading the shader and
/// compiling its pipeline take a variable number of frames, and more of them
/// when other tests share the GPU, so any fixed count is a race that reads a
/// texture nothing has drawn to yet.
fn centre_pixel(effects: Vec<PostEffect>, done: impl Fn([u8; 4]) -> bool) -> Option<[u8; 4]> {
    if !renzora_test_harness::gpu_tests_enabled() {
        return None;
    }
    let mut app = app();
    app.finish();

    let mut image = Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(CLEAR),
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            // So the cleared colour reaches the effect unchanged.
            Tonemapping::None,
            Msaa::Off,
        ))
        .id();
    let source = app.world_mut().spawn(PostEffects { effects }).id();
    app.world_mut().resource_mut::<EffectRouting>().routes = vec![(camera, vec![source])];

    let latest: Arc<Mutex<Option<Vec<u8>>>> = Arc::default();
    let sink = latest.clone();
    app.world_mut()
        .spawn(Readback::texture(target))
        .observe(move |event: On<ReadbackComplete>| {
            *sink.lock().unwrap() = Some(event.data.clone());
        });

    let pixel = |data: &[u8]| {
        let i = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
        [data[i], data[i + 1], data[i + 2], data[i + 3]]
    };
    let mut last = None;
    for _ in 0..2000 {
        app.update();
        if let Some(data) = latest.lock().unwrap().as_deref() {
            let px = pixel(data);
            last = Some(px);
            if done(px) {
                break;
            }
        }
    }
    assert!(
        app.world()
            .get::<renzora_postprocess::PostEffectChain>(camera)
            .is_some(),
        "the chain never reached the camera"
    );
    Some(last.expect("no readback arrived"))
}

/// An opaque pixel: the camera has drawn at least once.
fn drawn(px: [u8; 4]) -> bool {
    px[3] == 255
}

/// The clear colour is about 51 in sRGB, fully inverted about 251, and half
/// way (a linear 0.5 grey) about 188.
fn inverted(px: [u8; 4]) -> bool {
    drawn(px) && px[0] > 230
}
fn half_inverted(px: [u8; 4]) -> bool {
    drawn(px) && (160..220).contains(&px[0])
}

#[test]
fn an_invert_effect_turns_a_dark_clear_bright() {
    let Some(px) = centre_pixel(vec![PostEffect::new("invert.wesl")], inverted) else {
        return;
    };
    assert!(inverted(px), "expected an inverted, bright pixel, got {px:?}");
}

#[test]
fn a_slider_value_reaches_the_shader() {
    // Half way between the clear colour and its inverse can only come from
    // the pass running with the packed 0.5, not the shader default of 1 and
    // not from the pass being skipped.
    let mut effect = PostEffect::new("invert.wesl");
    effect.set_value("amount", 0.5);
    let Some(px) = centre_pixel(vec![effect], half_inverted) else {
        return;
    };
    assert!(half_inverted(px), "expected a half-inverted grey, got {px:?}");
}

#[test]
fn every_effect_in_the_list_runs_on_the_one_before() {
    // A full invert then a quarter one lands near 223. Either alone gives
    // about 251 or about 140, so only both, the second reading what the first
    // wrote, comes out in between.
    let mut quarter = PostEffect::new("invert.wesl");
    quarter.set_value("amount", 0.25);
    let effects = vec![PostEffect::new("invert.wesl"), quarter];
    let both = |px: [u8; 4]| drawn(px) && (205..240).contains(&px[0]);
    let Some(px) = centre_pixel(effects, both) else {
        return;
    };
    assert!(both(px), "expected both effects applied, got {px:?}");
}

#[test]
fn a_snapshot_effect_reads_the_frozen_copy_of_the_frame() {
    // The fixture draws only the inverted snapshot, so a bright pixel means
    // the copy was taken from the live frame and bound at 3 and 4.
    let Some(px) = centre_pixel(vec![PostEffect::new("frozen_invert.wesl")], inverted) else {
        return;
    };
    assert!(inverted(px), "expected the inverted snapshot, got {px:?}");
}

#[test]
fn a_switched_off_effect_is_not_drawn() {
    let mut effect = PostEffect::new("invert.wesl");
    effect.enabled = false;
    if !renzora_test_harness::gpu_tests_enabled() {
        return;
    }
    // Off means no chain at all, so build the app by hand and check that.
    let mut app = app();
    app.finish();
    let camera = app.world_mut().spawn(Camera3d::default()).id();
    let source = app
        .world_mut()
        .spawn(PostEffects {
            effects: vec![effect],
        })
        .id();
    app.world_mut().resource_mut::<EffectRouting>().routes = vec![(camera, vec![source])];
    for _ in 0..60 {
        app.update();
    }
    assert!(app
        .world()
        .get::<renzora_postprocess::PostEffectChain>(camera)
        .is_none());
}
