use std::num::NonZeroU32;

use bevy::{asset::AssetLoader, platform::collections::HashMap, prelude::*};
use bevy_resvg::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::chess_core::{
    Side, StateName, board::{BoardCell, Column, Row},
};

/// The plugin of a chess pieces.
///
/// It's separated from board since each chess piece can be so complicated,
/// and mixing them will end up an complicated situation.
///
/// This plugin introduces available chess pieces, providing information about,
/// appropriate assets and way to control visual information of pieces on the
/// board.
pub struct ChessPiecesPlugin;

impl Plugin for ChessPiecesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChessPieceStorage>()
            .init_asset::<ChessPieceAsset>()
            .init_asset_loader::<ChessPieceAssetLoader>()
            .init_state::<PiecesResourceState>()
            .add_systems(Startup, load_pieces)
            .add_systems(
                Update,
                check_assets_loaded.run_if(in_state(PiecesResourceState::Loading)),
            );
    }
}

/// Pieces resource are loaded by [AssetServer] and they might be not ready immediately.
/// This state enumeration can help to call a method only at an appropriate moment, e.g.
/// after loading all assets.
#[derive(States, Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum PiecesResourceState {
    #[default]
    Loading,
    Ready,
}

/// Verifies whether all chess piece assets are loaded or not.
///
/// In success the resource availability state changes to [PiecesResourceState::Ready].
fn check_assets_loaded(
    mut state: ResMut<NextState<PiecesResourceState>>,
    piece_storage: Res<ChessPieceStorage>,
    asset_server: Res<AssetServer>,
) {
    if piece_storage.is_loaded_with_dependencies(&asset_server) {
        state.set(PiecesResourceState::Ready)
    }
}

/// Loads all available chess pieces from assets/pieces folder with.
pub fn load_pieces(mut storage: ResMut<ChessPieceStorage>, asset_server: Res<AssetServer>) {
    let piece_definitions = discover_piece_definitions().unwrap();

    for (piece_name, asset_path) in piece_definitions {
        storage
            .vocabulary
            .insert(ChessPieceName(piece_name), asset_server.load(asset_path));
    }
}

/// The storage of all available chess piece assets.
///
/// Used for requesting a chess piece asset to recieve definition and images.
#[derive(Resource, Default)]
pub struct ChessPieceStorage {
    vocabulary: HashMap<ChessPieceName, Handle<ChessPieceAsset>>,
}

impl ChessPieceStorage {
    pub fn is_loaded_with_dependencies(&self, asset_server: &AssetServer) -> bool {
        self.vocabulary
            .values()
            .all(|handle| asset_server.is_loaded_with_dependencies(handle))
    }

    pub fn load_piece_asset<'a>(
        &self,
        piece_name: ChessPieceName,
        assets: &'a Assets<ChessPieceAsset>,
    ) -> Option<&'a ChessPieceAsset> {
        self.vocabulary
            .get(&piece_name)
            .and_then(move |handle| assets.get(handle))
    }
}

/// An unique chess piece name, used as key to a specific chess piece asset if available.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ChessPieceName(pub String);

/// A short chess piece description, used only for rendering.
#[derive(Component, Clone)]
pub struct ChessPiece {
    pub name: ChessPieceName,
    pub color: ChessPieceColor,
}

/// The color of chess piece, defining the side.
#[derive(Debug, Default, Clone, Deserialize)]
pub enum ChessPieceColor {
    Black,
    #[default]
    White,
}

/// A chess piece asset, containing all important information about it:
///
///  - the description of the chess piece;
///  - images of the chess piece.
///
/// Used as source of truth about information of a piece.
#[derive(Asset, TypePath, Debug)]
pub struct ChessPieceAsset {
    pub definition: ChessPieceDefinition,
    pub images: ChessPieceImageAssets,
}

/// The definition of a chess piece, containing all information about it.
///
/// Used to define available legal moves, captures, special moves and other
/// kind of information.
///
/// It deserialized from `pieces/{short_piece_name}/definition.ron`
#[derive(Debug, Deserialize)]
pub struct ChessPieceDefinition {
    /// The short name of the chess piece, usually an unique name.
    pub short_name: String,

    /// The full name of the chess piece to display to a user.
    pub full_name: String,

    /// The list of movement rules of the chess pieces.
    pub movement_rules: Vec<MovementRule>,

    /// The images of a chess piece, used to display on screen.
    images: ChessPieceImages,
}

/// The definition of moves for the chess piece.
///
/// Instead of thinking about "just moving" better to think
/// about it as "generate candidate squares to which the move
/// can be performed". Only in this case the move rules are
/// absurdly simple.
///
/// In result there are should be available candidate squares
/// that'll be used in next stage of finding legal, attacking
/// and other kind of squares.
#[derive(Debug, Deserialize)]
pub struct MovementRule {
    /// The conditions of the candidate square to which the move
    /// might be performed.
    conditions: Vec<MoveCondition>,

