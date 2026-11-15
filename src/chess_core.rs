use bevy::{platform::collections::HashMap, prelude::*};

use crate::chess_core::board::{BoardCell, ChessBoardPlugin, Label};
use crate::chess_core::pieces::ChessPiecesPlugin;

mod board;
mod pieces;

pub struct ChessCorePlugin {}

#[derive(Resource, Default)]
struct ChessBoard {
    labels: HashMap<Label, Entity>,
    cells: HashMap<BoardCell, Entity>,
}

impl Plugin for ChessCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChessBoard>()
            .add_plugins(ChessBoardPlugin)
            .add_plugins(ChessPiecesPlugin);
    }
}
