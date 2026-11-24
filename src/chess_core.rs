use std::num::NonZeroU32;

use bevy::{ecs::system::SystemParam, platform::collections::HashMap, prelude::*};
use bevy_resvg::{
    effects::components::{SvgSize, Unit},
    raster::component::Svg,
};
use serde::Deserialize;

use crate::{
    chess_core::{
        board::{
            AsIndex, BoardCell, BoardVisualInformation, CellPosition, ChessBoardPlugin, Label,
        },
        pieces::{
            CandidateSquare, ChessPiece, ChessPieceAsset, ChessPieceColor, ChessPieceStorage,
            ChessPiecesPlugin, PiecesResourceState,
        },
    },
    fen::{self, InitialGameInformation},
};

pub mod board;
pub mod pieces;

const BOARD_Z: f32 = 0.0;
const BOARD_MARKS_Z: f32 = 0.1;
const PIECE_Z: f32 = 0.2;
const DRAGGING_PIECE_Z: f32 = 0.3;

pub struct ChessCorePlugin {}

#[derive(Resource, Default)]
struct VisualChessBoard {
    labels: HashMap<Label, Entity>,
    cells: HashMap<BoardCell, Entity>,
    pieces: HashMap<BoardCell, Entity>,
}

impl Plugin for ChessCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VisualChessBoard>()
            .init_resource::<ChessGameEngine>()
            .init_resource::<PointerInteraction>()
            .init_resource::<CellHover>()
            .add_plugins(ChessBoardPlugin)
            .add_plugins(ChessPiecesPlugin)
            .add_systems(
                Startup,
                (initialize_materials, initialize_outline_marker).chain(),
            )
            .add_systems(OnEnter(PiecesResourceState::Ready), setup_board)
            .add_systems(Update, initial_resize_pieces)
            .add_systems(
                Update,
                (resize_pieces_on_board_resize, resize_outline_marker)
                    .run_if(resource_changed::<BoardVisualInformation>),
            )
            .add_systems(
                PostUpdate,
                (
                    handle_mouse_interaction,
                    move_dragging_piece,
                    outline_square_on_hover,
                ),
            );
    }
}

fn setup_board(
    mut commands: Commands,

    mut chess_game_engine: ResMut<ChessGameEngine>,
    mut visual_chess_board: ResMut<VisualChessBoard>,

    mut board_visual_information: ResMut<BoardVisualInformation>,
    piece_storage: Res<ChessPieceStorage>,
    chess_piece_assets: Res<Assets<ChessPieceAsset>>,
) {
    const DEFAULT_PIECE_SIZE: u32 = 256;

    let input = "header: orientation=w\n\
                       rnbqkbnr/pppppppp/8/8/3pp3/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 1";
    let initial_game_information = fen::parse(input).unwrap();

    board_visual_information.orientation = initial_game_information.headers.orientation.clone();

    for (cell, piece) in &initial_game_information.pieces {
        let translation = board_visual_information.cell_position(cell);
        let chess_piece_asset = piece_storage
            .load_piece_asset(&piece.name, &chess_piece_assets)
            .unwrap();

        visual_chess_board.pieces.insert(
            cell.clone(),
            commands
                .spawn((
                    piece.clone(),
                    cell.clone(),
                    Svg(chess_piece_asset.images.resolve_color(&piece.color).clone()),
                    SvgSize::both(Unit::pixels(DEFAULT_PIECE_SIZE)),
                    Transform::from_xyz(translation.0, translation.1, PIECE_Z),
                ))
                .id(),
        );
    }

    *chess_game_engine = ChessGameEngine::from(initial_game_information);
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
        let cell_position = board_visual_information.cell_position(cell);
        transform.translation.x = cell_position.0;
        transform.translation.y = cell_position.1;

        sprite.custom_size = Some(Vec2::splat(piece_size));
    }
}

#[derive(SystemParam)]
struct Input<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    window: Single<'w, 's, &'static Window>,
    camera: Single<'w, 's, (&'static Camera, &'static GlobalTransform)>,
    pointer_interaction: ResMut<'w, PointerInteraction>,
}

