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
            CandidateMove, ChessPiece, ChessPieceAsset, ChessPieceColor, ChessPieceStorage,
            ChessPiecesPlugin, PiecesResourceState,
        },
        pointer_interaction::PointerInteractionPlugin,
    },
    fen::{self, InitialGameInformation},
};

pub mod board;
pub mod pieces;
pub mod pointer_interaction;

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
            .add_plugins(ChessBoardPlugin)
            .add_plugins(ChessPiecesPlugin)
            .add_plugins(PointerInteractionPlugin)
            .add_systems(OnEnter(PiecesResourceState::Ready), setup_board)
            .add_systems(Update, initial_resize_pieces)
            .add_systems(
                Update,
                resize_pieces_on_board_resize.run_if(resource_changed::<BoardVisualInformation>),
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
struct BoardAndPieces<'w> {
    board: Res<'w, BoardVisualInformation>,
    visual_board: ResMut<'w, VisualChessBoard>,
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

pub struct MoveError {
    pub kind: MoveErrorKind,
    pub message: String,
}

pub enum MoveErrorKind {
    MissingPieceAtOrigin,
    MissingPieceAtTarget,
}

impl ChessGameEngine {
    pub fn make_move(
        &mut self,
        origin_cell: &BoardCell,
        candidate_move: &CandidateMove,
    ) -> Result<MoveInfo, MoveError> {
        let mut changes = vec![];

        let Square::Occupied(piece) = self.board[origin_cell].clone() else {
            return Err(MoveError {
                kind: MoveErrorKind::MissingPieceAtOrigin,
                message: "To make a move, the piece should exist at origin cell".to_string(),
            });
        };

        if let Some(capture_behavior) = &candidate_move.capture {
            match capture_behavior {
                pieces::CaptureBehavior::DoNothing => (),
                pieces::CaptureBehavior::RemovePiece => {
                    let Square::Occupied(target_piece) =
                        self.board[&candidate_move.target_square].take()
                    else {
                        return Err(MoveError {
                            kind: MoveErrorKind::MissingPieceAtTarget,
                            message: "The target piece should be exist at target cell to capture"
                                .to_string(),
                        });
                    };

                    changes.push(ChessGameChange::PieceRemoved {
                        piece: target_piece,
                        from: candidate_move.target_square.clone(),
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

    fn is_move_legal(&mut self, origin_cell: &BoardCell, candidate_move: &CandidateMove) -> bool {
        if self.make_move(origin_cell, candidate_move).is_err() {
            return false;
        }

        // TODO: check whether VIPs (very important pieces) are under attack or not.

        self.unmake_move();

        true
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
            .filter(|candidate_move| self.is_move_legal(&cell, candidate_move))
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
struct MoveInfo {
    changes: Vec<ChessGameChange>,
}

#[derive(Clone)]
enum ChessGameChange {
    PieceMoved {
        from: BoardCell,
        to: BoardCell,
    },
    PieceRemoved {
        piece: ChessPiece,
        from: BoardCell,
    },
    AddedState {
        state_key: Side<StateName>,
        state_value: StateValue,
    },
    RemovedState {
        state_key: Side<StateName>,
        state_value: StateValue,
    },
}
