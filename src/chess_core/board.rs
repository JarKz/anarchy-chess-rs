use bevy::{platform::collections::HashMap, prelude::*, sprite::Anchor, window::WindowResized};

use crate::chess_core::ChessBoard;

/// The pluign of the chess board.
///
/// It provides the chess board with labels, a helper struct to
/// correctly place pieces (and not only) on squares.
pub struct ChessBoardPlugin;

impl Plugin for ChessBoardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardVisualInformation>()
            .add_systems(Startup, create_board)
            .add_systems(Update, on_window_resize)
            .add_systems(
                Update,
                (resize_board, reposition_labels)
                    .run_if(resource_changed::<BoardVisualInformation>),
            );
    }
}

/// Since the chess board must be always visible on the window, it should follow the window size.
///
/// Of course, there's a minimal size of the chess board (more information in
/// [BoardVisualInformation]).
fn on_window_resize(
    mut resize_reader: MessageReader<WindowResized>,
    mut board_settings: ResMut<BoardVisualInformation>,
) {
    for event in resize_reader.read() {
        board_settings.window_size = UVec2::new(event.width as u32, event.height as u32);
    }
}

/// Resizes the chess board after changing the dimension of [`BoardVisualInformation`].
fn resize_board(
    board_visual_information: Res<BoardVisualInformation>,
    board_query: Query<(&BoardCell, &mut Transform), With<BoardSquare>>,
) {
    let cell_size = board_visual_information.cell_size();

    for (cell, mut transform) in board_query {
        let cell_index = board_visual_information.cell_index(cell);
        transform.scale = Vec3::splat(cell_size);

        let cell_position = board_visual_information.cell_position(cell_index.0, cell_index.1);
        transform.translation.x = cell_position.0;
        transform.translation.y = cell_position.1;
    }
}

/// Correctly positions board labels after the board visual information change.
fn reposition_labels(
    board_visual_information: Res<BoardVisualInformation>,
    label_query: Query<(&Label, &mut Transform)>,
) {
    for (label, mut transform) in label_query {
        let label_position = match label.symbol {
            'a'..='h' => {
                let index = label.symbol as u32 - 'a' as u32;
                board_visual_information.label_position(
                    index as usize,
                    LabelCategory::ColDefinition,
                    &board_visual_information.orientation,
                )
            }
            '1'..='8' => {
                let index = label.symbol as u32 - '1' as u32;
                board_visual_information.label_position(
                    index as usize,
                    LabelCategory::RowDefinition,
                    &board_visual_information.orientation,
                )
            }
            _ => unreachable!(),
        };

        transform.translation.x = label_position.0;
        transform.translation.y = label_position.1;
    }
}

/// The label of the chess board, denoting the row number or column symbol.
#[derive(Component, Hash, PartialEq, Eq, Clone)]
pub struct Label {
    symbol: char,
}

/// The component that marks a board square as actual square.
///
/// It avoids misuse of the [BoardCell] while querying.
#[derive(Component)]
struct BoardSquare;

/// The board cell, containing a row number and a column symbol.
///
/// Instead of indexing, which can can be ambiguous, uses a concrete position
/// containing a row number and a column symbol. This pair cannot be misused
/// from different orientations.
#[derive(Component, Hash, PartialEq, Eq, Clone)]
pub struct BoardCell {
    row: char,
    col: char,
}

impl From<(char, char)> for BoardCell {
    fn from(value: (char, char)) -> Self {
        Self {
            row: value.0,
            col: value.1,
        }
    }
}

/// The board visual information, helps to correctly resize the chess board,
/// place squares, pieces and other objects.
///
/// It follows the window size and have its minimal and maximal sizes:
///  - min: 600x600
///  - max: 1200x1200
///
/// Also it have orientation, so squares, labels, pieces and other objects
/// can be positioned correctly using [BoardCell] which denotes an unique
/// square on the board instead of simple indexing.
#[derive(Resource)]
pub struct BoardVisualInformation {
    window_size: UVec2,

    min_board_size: u32,
    max_board_size: u32,

    orientation: BoardOrientation,
}

impl Default for BoardVisualInformation {
    fn default() -> Self {
        Self {
            window_size: (1280, 720).into(),
            min_board_size: 600,
            max_board_size: 1200,
            orientation: BoardOrientation::WhiteSide,
        }
    }
}

/// The board orientation defines on which side the board shows: on the white or on the black side.
pub enum BoardOrientation {
    WhiteSide,
    BlackSide,
}

impl BoardVisualInformation {
    const CELLS_IN_ROW: f32 = 8.0;
    const LABEL_PADDING: f32 = 30.0;

    /// Returns a full board size including labels.
    ///
    /// To get an actual board size (only squares), use [BoardVisualInformation::cell_board_size].
    fn board_size(&self) -> f32 {
        std::cmp::min(self.window_size.x, self.window_size.y)
            .clamp(self.min_board_size, self.max_board_size) as f32
    }

