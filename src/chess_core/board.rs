use bevy::{platform::collections::HashMap, prelude::*, sprite::Anchor, window::WindowResized};
use derive_more::Display;
use serde::Deserialize;

use crate::chess_core::{BOARD_Z, VisualChessBoard};

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
        transform.scale = Vec3::splat(cell_size);

        let cell_position = board_visual_information.cell_position(cell);
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
        let label_position =
            board_visual_information.label_position(label, &board_visual_information.orientation);

        transform.translation.x = label_position.0;
        transform.translation.y = label_position.1;
    }
}

/// The label of the chess board, denoting the row number or column symbol.
#[derive(Component, Hash, PartialEq, Eq, Clone)]
pub enum Label {
    Row(Row),
    Column(Column),
}

impl Label {
    fn as_symbol(&self) -> char {
        match self {
            Label::Row(row) => row.label_symbol(),
            Label::Column(col) => col.label_symbol(),
        }
    }
}

impl AsIndex for Label {
    fn as_index(&self) -> usize {
        match self {
            Label::Row(row) => row.as_index(),
            Label::Column(col) => col.as_index(),
        }
    }
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
#[derive(Component, Hash, PartialEq, Eq, Clone, Debug, Deserialize, Display)]
#[serde(try_from = "String")]
#[display("{col}{row}")]
pub struct BoardCell {
    pub col: Column,
    pub row: Row,
}

impl From<(Column, Row)> for BoardCell {
    fn from((col, row): (Column, Row)) -> Self {
        Self { col, row }
    }
}

#[derive(Debug, Display, thiserror::Error)]
pub enum BoardCellParseError {
    #[display("\"{input}\" is not the board cell.")]
    NotBoardCell {
        input: String,
    },
    InvalidColumn(#[from] InvalidColumn),
    InvalidRow(#[from] InvalidRow),
}

impl TryFrom<String> for BoardCell {
    type Error = BoardCellParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.is_ascii() || value.len() != 2 {
            return Err(BoardCellParseError::NotBoardCell { input: value });
        }

        let mut chars = value.chars();
        let col = Column::try_from(
            chars
                .next()
                .expect("The first letter is a board cell column."),
        )?;
        let row = Row::try_from(
            chars
                .next()
                .expect("The second letter is a board cell row."),
        )?;

        Ok(BoardCell { row, col })
    }
}

pub trait LabelSymbol {
    fn label_symbol(&self) -> char;
}

pub trait AsIndex {
    /// Retrieves the index of provided label.
    ///
    /// The index starts from top-left corner of board (i.e. "a8").
    fn as_index(&self) -> usize;
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Deserialize, Display)]
#[serde(try_from = "char")]
pub struct Row(char);

#[allow(unused)]
impl Row {
    pub const R1: Row = Row('1');
    pub const R2: Row = Row('2');
    pub const R3: Row = Row('3');
    pub const R4: Row = Row('4');
    pub const R5: Row = Row('5');
    pub const R6: Row = Row('6');
    pub const R7: Row = Row('7');
    pub const R8: Row = Row('8');

    pub const DEFAULT_RANGE: std::ops::RangeInclusive<char> = Self::R1.0..=Self::R8.0;
}

impl LabelSymbol for Row {
    fn label_symbol(&self) -> char {
        self.0
    }
}

impl AsIndex for Row {
    fn as_index(&self) -> usize {
        (b'8' - self.0 as u8) as usize
    }
}

impl std::ops::Add<i32> for Row {
    type Output = Option<Self>;

    fn add(self, rhs: i32) -> Self::Output {
        // INFO: because row in chess represents from 1 to 8, but
        // in current chess board model indices starts form the top-left
        // corner, which is 8-th row.
        //
        // To move down need to add, and to move above need to substract.
        // Therefore there's need to invert the operation.
        #[allow(clippy::suspicious_arithmetic_impl)]
        let result = self.0 as i32 - rhs;

        if (Row::R1.0 as i32..=Row::R8.0 as i32).contains(&result) {
            Some(Row(result as u8 as char))
        } else {
            None
        }
    }
}

#[derive(Debug, Display)]
#[display("Expected a row within range from 'a' to 'h' (in lower case!), but got '{row}.")]
pub struct InvalidRow {
    row: char,
}

impl std::error::Error for InvalidRow {}

impl TryFrom<char> for Row {
    type Error = InvalidRow;
    fn try_from(row: char) -> std::prelude::v1::Result<Self, Self::Error> {
        if ('1'..='8').contains(&row) {
            Ok(Self(row))
        } else {
            Err(InvalidRow { row })
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Deserialize, Display)]
#[serde(try_from = "char")]
pub struct Column(char);

#[allow(unused)]
impl Column {
    pub const A: Column = Column('a');
    pub const B: Column = Column('b');
    pub const C: Column = Column('c');
    pub const D: Column = Column('d');
    pub const E: Column = Column('e');
    pub const F: Column = Column('f');
    pub const G: Column = Column('g');
    pub const H: Column = Column('h');

    pub const DEFAULT_RANGE: std::ops::RangeInclusive<char> = Self::A.0..=Self::H.0;
}

impl LabelSymbol for Column {
    fn label_symbol(&self) -> char {
        self.0
    }
}

impl AsIndex for Column {
    fn as_index(&self) -> usize {
        (self.0 as u8 - b'a') as usize
    }
}

impl std::ops::Add<i32> for Column {
    type Output = Option<Self>;