#[derive(SystemParam)]
struct BoardAndPieces<'w> {
    board: Res<'w, BoardVisualInformation>,
    visual_board: ResMut<'w, VisualChessBoard>,
    piece_storage: Res<'w, ChessPieceStorage>,
    chess_piece_assets: Res<'w, Assets<ChessPieceAsset>>,
}

#[derive(Resource)]
struct InteractionMaterials {
    action_circle: Handle<Mesh>,
    action_color: Handle<ColorMaterial>,

    outline_square: Handle<Mesh>,
    outline_color: Handle<ColorMaterial>,
}

fn initialize_materials(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(InteractionMaterials {
        action_circle: meshes.add(Circle::new(1.0)),
        action_color: materials.add(Color::srgba(0.5, 0.5, 0.5, 0.6)),

        outline_square: meshes.add(Rectangle::new(1.0, 1.0).to_ring(0.04)),
        outline_color: materials.add(Color::WHITE),
    })
}

#[derive(Component)]
struct CellOutlineMarker;

#[derive(Resource, Default)]
struct CellHover {
    cell: Option<BoardCell>,
}

fn initialize_outline_marker(
    mut commands: Commands,
    interaction_materials: Res<InteractionMaterials>,
) {
    commands.spawn((
        CellOutlineMarker,
        Mesh2d(interaction_materials.outline_square.clone()),
        MeshMaterial2d(interaction_materials.outline_color.clone()),
        Transform::from_xyz(0.0, 0.0, BOARD_MARKS_Z),
        Visibility::Hidden,
    ));
}

fn resize_outline_marker(
    board_visual_information: Res<BoardVisualInformation>,
    cell_hover: Res<CellHover>,
    mut outline_marker: Single<&mut Transform, With<CellOutlineMarker>>,
) {
    let Some(cell) = &cell_hover.cell else {
        return;
    };

    let position = board_visual_information.cell_position(cell);
    outline_marker.translation.x = position.0;
    outline_marker.translation.y = position.1;
    outline_marker.scale = Vec3::splat(board_visual_information.cell_size());
}

#[derive(Resource, Default)]
enum PointerInteraction {
    #[default]
    Idle,
    WaitingMove {
        selected_piece: Entity,
        has_released: bool,

        origin_cell: BoardCell,
        candidates: Vec<CandidateSquare>,
        handles: Vec<Entity>,
    },
}

impl PointerInteraction {
    fn reset_to_idle(&mut self, commands: &mut Commands) {
        match self {
            PointerInteraction::Idle => (),
            PointerInteraction::WaitingMove { handles, .. } => {
                handles.iter().for_each(|handle| {
                    commands.entity(*handle).despawn();
                });
                *self = PointerInteraction::Idle;
            }
        }
    }

    fn release_piece(&mut self) {
        match self {
            PointerInteraction::Idle => (),
            PointerInteraction::WaitingMove { has_released, .. } => {
                *has_released = true;
            }
        }
    }
}

#[derive(Component)]
struct DraggingPiece;

fn outline_square_on_hover(
    input: Input,
    board_and_pieces: BoardAndPieces,
    chess_game_engine: Res<ChessGameEngine>,
    mut cell_hover: ResMut<CellHover>,
    outline_marker: Single<(&mut Transform, &mut Visibility), With<CellOutlineMarker>>,

    dragging_piece_query: Query<(), With<DraggingPiece>>,
) {
    let (camera, transform) = input.camera.into_inner();
    let (mut outline_transform, mut outline_visibility) = outline_marker.into_inner();

    let Some(cell) = input
        .window
        .cursor_position()
        .and_then(|cursor_position| camera.viewport_to_world_2d(transform, cursor_position).ok())
        .and_then(|world_position| board_and_pieces.board.locate_cell(world_position))
    else {
        if cell_hover.cell.is_some() {
            *outline_visibility = Visibility::Hidden;
            cell_hover.cell = None;
        }
        return;
    };

    match (&*input.pointer_interaction, &chess_game_engine.board[&cell]) {
        (PointerInteraction::WaitingMove { selected_piece, .. }, Square::Vacant)
            if dragging_piece_query.get(*selected_piece).is_ok() => {}
        (_, Square::Vacant) => {
            *outline_visibility = Visibility::Hidden;
            cell_hover.cell = None;
            return;
        }
        _ => (),
    }

    if cell_hover
        .cell
        .as_ref()
        .is_some_and(|last_hovered| last_hovered == &cell)
    {
        return;
    }

    let position = board_and_pieces.board.cell_position(&cell);
    outline_transform.translation.x = position.0;
    outline_transform.translation.y = position.1;
    outline_transform.scale = Vec3::splat(board_and_pieces.board.cell_size());
    *outline_visibility = Visibility::Visible;
    cell_hover.cell = Some(cell);
}

