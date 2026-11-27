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
            AsIndex, BoardCell, BoardGeometry, CellPosition, ChessBoardPlugin, Column, Label, Row,
        },
        pieces::{
            CandidateMove, ChessPiece, ChessPieceAsset, ChessPieceColor, ChessPieceStorage,
            ChessPiecesPlugin, PiecesResourceState, Vector,
        },
        pointer_interaction::{PieceMessageBroker, PointerInteractionPlugin},
    },
    fen::{self, InitialGameInformation},
};

pub mod board;
pub mod pieces;
pub mod pointer_interaction;

const BOARD_Z: f32 = 0.0;
const BOARD_MARKS_Z: f32 = 0.1;
const OUTLINE_MARK_Z: f32 = 0.2;
const PIECE_Z: f32 = 0.3;
const DRAGGING_PIECE_Z: f32 = 0.4;

pub struct ChessCorePlugin {}

impl Plugin for ChessCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardEntities>()
            .init_resource::<ChessGameEngine>()
            .add_message::<PieceRemoved>()
            .add_message::<PieceMoved>()
            .add_plugins(ChessBoardPlugin)
            .add_plugins(ChessPiecesPlugin)
            .add_plugins(PointerInteractionPlugin)
            .add_systems(OnEnter(PiecesResourceState::Ready), setup_board)
            .add_systems(Update, initial_resize_pieces)
            .add_systems(
                Update,
                resize_pieces_on_board_resize.run_if(resource_changed::<BoardGeometry>),
            )
            .add_systems(Update, (remove_piece, start_piece_animation, animate_piece));
    }
}

#[derive(Resource, Default)]
struct BoardEntities {
    labels: HashMap<Label, Entity>,
    cells: HashMap<BoardCell, Entity>,
    pieces: HashMap<BoardCell, Entity>,
}

impl BoardEntities {
    fn process_moves(
        &mut self,
        move_info: MoveInfo,
        board_geometry: &BoardGeometry,
        origin_cell: &BoardCell,
        origin_piece_translation: Vec2,
        piece_message_broker: &mut PieceMessageBroker,
        commands: &mut Commands,
    ) {
        for change in move_info.changes {
            match change {
                ChessGameChange::PieceMoved { from, to } => {
                    if &from == origin_cell {
                        self.move_piece(
                            &from,
                            &to,
                            origin_piece_translation,
                            &mut piece_message_broker.piece_moved_writer,
                            commands,
                        );
                    } else {
                        self.move_piece(
                            &from,
                            &to,
                            board_geometry.cell_position(&from),
                            &mut piece_message_broker.piece_moved_writer,
                            commands,
                        );
                    }
                }
                ChessGameChange::PieceRemoved { from, .. } => {
                    self.remove_piece(&from, &mut piece_message_broker.piece_removed_writer);
                }
                ChessGameChange::AddedState { .. } | ChessGameChange::RemovedState { .. } => (),
            }
        }
    }

    fn move_piece(
        &mut self,
        from: &BoardCell,
        to: &BoardCell,
        piece_translation: Vec2,
        message_writer: &mut MessageWriter<PieceMoved>,
        commands: &mut Commands,
    ) {
        let Some(piece) = self.pieces.remove(from) else {
            return;
        };

        debug_assert!(self.pieces.insert(to.clone(), piece).is_none());
        commands.entity(piece).insert(to.clone());

        message_writer.write(PieceMoved {
            origin: piece_translation,
            target_cell: to.clone(),
            piece,
        });
    }

    fn remove_piece(&mut self, from: &BoardCell, message_writer: &mut MessageWriter<PieceRemoved>) {
        let Some(piece) = self.pieces.remove(from) else {
            return;
        };

        message_writer.write(PieceRemoved { entity: piece });
    }
}

