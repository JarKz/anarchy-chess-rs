use bevy::{asset::AssetLoader, platform::collections::HashMap, prelude::*};
use bevy_resvg::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
#[derive(Default, Clone)]
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
#[derive(Debug, Deserialize, Serialize)]
pub struct ChessPieceDefinition {
    /// Short name of the chess piece, usually an unique name.
    pub short_name: String,

    /// Full name of the chess piece to display to a user.
    pub full_name: String,

    /// Images of a chess piece, used to display on screen.
    images: ChessPieceImages,
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