    fn add(self, rhs: i32) -> Self::Output {
        let result = self.0 as i32 + rhs;

        if (Self::A.0 as i32..=Self::H.0 as i32).contains(&result) {
            Some(Column(result as u8 as char))
        } else {
            None
        }
    }
}

#[derive(Debug, Display)]
#[display("Expected a column within range from 'a' to 'h' (in lower case!), but got '{column}.")]
pub struct InvalidColumn {
    column: char,
}

impl std::error::Error for InvalidColumn {}

impl TryFrom<char> for Column {
    type Error = InvalidColumn;

    fn try_from(column: char) -> std::prelude::v1::Result<Self, Self::Error> {
        if ('a'..='h').contains(&column) {
            Ok(Self(column))
        } else {
            Err(InvalidColumn { column })
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

    pub orientation: BoardOrientation,
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
#[derive(Default, Clone)]
pub enum BoardOrientation {
    #[default]
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
            col: Column((b'a' + col as u8) as char),
            row: Row((b'1' + row as u8) as char),
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
        let mut row = board_cell.row.label_symbol() as u32 - '1' as u32;
        let mut col = board_cell.col.label_symbol() as u32 - 'a' as u32;

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
    fn label_position(&self, label: &Label, orientation: &BoardOrientation) -> (f32, f32) {
        let mut index = label.as_index();

        if matches!(orientation, BoardOrientation::BlackSide) {
            index = Self::CELLS_IN_ROW as usize - index - 1;
        }

        let board_size = self.board_size();
        let cell_size = self.cell_size();

        match label {
            Label::Row(_) => (
                -board_size / 2.0 + Self::LABEL_PADDING / 2.0,
                board_size / 2.0 - cell_size / 2.0 - cell_size * index as f32,
            ),
            Label::Column(_) => (
                -board_size / 2.0
                    + Self::LABEL_PADDING
                    + cell_size / 2.0
                    + cell_size * index as f32,
                -board_size / 2.0 + Self::LABEL_PADDING / 2.0,
            ),
        }
    }

    pub fn locate_cell(&self, world_position: Vec2) -> Option<BoardCell> {
        let center_position =
            Vec2::splat(self.board_size()) / 2.0 - Vec2::new(Self::LABEL_PADDING, 0.0);
        let transition_matrix = Mat2::from_rows(Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0));

        let local_coords = transition_matrix * world_position + center_position;
        let cell_board_size = self.cell_board_size();

        if !(0.0..cell_board_size).contains(&local_coords.x)
            || !(0.0..cell_board_size).contains(&local_coords.y)
        {
            return None;
        }

        let indices = (local_coords / self.cell_size()).floor();

        Some(self.cell_name(indices.y as u32, indices.x as u32))
    }
}

pub trait CellPosition<T> {
    fn cell_position(&self, key: T) -> (f32, f32);
}

impl CellPosition<(u32, u32)> for BoardVisualInformation {
    fn cell_position(&self, (row, col): (u32, u32)) -> (f32, f32) {
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
}

impl CellPosition<&BoardCell> for BoardVisualInformation {
    fn cell_position(&self, cell: &BoardCell) -> (f32, f32) {
        self.cell_position(self.cell_index(cell))
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
    mut chess_board: ResMut<VisualChessBoard>,
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

    for label in Row::DEFAULT_RANGE
        .map(|symbol| Label::Row(Row(symbol)))
        .chain(Column::DEFAULT_RANGE.map(|symbol| Label::Column(Column(symbol))))
    {
        let label_position =
            board_visual_information.label_position(&label, &board_visual_information.orientation);

        let label_entity = commands.spawn((
            label.clone(),
            Text2d(label.as_symbol().to_string()),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..Default::default()
            },
            Anchor::CENTER,
            Transform::from_xyz(label_position.0, label_position.1, BOARD_Z),
        ));

        labels.insert(label, label_entity.id());
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
            let cell_position = board_visual_information.cell_position((row, col));

            let entity = commands.spawn((
                BoardSquare,
                board_cell.clone(),
                Sprite {
                    color: if is_white {
                        board_materials.white.normal
                    } else {
                        board_materials.black.normal
                    },
                    alpha_mode: bevy::sprite::SpriteAlphaMode::Opaque,
                    custom_size: Some((1.0, 1.0).into()),
                    ..default()
                },
                Transform::from_xyz(cell_position.0, cell_position.1, BOARD_Z)
                    .with_scale(Vec3::splat(cell_size)),
            ));

            cells.insert(board_cell, entity.id());
        }
    }

    cells
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::chess_core::pieces::Vector;

    #[test]
    fn row_arithmetic() {
        let vector = Vector::from([0, 1]);
        let mut origin = BoardCell {
            row: Row::R5,
            col: Column::A,
        };

        origin.row = (origin.row + vector.y()).unwrap();

        assert_eq!(origin.row, Row::R4);

        origin = BoardCell {
            row: Row::R7,
            col: Column::H,
        };

        let vector = Vector::from([0, -1]);
        origin.row = (origin.row + vector.y()).unwrap();
        assert_eq!(origin.row, Row::R8);

        assert!((origin.row + vector.y()).is_none());
    }

    #[test]
    fn col_arithmetic() {
        let vector = Vector::from([1, 0]);
        let mut origin = BoardCell {
            row: Row::R5,
            col: Column::A,
        };

        origin.col = (origin.col + vector.x()).unwrap();

        assert_eq!(origin.col, Column::B);

        origin = BoardCell {
            row: Row::R7,
            col: Column::H,
        };

        assert!((origin.col.clone() + vector.x()).is_none());

        let vector = Vector::from([-1, 0]);
        origin.col = (origin.col + vector.x()).unwrap();

        assert_eq!(origin.col, Column::G);
    }
}