fn handle_mouse_interaction(
    commands: Commands,

    input: Input,

    interaction_materials: Res<InteractionMaterials>,

    board_and_pieces: BoardAndPieces,
    chess_game_engine: ResMut<ChessGameEngine>,

    dragging_piece_query: Query<(&BoardCell, &mut Transform), With<DraggingPiece>>,
) {
    if input.buttons.just_pressed(MouseButton::Left) {
        on_piece_drag(
            commands,
            input,
            interaction_materials,
            board_and_pieces,
            chess_game_engine,
        );
    } else if input.buttons.just_released(MouseButton::Left) {
        on_piece_release(
            commands,
            input,
            board_and_pieces,
            chess_game_engine,
            dragging_piece_query,
        );
    }
}

fn on_piece_drag(
    mut commands: Commands,
    mut input: Input,

    interaction_materials: Res<InteractionMaterials>,

    board_and_pieces: BoardAndPieces,
    chess_game_engine: ResMut<ChessGameEngine>,
) {
    let (camera, transform) = input.camera.into_inner();

    let Some(selected_cell) = input
        .window
        .cursor_position()
        .and_then(|cursor_position| camera.viewport_to_world_2d(transform, cursor_position).ok())
        .and_then(|world_position| board_and_pieces.board.locate_cell(world_position))
    else {
        return;
    };

    match &*input.pointer_interaction {
        PointerInteraction::WaitingMove {
            origin_cell,
            candidates,
            handles,
            ..
        } if *origin_cell != selected_cell => {
            match candidates
                .iter()
                .find(|candidate| candidate.target_square == selected_cell)
            {
                Some(candidate) => {
                    println!("Going to move here! {}", candidate.target_square);
                    return;
                }
                None => {
                    input.pointer_interaction.reset_to_idle(&mut commands);
                }
            }
        }
        _ => (),
    }

    let Square::Occupied(piece) = &chess_game_engine.board[&selected_cell] else {
        return;
    };

    if piece.color != chess_game_engine.active_color {
        return;
    }

    let Some(piece_entity) = board_and_pieces.visual_board.pieces.get(&selected_cell) else {
        return;
    };
    commands.entity(*piece_entity).insert(DraggingPiece);

    match &mut *input.pointer_interaction {
        PointerInteraction::Idle => {
            let mut handles = vec![];
            let candidates = chess_game_engine.generate_legal_moves(
                selected_cell.clone(),
                &board_and_pieces.piece_storage,
                &board_and_pieces.chess_piece_assets,
            );

            for candidate_move in &candidates {
                let position = board_and_pieces
                    .board
                    .cell_position(&candidate_move.target_square);

                handles.push(
                    commands
                        .spawn((
                            Mesh2d(interaction_materials.action_circle.clone()),
                            MeshMaterial2d(interaction_materials.action_color.clone()),
                            Transform::from_xyz(position.0, position.1, BOARD_MARKS_Z)
                                .with_scale(Vec3::splat(board_and_pieces.board.cell_size() * 0.15)),
                        ))
                        .id(),
                );
            }

            *input.pointer_interaction = PointerInteraction::WaitingMove {
                selected_piece: *piece_entity,
                has_released: false,
                origin_cell: selected_cell,
                candidates,
                handles,
            };
        }
        PointerInteraction::WaitingMove { has_released, .. } => {
            *has_released = true;
        }
    }
}