fn setup_board(
    mut commands: Commands,

    mut chess_game_engine: ResMut<ChessGameEngine>,
    mut board_entities: ResMut<BoardEntities>,

    mut board_geometry: ResMut<BoardGeometry>,
    piece_storage: Res<ChessPieceStorage>,
    chess_piece_assets: Res<Assets<ChessPieceAsset>>,
) {
    const DEFAULT_PIECE_SIZE: u32 = 256;

    let input = "header: orientation=w\n\
                       rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let initial_game_information = fen::parse(input).unwrap();

    board_geometry.orientation = initial_game_information.headers.orientation.clone();

    for (cell, piece) in &initial_game_information.pieces {
        let translation = board_geometry.cell_position(cell);
        let chess_piece_asset = piece_storage
            .load_piece_asset(&piece.name, &chess_piece_assets)
            .unwrap();

        board_entities.pieces.insert(
            cell.clone(),
            commands
                .spawn((
                    piece.clone(),
                    cell.clone(),
                    Svg(chess_piece_asset.images.resolve_color(&piece.color).clone()),
                    SvgSize::both(Unit::pixels(DEFAULT_PIECE_SIZE)),
                    Transform::from_translation(translation.extend(PIECE_Z)),
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
    board_geometry: Res<BoardGeometry>,
    pieces: Query<(&BoardCell, &mut Sprite, &mut Transform), (With<ChessPiece>, Added<Sprite>)>,
) {
    resize_pieces(board_geometry, pieces.into_iter());
}

/// The chess board can change its dimensions during window resize. Squares and labels of the board
/// are changed independently, and chess pieces are changing separately.
fn resize_pieces_on_board_resize(
    board_geometry: Res<BoardGeometry>,
    pieces: Query<(&BoardCell, &mut Sprite, &mut Transform), With<ChessPiece>>,
) {
    resize_pieces(board_geometry, pieces.into_iter());
}

/// An convenient menthod that can be applied for both intial resize and rezise after board's resize.
fn resize_pieces<'a>(
    board_geometry: Res<BoardGeometry>,
    pieces: impl Iterator<Item = (&'a BoardCell, Mut<'a, Sprite>, Mut<'a, Transform>)>,
) {
    let piece_size = board_geometry.cell_size();

    for (cell, mut sprite, mut transform) in pieces {
        transform.translation = board_geometry
            .cell_position(cell)
            .extend(transform.translation.z);
        sprite.custom_size = Some(Vec2::splat(piece_size));
    }
}

#[derive(Message)]
pub struct PieceRemoved {
    entity: Entity,
}

fn remove_piece(mut commands: Commands, mut message_reader: MessageReader<PieceRemoved>) {
    for piece_removed in message_reader.read() {
        commands.entity(piece_removed.entity).despawn();
    }
}

#[derive(Message)]
pub struct PieceMoved {
    piece: Entity,
    origin: Vec2,
    target_cell: BoardCell,
}

#[derive(Component)]
pub struct PieceMoveAnimation {
    origin: Vec2,
    target_cell: BoardCell,
    elapsed: f32,
    duration: f32,
}

fn start_piece_animation(mut commands: Commands, mut message_reader: MessageReader<PieceMoved>) {
    for piece_moved in message_reader.read() {
        commands
            .entity(piece_moved.piece)
            .insert(PieceMoveAnimation {
                origin: piece_moved.origin,
                target_cell: piece_moved.target_cell.clone(),
                elapsed: 0.0,
                duration: 0.3,
            });
    }
}

fn animate_piece(
    mut commands: Commands,
    board_geometry: Res<BoardGeometry>,
    animated_pieces: Query<(Entity, &mut PieceMoveAnimation, &mut Transform)>,
    time: Res<Time>,
) {
    let time_delta = time.delta_secs();

    for (piece_entity, mut move_animation, mut transform) in animated_pieces {
        let target = board_geometry.cell_position(&move_animation.target_cell);

        move_animation.elapsed = (move_animation.elapsed + time_delta).min(move_animation.duration);

        let translation = move_animation.origin
            + (target - move_animation.origin) * (move_animation.elapsed / move_animation.duration);

        transform.translation = translation.extend(transform.translation.z);

        if move_animation.elapsed == move_animation.duration {
            commands.entity(piece_entity).remove::<PieceMoveAnimation>();
        }
    }
}

#[derive(SystemParam)]
struct BoardAndPieces<'w> {
    geometry: Res<'w, BoardGeometry>,
    entities: ResMut<'w, BoardEntities>,
    piece_storage: Res<'w, ChessPieceStorage>,
    chess_piece_assets: Res<'w, Assets<ChessPieceAsset>>,
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

    /// The current chess board.
    board: ChessBoard,

    moves: Vec<MoveInfo>,
}

#[derive(Debug, derive_more::Display)]
#[display("{message}: {kind}")]
pub struct MoveError {
    pub kind: MoveErrorKind,
    pub message: String,
}

#[derive(Debug, derive_more::Display)]
pub enum MoveErrorKind {
    #[display("The piece is missing at {origin_cell}")]
    MissingPieceAtOrigin { origin_cell: BoardCell },
    #[display("The piece is missing at {target_cell}")]
    MissingPieceAtTarget { target_cell: BoardCell },
    #[display("The piece is the moving on at {origin_cell}")]
    TryCaptureOriginPiece { origin_cell: BoardCell },
    #[display(
        "The target cell is out of bounds of the board. Origin cell is at {origin_cell} and the vector is {vector}"
    )]
    OutOfBounds {
        origin_cell: BoardCell,
        vector: Vector,
    },
}

impl std::error::Error for MoveError {}

impl ChessGameEngine {
    pub fn make_move(
        &mut self,
        origin_cell: &BoardCell,
        candidate_move: &CandidateMove,
    ) -> Result<MoveInfo, MoveError> {
        let mut changes = vec![];

        let Square::Occupied(piece) = self.board[origin_cell].clone() else {
            return Err(MoveError {
                kind: MoveErrorKind::MissingPieceAtOrigin {
                    origin_cell: origin_cell.clone(),
                },
                message: "To make a move, the piece should exist at origin cell".to_string(),
            });
        };

        if let Some(capture_behavior) = &candidate_move.capture {
            match capture_behavior {
                pieces::CaptureBehavior::DoNothing => (),
                pieces::CaptureBehavior::RemovePiece(target) => {
                    let target_cell = match target {
                        pieces::Square::Origin => return Err(MoveError {
                            kind: MoveErrorKind::TryCaptureOriginPiece { origin_cell: origin_cell.clone() },
                            message: "Tried to remove the moving piece, and it's forbidden in the chess game engine".to_string(),
                        }),
                        pieces::Square::Target => candidate_move.target_square.clone(),
                        pieces::Square::Explicit(board_cell) => {
                            if board_cell == origin_cell {
                                return Err(MoveError {
                                    kind: MoveErrorKind::TryCaptureOriginPiece { origin_cell: origin_cell.clone() },
                                    message: "Tried to remove the moving piece, and it's forbidden in the chess game engine".to_string(),
                                });
                            }

                            board_cell.clone()
                        },
                        pieces::Square::OffsetFromOrigin(vector) => {
                            let row = origin_cell.row.clone() + vector.y();
                            let col = origin_cell.col.clone() + vector.x();

                            let (Some(row), Some(col)) = (row, col) else {
                                return Err(MoveError {
                                    kind: MoveErrorKind::OutOfBounds { origin_cell: origin_cell.clone(), vector:*vector},
                                    message: "Tried to get the cell that is out of bounds of the board".to_string()
                                });
                            };

                            let target_cell = BoardCell { col, row };
                            if &target_cell == origin_cell {
                                return Err(MoveError {
                                    kind: MoveErrorKind::TryCaptureOriginPiece { origin_cell: origin_cell.clone() },
                                    message: "Tried to remove the moving piece, and it's forbidden in the chess game engine".to_string(),
                                });
                            }

                            target_cell
                        },
                    };

                    let Square::Occupied(target_piece) = self.board[&target_cell].take() else {
                        return Err(MoveError {
                            kind: MoveErrorKind::MissingPieceAtTarget { target_cell },
                            message: "The target piece should be exist at target cell to capture"
                                .to_string(),
                        });
                    };

                    changes.push(ChessGameChange::PieceRemoved {
                        piece: target_piece,
                        from: target_cell,
                    })
                }
            }
        }

        for action in &candidate_move.actions {
            match action {
                pieces::Action::MoveSelf => {
                    // INFO: there's not so much verifications because engine assumes that the move
                    // definition is perfectly valid and check only at debug release.
                    debug_assert!(self.board[&candidate_move.target_square].is_vacant());
                    self.board[&candidate_move.target_square] = self.board[origin_cell].take();

                    changes.push(ChessGameChange::PieceMoved {
                        from: origin_cell.clone(),
                        to: candidate_move.target_square.clone(),
                    });
                }
                pieces::Action::Move { from, to } => {
                    // INFO: there's not so much verifications because engine assumes that the move
                    // definition is perfectly valid and check only at debug release.
                    debug_assert!(self.board[to].is_vacant());
                    self.board[to] = self.board[from].take();

                    changes.push(ChessGameChange::PieceMoved {
                        from: from.clone(),
                        to: to.clone(),
                    });
                }
                pieces::Action::RemoveState(state_key) => {
                    if let Some(state_value) = self.states.remove(state_key.clone()) {
                        changes.push(ChessGameChange::RemovedState {
                            state_key: state_key.clone(),
                            state_value,
                        })
                    }
                }
                pieces::Action::AddState { name, value } => {
                    let state_value: StateValue = value
                        .encode_to_string(
                            pieces::ConditionCheckContext {
                                piece: &piece,
                                origin_cell,
                                target_cell: &candidate_move.target_square,
                            },
                            self,
                        )
                        .into();

                    if let Some(state_value) = self.states.insert(name.clone(), state_value.clone())
                    {
                        changes.push(ChessGameChange::RemovedState {
                            state_key: name.clone(),
                            state_value,
                        })
                    }

                    changes.push(ChessGameChange::AddedState {
                        state_key: name.clone(),
                        state_value,
                    })
                }
            }
        }

        // TODO: increment halfmove clock

        match self.active_color {
            ChessPieceColor::Black => {
                self.fullmove_number = self.fullmove_number.saturating_add(1);
                self.active_color = ChessPieceColor::White;
            }
            ChessPieceColor::White => {
                self.active_color = ChessPieceColor::Black;
            }
        }

        let move_info = MoveInfo { changes };
        self.moves.push(move_info.clone());

        Ok(move_info)
    }

    // TODO: restore the move information for visual board representation
    pub fn unmake_move(&mut self) {
        let Some(move_info) = self.moves.pop() else {
            return;
        };

        // TODO: reduce the halfmove clock

        match self.active_color {
            ChessPieceColor::Black => {
                self.active_color = ChessPieceColor::White;
            }
            ChessPieceColor::White => {
                self.active_color = ChessPieceColor::Black;
                if self.fullmove_number.get() > 1 {
                    self.fullmove_number = NonZeroU32::new(self.fullmove_number.get() - 1)
                        .expect("The number is greater than 1");
                }
            }
        }

        for change in move_info.changes.into_iter().rev() {
            match change {
                ChessGameChange::PieceMoved { from, to } => {
                    self.board[&from] = self.board[&to].take();
                }
                ChessGameChange::PieceRemoved { piece, from } => {
                    self.board[&from] = Square::Occupied(piece);
                }
                ChessGameChange::AddedState { state_key, .. } => {
                    self.states.remove(state_key);
                }
                ChessGameChange::RemovedState {
                    state_key,
                    state_value,
                } => {
                    self.states.insert(state_key, state_value);
                }
            }
        }
    }

    fn is_move_legal(
        &mut self,
        origin_cell: &BoardCell,
        candidate_move: &CandidateMove,
        piece_storage: &ChessPieceStorage,
        chess_piece_assets: &Assets<ChessPieceAsset>,
    ) -> bool {
        if self.make_move(origin_cell, candidate_move).is_err() {
            return false;
        }

        let inactive_color = self.active_color.opposite();

        let important_squares: Vec<_> = self
            .board
            .squares()
            .filter_map(|(cell, square)| match square {
                Square::Vacant => None,
                Square::Occupied(chess_piece) => {
                    let Some(asset) =
                        piece_storage.load_piece_asset(&chess_piece.name, chess_piece_assets)
                    else {
                        warn!("The asset of chess piece is absent!");
                        return None;
                    };

                    if chess_piece.color == inactive_color && asset.definition.is_vip {
                        Some(cell)
                    } else {
                        None
                    }
                }
            })
            .collect();

        let attacking_pieces: Vec<_> = self
            .board
            .squares()
            .filter_map(|(cell, square)| match square {
                Square::Vacant => None,
                Square::Occupied(chess_piece) => {
                    if chess_piece.color == self.active_color {
                        Some((cell, chess_piece))
                    } else {
                        None
                    }
                }
            })
            .collect();

        let is_illegal = attacking_pieces
            .into_iter()
            .any(|(origin_cell, chess_piece)| {
                let Some(asset) =
                    piece_storage.load_piece_asset(&chess_piece.name, chess_piece_assets)
                else {
                    warn!("The asset of chess piece is absent!");
                    return false;
                };

                asset
                    .definition
                    .movement_rules
                    .iter()
                    .flat_map(|movement_rule| {
                        movement_rule.generate_candidate_squares(&origin_cell, chess_piece, self)
                    })
                    .any(|candidate_move| {
                        important_squares.contains(&candidate_move.target_square)
                            && candidate_move
                                .capture
                                .as_ref()
                                .is_some_and(|capture_behavior| {
                                    // TODO: the remove piece is actually not the same as square
                                    // (for instance, the en passant takes another square)
                                    matches!(
                                        capture_behavior,
                                        pieces::CaptureBehavior::RemovePiece(_)
                                    )
                                })
                    })
            });

        self.unmake_move();

        !is_illegal
    }

    pub fn generate_legal_moves(
        &mut self,
        cell: BoardCell,
        piece_storage: &ChessPieceStorage,
        chess_piece_assets: &Assets<ChessPieceAsset>,
    ) -> Vec<CandidateMove> {
        let Square::Occupied(piece) = self.board[&cell].clone() else {
            return vec![];
        };

        let Some(asset) = piece_storage.load_piece_asset(&piece.name, chess_piece_assets) else {
            warn!("The asset of chess piece is absent!");
            return vec![];
        };

        let candidates: Vec<_> = asset
            .definition
            .movement_rules
            .iter()
            .flat_map(|movement_rule| movement_rule.generate_candidate_squares(&cell, &piece, self))
            .collect();

        candidates
            .into_iter()
            .filter(|candidate_move| {
                self.is_move_legal(&cell, candidate_move, piece_storage, chess_piece_assets)
            })
            .collect()
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
            moves: vec![],
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
                format!("square:{}", en_passant_info.target_square),
            );

            states.insert(
                side(StateName::en_passant_capture_piece()),
                format!("square:{}", en_passant_info.capture_piece),
            );
        }

        Self {
            states,
            active_color: information.active_color,
            halfmove_clock: information.halfmove_clock,
            fullmove_number: information.fullmove_number,
            board: ChessBoard::from(&information.pieces),
            moves: vec![],
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

    fn remove<SN>(&mut self, key: Side<SN>) -> Option<StateValue>
    where
        SN: Into<StateName>,
    {
        self.states.remove(&key.map(<SN as Into<StateName>>::into))
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

    fn squares(&self) -> impl Iterator<Item = (BoardCell, &Square)> {
        SquareIterator {
            index: 0,
            cells: &self.cells,
        }
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

struct SquareIterator<'a> {
    index: usize,
    cells: &'a [Square; 64],
}

impl<'a> Iterator for SquareIterator<'a> {
    type Item = (BoardCell, &'a Square);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.cells.len() {
            return None;
        }

        let row_index = self.index / ChessBoard::CELLS_IN_ROW;
        let col_index = self.index - ChessBoard::CELLS_IN_ROW * row_index;

        let row = Row::try_from((b'8' - row_index as u8) as char).expect("The row is invalid");
        let col =
            Column::try_from((b'a' + col_index as u8) as char).expect("The column is invalid");

        let iter_element = (BoardCell { col, row }, &self.cells[self.index]);

        self.index += 1;

        Some(iter_element)
    }
}

/// This enumeration represents the board square and answers, whether
/// the square is occupied or vacant.
///
/// Although it's possible to use with [`Option<T>`], but it's not the
/// chess's conventional way.
#[derive(Default, Debug, Clone)]
enum Square {
    #[default]
    Vacant,
    Occupied(ChessPiece),
}

impl Square {
    fn is_vacant(&self) -> bool {
        matches!(self, Square::Vacant)
    }

    fn take(&mut self) -> Square {
        std::mem::replace(self, Square::Vacant)
    }
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

#[derive(Clone)]
pub struct MoveInfo {
    pub changes: Vec<ChessGameChange>,
}

#[derive(Clone)]
pub enum ChessGameChange {
    PieceMoved {
        from: BoardCell,
        to: BoardCell,
    },
    PieceRemoved {
        piece: ChessPiece,
        from: BoardCell,
    },
    #[allow(unused)]
    AddedState {
        state_key: Side<StateName>,
        // I doubt that the state value is needed, but
        // I'll leave it for an obscure case.
        state_value: StateValue,
    },
    RemovedState {
        state_key: Side<StateName>,
        state_value: StateValue,
    },
}