    /// Returns a board size without labels.
    ///
    /// To get a full board size (with labels), use [BoardVisualInformation::board_size].
    fn cell_board_size(&self) -> f32 {
        self.board_size() - Self::LABEL_PADDING
    }

    /// Returns a size of the board square.
    pub fn cell_size(&self) -> f32 {
        self.cell_board_size() / Self::CELLS_IN_ROW
    }

    /// Retrieves a position (in coordinates) of a given square, using its indices.
    ///
    /// Helpful information:
    ///
    ///  - Indices are 0-based.
    ///  - The row start from top of the board regardless to the orientation.
    ///  - The column stats form left of the board regardless to the orientation.
    ///
    ///  ## Return
    ///
    ///  In result returns an actual coordinate of a cell in XY, and it's a center of
    ///  a square.
    pub fn cell_position(&self, row: u32, col: u32) -> (f32, f32) {
        let board_size = self.board_size();
        let cell_size = self.cell_size();

        let top_left_position = (
            -board_size / 2.0 + Self::LABEL_PADDING + cell_size / 2.0,
            board_size / 2.0 - cell_size / 2.0,
        );

        (
            top_left_position.0 + cell_size * col as f32,
            top_left_position.1 - cell_size * row as f32,
        )
    }

    /// Retrieves a cell name a given square, using its indices.
    ///
    /// Helpful information:
    ///
    ///  - Indices are 0-based.
    ///  - The row start from top of the board regardless to the orientation.
    ///  - The column stats form left of the board regardless to the orientation.
    ///  - The orientation used to find a correct cell name.
    ///
    ///  ## Return
    ///
    ///  In result returns [BoardCell] denoting an actual square regarding to the orientation.
    pub fn cell_name(&self, mut row: u32, mut col: u32) -> BoardCell {
        match self.orientation {
            BoardOrientation::WhiteSide => {
                row = Self::CELLS_IN_ROW as u32 - row - 1;
            }
            BoardOrientation::BlackSide => {
                col = Self::CELLS_IN_ROW as u32 - col - 1;
            }
        }

        BoardCell {
            row: (b'1' + row as u8) as char,
            col: (b'a' + col as u8) as char,
        }
    }

    /// Retrieves a cell indices of a given [BoardCell].
    ///
    /// Helpful information:
    ///
    ///  - Indices are 0-based.
    ///  - The row start from top of the board regardless to the orientation.
    ///  - The column stats form left of the board regardless to the orientation.
    ///  - The orientation used to find a correct cell indices.
    ///
    ///  ## Return
    ///
    ///  In result returns a pair (row, column).
    pub fn cell_index(&self, board_cell: &BoardCell) -> (u32, u32) {
        let mut row = board_cell.row as u32 - '1' as u32;
        let mut col = board_cell.col as u32 - 'a' as u32;

        match &self.orientation {
            BoardOrientation::WhiteSide => {
                row = Self::CELLS_IN_ROW as u32 - row - 1;
            }
            BoardOrientation::BlackSide => {
                col = Self::CELLS_IN_ROW as u32 - col - 1;
            }
        }

        (row, col)
    }

    /// Retrieves a position (in coordinates) of a given label's index.
    ///
    /// Helpful information:
    ///
    ///  - Indices are 0-based (from '1' to '8' or from 'a' to 'h').
    ///  - The label category defines whether it's the row's or column's label.
    ///  - The board orientation uses to find correct position of a label.
    ///
    ///  ## Return
    ///
    ///  In result returns a pair (x, y).
    fn label_position(
        &self,
        index: usize,
        label_category: LabelCategory,
        orientation: &BoardOrientation,
    ) -> (f32, f32) {
        let label_direction = LabelDirection::new_from(&label_category, orientation);

        let correct_index = match label_direction {
            LabelDirection::StartToEnd => index,
            LabelDirection::EndToStart => Self::CELLS_IN_ROW as usize - index - 1,
        };

        let board_size = self.board_size();
        let cell_size = self.cell_size();

        match label_category {
            LabelCategory::RowDefinition => (
                -board_size / 2.0 + Self::LABEL_PADDING / 2.0,
                board_size / 2.0 - cell_size / 2.0 - cell_size * correct_index as f32,
            ),
            LabelCategory::ColDefinition => (
                -board_size / 2.0
                    + Self::LABEL_PADDING
                    + cell_size / 2.0
                    + cell_size * correct_index as f32,
                -board_size / 2.0 + Self::LABEL_PADDING / 2.0,
            ),
        }
    }
}

/// The label category defines to which side it belongs: to rows or to columns.
///
/// It helps to determine where to put a label only based on indices instead of
/// symbols.
enum LabelCategory {
    RowDefinition,
    ColDefinition,
}