fn on_piece_release(
    mut commands: Commands,
    mut input: Input,
    board_and_pieces: BoardAndPieces,
    chess_game_engine: ResMut<ChessGameEngine>,

    mut dragging_piece_query: Query<(&BoardCell, &mut Transform), With<DraggingPiece>>,
) {
    let PointerInteraction::WaitingMove {
        selected_piece,
        has_released,
        candidates,
        ..
    } = &mut *input.pointer_interaction
    else {
        return;
    };

    let Ok((origin_cell, mut transform)) = dragging_piece_query.get_mut(*selected_piece) else {
        return;
    };

    let (camera, camera_transform) = input.camera.into_inner();

    let Some(selected_cell) = input
        .window
        .cursor_position()
        .and_then(|cursor_position| {
            camera
                .viewport_to_world_2d(camera_transform, cursor_position)
                .ok()
        })
        .and_then(|world_position| board_and_pieces.board.locate_cell(world_position))
    else {
        reset_drag_piece(
            &mut commands,
            *selected_piece,
            origin_cell,
            &mut transform,
            &board_and_pieces.board,
        );
        input.pointer_interaction.reset_to_idle(&mut commands);
        return;
    };

    reset_drag_piece(
        &mut commands,
        *selected_piece,
        origin_cell,
        &mut transform,
        &board_and_pieces.board,
    );

    match candidates
        .iter()
        .find(|candidate| candidate.target_square == selected_cell)
    {
        Some(candidate) => {
            println!("dropped to {}!", candidate.target_square);
        }
        None => {
            if *has_released && *origin_cell == selected_cell {
                input.pointer_interaction.reset_to_idle(&mut commands);
            } else {
                input.pointer_interaction.release_piece();
            }
        }
    }
}

fn move_dragging_piece(
    mut commands: Commands,
    mut input: Input,
    board_visual_information: Res<BoardVisualInformation>,
    dragging_piece: Single<(Entity, &BoardCell, &mut Transform), With<DraggingPiece>>,
) {
    if !input.buttons.pressed(MouseButton::Left) {
        return;
    }

    let (camera, camera_transform) = input.camera.into_inner();
    let (piece, board_cell, mut transform) = dragging_piece.into_inner();

    let cursor_position = match input.window.cursor_position() {
        Some(position) => position,
        None => {
            reset_drag_piece(
                &mut commands,
                piece,
                board_cell,
                &mut transform,
                &board_visual_information,
            );
            input.pointer_interaction.release_piece();
            return;
        }
    };

    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    transform.translation = world_position.extend(DRAGGING_PIECE_Z);
}

fn reset_drag_piece(
    commands: &mut Commands,
    piece: Entity,
    board_cell: &BoardCell,
    transform: &mut Transform,
    board_visual_information: &BoardVisualInformation,
) {
    commands.entity(piece).remove::<DraggingPiece>();

    let position = board_visual_information.cell_position(board_cell);
    transform.translation.x = position.0;
    transform.translation.y = position.1;
    transform.translation.z = 0.2;
}

/// The chess game engine that provides functionality of managing chess pieces, moves, rules, states
/// and other important things. This structure is responsible to ensure the correctness of the game.
#[derive(Resource, Clone)]
pub struct ChessGameEngine {
    /// The active states at current moment of the game.
    states: ChessGameStates,

    /// The active color tells who is going to move.
    active_color: ChessPieceColor,

    /// The clock of halfmoves since last captrue or pawn advance.
    ///
    /// It was made for 50-move rule.
    halfmove_clock: u32,

    /// The total number of full moves, starting from 1. Increments after each move from the black side.
    fullmove_number: NonZeroU32,

    // moves: Vec<MoveInfo>,
    /// The current chess board.
    board: ChessBoard,
}

impl ChessGameEngine {
    pub fn generate_legal_moves(
        &self,
        cell: BoardCell,
        piece_storage: &ChessPieceStorage,
        chess_piece_assets: &Assets<ChessPieceAsset>,
    ) -> Vec<CandidateSquare> {
        let Square::Occupied(piece) = &self.board[&cell] else {
            return vec![];
        };

        let Some(asset) = piece_storage.load_piece_asset(&piece.name, chess_piece_assets) else {
            warn!("The asset of chess piece is absent!");
            return vec![];
        };

        let mut candidates = vec![];

        for movement_rule in &asset.definition.movement_rules {
            candidates.extend(movement_rule.generate_candidate_squares(&cell, piece, self));
        }

        // TODO: these candidates are not legal moves
        // to filter from these candidates need to make a move
        // and figure out whether the move is legal or not (by verifying
        // some properties using the ChessGameEngine)

        candidates
    }
}

