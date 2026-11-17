use bevy::prelude::*;
use bevy_resvg::plugin::SvgPlugin;

use crate::chess_core::ChessCorePlugin;

mod chess_core;
mod fen;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (1280, 720).into(),
                    resize_constraints: WindowResizeConstraints {
                        min_width: 1040.0,
                        min_height: 600.0,
                        max_width: f32::INFINITY,
                        max_height: f32::INFINITY,
                    },
                    ..default()
                }),
                ..Default::default()
            }),
            SvgPlugin
        ))
        .add_systems(Startup, setup)
        .add_plugins(ChessCorePlugin {})
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
