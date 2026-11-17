use bevy::{platform::collections::HashMap, prelude::*};
use bevy_resvg::{
    effects::components::{SvgSize, Unit},
    raster::component::Svg,
};

use crate::chess_core::pieces::{
    ChessPiece, ChessPieceAsset, ChessPieceName, ChessPiecesPlugin, PiecesResourceState,
};
use crate::chess_core::{
    board::{BoardCell, BoardVisualInformation, ChessBoardPlugin, Label},
    pieces::ChessPieceStorage,
};

pub mod board;
pub mod pieces;

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
            .add_plugins(ChessPiecesPlugin)
            .add_systems(OnEnter(PiecesResourceState::Ready), place_piece)
            .add_systems(Update, initial_resize_pieces)
            .add_systems(
                Update,
                resize_pieces_on_board_resize.run_if(resource_changed::<BoardVisualInformation>),
            );
    }
}

/// TEST METHOD
///
/// It places a specific piece to specific square using [BoardVisualInformation] to verify, whether
/// a piece looks normal or not.
fn place_piece(
    mut commands: Commands,

    board_visual_information: Res<BoardVisualInformation>,
    piece_storage: Res<ChessPieceStorage>,
    chess_piece_assets: Res<Assets<ChessPieceAsset>>,
) {
    const DEFAULT_PIECE_SIZE: u32 = 256;

    let board_cell: BoardCell = ('2', 'a').into();
    let (row, col) = board_visual_information.cell_index(&board_cell);
    let translation = board_visual_information.cell_position(row, col);

    let chess_piece_asset = piece_storage
        .load_piece_asset(ChessPieceName("B".to_string()), &chess_piece_assets)
        .unwrap();

    commands.spawn((
        ChessPiece {
            name: ChessPieceName("B".to_string()),
            color: pieces::ChessPieceColor::White,
        },
        board_cell,
        Svg(chess_piece_asset.images.white.clone()),
        SvgSize::both(Unit::pixels(DEFAULT_PIECE_SIZE)),
        Transform::from_xyz(translation.0, translation.1, 0.0),
    ));
}

/// Since pieces displayed by SVG (using [bevy_resvg]), it's impossible to set a correct custom size
/// of sprite. Therefore need to apply when a sprite added to a piece as component, allowing us to
/// fix the custom size of the sprite.
#[allow(clippy::type_complexity)]
fn initial_resize_pieces(
    board_visual_information: Res<BoardVisualInformation>,
    pieces: Query<(&BoardCell, &mut Sprite, &mut Transform), (With<ChessPiece>, Added<Sprite>)>,
) {
    resize_pieces(board_visual_information, pieces.into_iter());
}

/// The chess board can change its dimensions during window resize. Squares and labels of the board
/// are changed independently, and chess pieces are changing separately.
fn resize_pieces_on_board_resize(
    board_visual_information: Res<BoardVisualInformation>,
    pieces: Query<(&BoardCell, &mut Sprite, &mut Transform), With<ChessPiece>>,
) {
    resize_pieces(board_visual_information, pieces.into_iter());
}

/// An convenient menthod that can be applied for both intial resize and rezise after board's resize.
fn resize_pieces<'a>(
    board_visual_information: Res<BoardVisualInformation>,
    pieces: impl Iterator<Item = (&'a BoardCell, Mut<'a, Sprite>, Mut<'a, Transform>)>,
) {
    let piece_size = board_visual_information.cell_size();

    for (cell, mut sprite, mut transform) in pieces {
        let cell_index = board_visual_information.cell_index(cell);

        let cell_position = board_visual_information.cell_position(cell_index.0, cell_index.1);
        transform.translation.x = cell_position.0;
        transform.translation.y = cell_position.1;

        sprite.custom_size = Some(Vec2::splat(piece_size));
    }
}