/// The label direction defines from where to start.
enum LabelDirection {
    /// Definitions:
    ///  - for rows it means start from '1' at top and end with '8' at bottom.
    ///  - for columns it means start from 'a' at left and end with 'h' at right.
    StartToEnd,

    /// Definitions:
    ///  - for rows it means start from '8' at top and end with '1' at bottom.
    ///  - for columns it means start from 'h' at left and end with 'a' at right.
    EndToStart,
}

impl LabelDirection {
    /// Creates a correct label direction based on a category and the board orientation.
    ///
    /// The provided information is sufficient to determine a correct label direction.
    fn new_from(category: &LabelCategory, orientation: &BoardOrientation) -> Self {
        let mut label_direction = match category {
            LabelCategory::RowDefinition => LabelDirection::EndToStart,
            LabelCategory::ColDefinition => LabelDirection::StartToEnd,
        };

        if let BoardOrientation::BlackSide = orientation {
            label_direction.inverse();
        }

        label_direction
    }

    /// Inverses the direction.
    fn inverse(&mut self) {
        match self {
            LabelDirection::StartToEnd => *self = LabelDirection::EndToStart,
            LabelDirection::EndToStart => *self = LabelDirection::StartToEnd,
        }
    }
}

/// The resource containing visual information for board, such as colors, textures and
/// other kinds.
///
/// Used to properly render sprites with less overhead.
#[derive(Resource)]
struct BoardMaterials {
    black: CellMaterials,
    white: CellMaterials,
}

impl BoardMaterials {
    fn new() -> Self {
        Self {
            black: CellMaterials::new_black(),
            white: CellMaterials::new_white(),
        }
    }
}

struct CellMaterials {
    normal: Color,
}

impl CellMaterials {
    fn new_white() -> Self {
        Self {
            normal: Color::srgb_u8(242, 225, 195),
        }
    }

    fn new_black() -> Self {
        Self {
            normal: Color::srgb_u8(195, 160, 130),
        }
    }
}

/// Creates a board with labels on screen based on the board visual information.
fn create_board(
    mut commands: Commands,
    window: Single<&Window>,
    mut chess_board: ResMut<ChessBoard>,
    mut board_visual_information: ResMut<BoardVisualInformation>,
) {
    let board_materials = BoardMaterials::new();
    board_visual_information.window_size = window.physical_size();

    chess_board.labels = create_labels(&mut commands, &board_visual_information);
    chess_board.cells = create_cells(&mut commands, &board_visual_information, &board_materials);

    commands.insert_resource(board_materials);
}

/// Creates labels for the board using the board visual information to correctly place them.
fn create_labels(
    commands: &mut Commands,
    board_visual_information: &BoardVisualInformation,
) -> HashMap<Label, Entity> {
    let mut labels = HashMap::new();

    let mut append_label = |symbol: char, position: (f32, f32)| {
        let label = Label { symbol };

        let label_entity = commands.spawn((
            label.clone(),
            Text2d(symbol.to_string()),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..Default::default()
            },
            Anchor::CENTER,
            Transform::from_xyz(position.0, position.1, 0.0),
        ));

        labels.insert(label, label_entity.id());
    };

    for (index, symbol) in ('1'..='8').enumerate() {
        let label_position = board_visual_information.label_position(
            index,
            LabelCategory::RowDefinition,
            &board_visual_information.orientation,
        );

        append_label(symbol, label_position);
    }

    for (index, symbol) in ('a'..='h').enumerate() {
        let label_position = board_visual_information.label_position(
            index,
            LabelCategory::ColDefinition,
            &board_visual_information.orientation,
        );

        append_label(symbol, label_position);
    }

    labels
}

/// Creates board cells for the board using the board visual information to correctly place them.
fn create_cells(
    commands: &mut Commands,
    board_visual_information: &BoardVisualInformation,
    board_materials: &BoardMaterials,
) -> HashMap<BoardCell, Entity> {
    let mut cells = HashMap::new();
    let cell_size = board_visual_information.cell_size();

    for row in 0..8 {
        for col in 0..8 {
            let is_white = (-1i32).pow(row + col).is_positive();
            let board_cell = board_visual_information.cell_name(row, col);
            let cell_position = board_visual_information.cell_position(row, col);

            let entity = commands.spawn((
                BoardSquare,
                board_cell.clone(),
                Sprite {
                    color: if is_white {
                        board_materials.white.normal
                    } else {
                        board_materials.black.normal
                    },
                    custom_size: Some((1.0, 1.0).into()),
                    ..default()
                },
                Transform::from_xyz(cell_position.0, cell_position.1, 0.0)
                    .with_scale(Vec3::splat(cell_size)),
            ));

            cells.insert(board_cell, entity.id());
        }
    }

    cells
}