    /// The list of move square generators.
    generators: Vec<MoveGenerator>,

    /// The list of actions that must be performed.
    actions: Vec<Action>,
}

/// The definition of a move condition.
///
/// Some of moves can be performed only after if their conditions are met.
/// Otherwise the move will be discarded and won't be used at current moment.
#[derive(Debug, Deserialize)]
pub enum MoveCondition {
    /// Verifies whether the value is true.
    ///
    /// There are different ways to verify the value:
    ///
    ///  - If the value is the state, then it verifies the existence
    ///    of the state and checks is the value equal `true`.
    ///  - If the value is arbitrary, the verification is based purely
    ///    on logic, and the result might be not expeted. If you're not
    ///    sure, then use [`MoveCondition::Equal`] variant which have
    ///    more predictable result.
    IsTrue(Value),

    /// Verifies is the both provided values are equal.
    ///
    /// To verify the values, all of them converts into a string and
    /// checks is both of strings are equal or not.
    ///
    /// To avoid ambiguity, these strings may have their tag. For instance,
    /// the [`Value::Row`] will have tag "row" before the value (i.e. "row:e").
    Equal(Value, Value),
}

/// The move generator generates candidate squares.
#[derive(Debug, Deserialize)]
pub enum MoveGenerator {
    Vector {
        /// The vector that defines a destination from a current square.
        direction: Vector,

        /// The distance tells how long the vector can be.
        ///
        /// It's basically vector multiplication: $direction \cdot distance$
        max_distance: MoveDistance,
    },
}

#[derive(Debug, Deserialize)]
enum MoveDistance {
    /// The finite amount of a distance.
    Times(NonZeroU32),

    /// The infinite amount of a distance.
    Infinite,
}

/// The value is the generic type that collect all available types
/// for checking and using value. Without it the declarative way
/// of moving pieces cannot be implemented.
///
/// There's a big complexity to perform specific operations. For instance,
/// a try to verify [`MoveCondition::IsTrue`] might not be success for any
/// value type. Therefore the verification is under explicit control and
/// can be performed for available types, even if all types are combined
/// in [Value] enumeration, though.
#[derive(Debug, Deserialize)]
pub enum Value {
    /// The state itself is the value.
    ///
    /// It also means that it can be verified either by its name or its value,
    /// depending on the context.
    State(Side<StateName>),

    /// The secific defined row itself is the value.
    ///
    /// **NOTE:** Cannot be stored in a state.
    Row(Row),

    /// The secific defined column itself is the value.
    ///
    /// **NOTE:** Cannot be stored in a state.
    Column(Column),

    /// The secific defined square itself is the value.
    ///
    /// **NOTE:** Can be stored in a state.
    Square(Square),

    /// The row on which the piece is placed before any move itself is the value.
    ///
    /// **NOTE:** Cannot be stored in a state.
    OriginRow,

    /// The row on which the piece is placed after any move itself is the value.
    ///
    /// **NOTE:** Cannot be stored in a state.
    TargetRow,

    /// The column on which the piece is placed before any move itself is the value.
    ///
    /// **NOTE:** Cannot be stored in a state.
    OriginColumn,

    /// The column on which the piece is placed after any move itself is the value.
    ///
    /// **NOTE:** Cannot be stored in a state.
    TargetColumn,

    /// The piece self color itself is the value.
    ///
    /// Usually this variant uses for verifying the current color of the chess piece.
    SelfColor,

    /// The piece color itself is the value.
    ///
    /// It helps to determine on which side the piece.
    Color(ChessPieceColor),
}

/// The information about square.
///
/// It can be converted into [BoardCell] to store in a state.
#[derive(Debug, Deserialize)]
pub enum Square {
    /// Uses the square where a piece is placed before any move.
    Origin,
    /// Uses the square where a piece is placed after any move.
    Target,
    /// Uses an explicitly defined square.
    Explicit(BoardCell),
    /// Uses the square that is placed somewhere from origin by [`Vector`].
    OffsetFromOrigin(Vector),
}

/// The list of actions that have to be performed.
///
/// These actions must be effective for [crate::chess_core::ChessGameEngine].
/// In other words, if some action actually have a effect for the chess game
/// engine, then it should be registered in the move history. Otherwise it
/// must be skipped. For instance, the king piece on each move can omit action
/// that removes "kingside_castling" state, but the chess game engine registers
/// only when the mentioned state exists, otherwise it'll be skipped.
#[derive(Debug, Deserialize)]
pub enum Action {
    /// Move the piece itself to a selected destination square.
    MoveSelf,

    /// Move a piece form one square to another.
    ///
    /// It also affects any other pieces.
    Move { from: BoardCell, to: BoardCell },

    /// Remove the state.
    RemoveState(Side<StateName>),

