use bevy::{platform::collections::HashMap, prelude::*};

use crate::chess_core::{board::BoardCell, ChessBoard};

pub struct ChessPiecesPlugin;

impl Plugin for ChessPiecesPlugin {
    fn build(&self, app: &mut App) {
        todo!()
    }
}

#[derive(Component, Clone)]
pub struct ChessPiece {
    pub name: String,
    pub color: ChessPieceColor,
}

#[derive(Default, Clone)]
pub enum ChessPieceColor {
    Dark,
    #[default]
    Light,
}

fn clear_board(commands: &mut Commands, chess_board: Res<ChessBoard>) {
    for entity in chess_board.cells.values() {
        commands.entity(*entity).remove::<ChessPiece>();
    }
}