impl Default for ChessGameEngine {
    fn default() -> Self {
        // INFO: these values are not meaningful and are just defaults.
        // To ensure the correctness use EFEN parser and make the chess engine by it.
        Self {
            states: ChessGameStates::default(),
            active_color: ChessPieceColor::default(),
            halfmove_clock: 0,
            fullmove_number: NonZeroU32::new(1).unwrap(),
            board: ChessBoard::default(),
        }
    }
}

impl From<InitialGameInformation> for ChessGameEngine {
    fn from(information: InitialGameInformation) -> Self {
        let mut states = ChessGameStates::default();

        if information.available_castles.black.king_side {
            states.insert(Side::Black(StateName::kingside_castling()), "true");
        }

        if information.available_castles.black.queen_side {
            states.insert(Side::Black(StateName::queenside_castling()), "true");
        }

        if information.available_castles.white.king_side {
            states.insert(Side::White(StateName::kingside_castling()), "true");
        }

        if information.available_castles.white.queen_side {
            states.insert(Side::White(StateName::queenside_castling()), "true");
        }

        if let Some(en_passant_info) = &information.en_passant_info {
            let side = match en_passant_info.for_side {
                ChessPieceColor::Black => Side::Black,
                ChessPieceColor::White => Side::White,
            };

            states.insert(
                side(StateName::en_passant_target_square()),
                en_passant_info.target_square.to_string(),
            );

            states.insert(
                side(StateName::en_passant_capture_piece()),
                en_passant_info.capture_piece.to_string(),
            );
        }

        Self {
            states,
            active_color: information.active_color,
            halfmove_clock: information.halfmove_clock,
            fullmove_number: information.fullmove_number,
            board: ChessBoard::from(&information.pieces),
        }
    }
}

/// The collection of active states inside of the chess game.
///
/// These states might control piece movement, their capture and other important thing.
/// It's one of alternative ways to describe chess rules instead of hardcoding them.
///
/// Each state is unique by both its belonging side and name.
#[derive(Default, Clone)]
struct ChessGameStates {
    states: HashMap<Side<StateName>, StateValue>,
}

impl ChessGameStates {
    pub fn get<SN>(&self, key: Side<SN>) -> Option<&StateValue>
    where
        SN: Into<StateName>,
    {
        self.states.get(&key.map(<SN as Into<StateName>>::into))
    }

    /// Insterts a new state with provided key and value.
    ///
    /// If there's an old value with the same key, it'll be returned.
    fn insert<SN, SV>(&mut self, key: Side<SN>, value: SV) -> Option<StateValue>
    where
        SN: Into<StateName>,
        SV: Into<StateValue>,
    {
        self.states
            .insert(key.map(<SN as Into<StateName>>::into), value.into())
    }
}

/// The state name representation.
///
/// Instead of hardcoded values or enumerations it provides possibility to use
/// strings. They're flexible regardless to less control of correctness.
///
/// To avoid mutability on any level, uses [`Box<str>`].
#[derive(Debug, PartialEq, Eq, Hash, Clone, Deserialize)]
#[serde(from = "String")]
pub struct StateName(Box<str>);

impl From<&str> for StateName {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for StateName {
    fn from(value: String) -> Self {
        Self(value.into_boxed_str())
    }
}

impl StateName {
    /// Retrieves the state name of the king side castling.
    fn kingside_castling() -> Self {
        Self("kingside_castling".into())
    }

    /// Retrieves the state name of the queen side castling.
    fn queenside_castling() -> Self {
        Self("queenside_castling".into())
    }

    /// Retrieves the state name of the en passant target square.
    fn en_passant_target_square() -> Self {
        Self("en_passant_target_square".into())
    }