    /// Add a state.
    AddState { name: Side<StateName>, value: Value },
}

/// The 2D vector.
///
/// To visualize it, better see the diagram below:
/// |       (0, -1)
/// |          ^
/// |(-1, 0) < O > (1, 0)
/// |          v
/// |       (0, 1)
///
/// The board squares count from the top-left corner, and
/// the vector represents in (X, Y) offset form a current square.
///
/// If:
/// - The X coordinate is positive, it moves right in X count.
/// - The X coordinate is negative, it moves left in X count.
/// - The Y coordinate is positive, it moves below in Y count.
/// - The Y coordinate is negative, it moves above in Y count.
///
/// Also it means that a move can pass through any other pieces,
/// i.e. "jumps over them", like the knight.
#[derive(Debug, Deserialize)]
#[serde(from = "[i32; 2]")]
pub struct Vector([i32; 2]);

impl Vector {
    /// The X_1 (X) component of the 2D vector.
    fn x(&self) -> i32 {
        self.0[0]
    }

    /// The X_2 (Y) component of the 2D vector.
    fn y(&self) -> i32 {
        self.0[1]
    }
}

impl From<[i32; 2]> for Vector {
    fn from(value: [i32; 2]) -> Self {
        Self(value)
    }
}

/// The chess piece image paths as strings.
///
/// Usually used local (resolved) path to an image instead of a path from the asset folder.
/// In other words, the `white_king.svg` path will be resolved as
/// `pieces/{short_piece_name}/white_king.svg`.
#[derive(Debug, Deserialize, Serialize)]
struct ChessPieceImages {
    white: String,
    black: String,
}

/// The loaded images of the chess piece.
///
/// Used only to display on screen.
#[derive(Debug)]
pub struct ChessPieceImageAssets {
    pub white: Handle<SvgAsset>,
    pub black: Handle<SvgAsset>,
}

impl ChessPieceImageAssets {
    pub fn resolve_color(&self, chess_piece_color: &ChessPieceColor) -> &Handle<SvgAsset> {
        match chess_piece_color {
            ChessPieceColor::White => &self.white,
            ChessPieceColor::Black => &self.black,
        }
    }
}

/// Loads a chess piece information from a given path.
///
/// The requirements:
///
///  - Piece information should be placed at `pieces/{short_piece_name}`
///  - The `pieces/{short_piece_name}` contains 3 files:
///     - `definition.ron`
///     - An image of white variant
///     - An image of black variant
///  - The `definition.ron` reads as [ChessPieceDefinition]
///  - The images are in SVG formats.
///
/// If requirements are met, then a chess piece asset will be loaded without any issues.
#[derive(Default, TypePath)]
struct ChessPieceAssetLoader;

#[derive(Debug, Error)]
enum ChessPieceAssetLoaderError {
    #[error("Could not load an asset: {0}")]
    Io(#[from] std::io::Error),

    #[error("Could not parse a path of an asset: {0}")]
    ParseAssetPath(#[from] bevy::asset::ParseAssetPathError),

    #[error("Could not parse RON: {0}")]
    RonSpannedError(#[from] ron::error::SpannedError),
}

impl AssetLoader for ChessPieceAssetLoader {
    type Asset = ChessPieceAsset;
    type Settings = ();
    type Error = ChessPieceAssetLoaderError;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::tasks::ConditionalSendFuture<
        Output = std::prelude::v1::Result<Self::Asset, Self::Error>,
    > {
        async {
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer).await?;
            let definition = ron::de::from_bytes::<ChessPieceDefinition>(&buffer)?;

            Ok(ChessPieceAsset {
                images: ChessPieceImageAssets {
                    white: load_context.load(
                        load_context
                            .path()
                            .resolve_embed_str(&definition.images.white)?,
                    ),
                    black: load_context.load(
                        load_context
                            .path()
                            .resolve_embed_str(&definition.images.black)?,
                    ),
                },
                definition,
            })
        }
    }
}

/// Discovers all available chess pieces in `assets/pieces`.
///
/// At current moment it just uses the `short_piece_name` from
/// `assets/pieces/{short_piece_name}/definition.ron` as unique name of a chess piece.
///
/// Since `AssetServer` cannot easily load immediately a chess piece definition,
/// and the code need to know what is the short name of a chess piece. So it's just an
/// convention.
fn discover_piece_definitions() -> std::io::Result<Vec<(String, String)>> {
    let mut definitions = Vec::new();

    for entry in std::fs::read_dir("assets/pieces")? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if !file_type.is_dir() {
            continue;
        }

        let definition = entry.path().join("definition.ron");

        if !definition.is_file() {
            continue;
        }

        let Some(piece_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };

        definitions.push((
            piece_name.clone(),
            format!("pieces/{piece_name}/definition.ron"),
        ));
    }

    definitions.sort();

    Ok(definitions)
}