    /// Retrieves the state name of the en passant capture piece position.
    fn en_passant_capture_piece() -> Self {
        Self("en_passant_capture_piece".into())
    }
}

/// The state value representation.
///
/// Instead of hardcoded values or enumerations it provides possibility to use
/// strings. They're flexible regardless to less control of correctness.
///
/// To avoid mutability on any level, uses [`Box<str>`].
#[derive(Debug, PartialEq, Eq, Clone, Deserialize)]
#[serde(from = "String")]
pub struct StateValue(Box<str>);

impl StateValue {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StateValue {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for StateValue {
    fn from(value: String) -> Self {
        Self(value.into_boxed_str())
    }
}

/// Describes the state of a chess game.
///
/// Different states can change behavior of the game.
#[derive(PartialEq, Eq, Hash, Clone, Debug, Deserialize)]
pub enum Side<T> {
    /// The state belongs to the white side.
    White(T),

    /// The state belongs to the black side.
    Black(T),

    /// The state belongs to the white and the black sides.
    ///
    /// NOTE: it's not the same as using [Side::White] and [Side::Black] together.
    /// [Side::Both] means that a state is shared between wite and black sides.
    /// If one side consumes the state, an opposite side won't use this state anymore.
    Both(T),
}

impl<T> Side<T> {
    fn map<R>(self, f: impl FnOnce(T) -> R) -> Side<R> {
        match self {
            Side::White(t) => Side::White(f(t)),
            Side::Black(t) => Side::Black(f(t)),
            Side::Both(t) => Side::Both(f(t)),
        }
    }
}

/// The chess board representation.
///
/// Instead of manually modifying array the structrue provides high-level API
/// to modfy the chess board.
#[derive(Clone)]
struct ChessBoard {
    /// The one-dimensional array represents the whole board, divided by ranks (rows).
    ///
    /// To disambigue, the orientation is white. In other words, the top left cell is 'a8'
    /// the bottom right cell is 'h1'. And the orientation doesn't change over time, only
    /// visual representation flips while the original board remains as is.
    ///
    /// Board representation:
    ///
    /// '8' \[ 0] \[ 1] \[ 2] \[ 3] \[ 4] \[ 5] \[ 6] \[ 7]
    /// '7' \[ 8] \[ 9] \[10] \[11] \[12] \[13] \[14] \[15]
    /// '6' \[16] \[17] \[18] \[19] \[20] \[21] \[22] \[23]
    /// '5' \[24] \[25] \[26] \[27] \[28] \[29] \[30] \[31]
    /// '4' \[32] \[33] \[34] \[35] \[36] \[37] \[38] \[39]
    /// '3' \[40] \[41] \[42] \[43] \[44] \[45] \[46] \[47]
    /// '2' \[48] \[49] \[50] \[51] \[52] \[53] \[54] \[55]
    /// '1' \[56] \[57] \[58] \[59] \[60] \[61] \[62] \[63]
    ///  ---  'a'   'b'   'c'   'd'   'e'   'f'   'g'   'h'
    cells: [Square; 64],
}

impl ChessBoard {
    const CELLS_IN_ROW: usize = 8;

    /// Retrieves an actual index of board square by a conventional [BoardCell].
    fn get_index_by(board_cell: &BoardCell) -> usize {
        Self::CELLS_IN_ROW * board_cell.row.as_index() + board_cell.col.as_index()
    }
}

impl std::ops::Index<&BoardCell> for ChessBoard {
    type Output = Square;
    fn index(&self, index: &BoardCell) -> &Self::Output {
        &self.cells[Self::get_index_by(index)]
    }
}

impl std::ops::IndexMut<&BoardCell> for ChessBoard {
    fn index_mut(&mut self, index: &BoardCell) -> &mut Self::Output {
        &mut self.cells[Self::get_index_by(index)]
    }
}

/// This enumeration represents the board square and answers, whether
/// the square is occupied or vacant.
///
/// Although it's possible to use with [`Option<T>`], but it's not the
/// chess's conventional way.
#[derive(Default, Clone)]
enum Square {
    #[default]
    Vacant,
    Occupied(ChessPiece),
}

impl Default for ChessBoard {
    fn default() -> Self {
        Self {
            cells: [const { Square::Vacant }; 64],
        }
    }
}

impl From<&HashMap<BoardCell, ChessPiece>> for ChessBoard {
    fn from(table: &HashMap<BoardCell, ChessPiece>) -> Self {
        let mut board = Self::default();

        for (cell, piece) in table {
            board[cell] = Square::Occupied(piece.clone());
        }

        board
    }
}
