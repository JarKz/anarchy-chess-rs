use bevy::{platform::collections::HashMap, prelude::*, sprite::Anchor, window::WindowResized};

use crate::chess_core::ChessBoard;

pub struct ChessBoardPlugin;

impl Plugin for ChessBoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_board)
            .add_systems(Update, on_window_resize)
            .add_systems(
                Update,
                (resize_board, reposition_labels).run_if(resource_changed::<VisualSettings>),
            );
    }
}

fn on_window_resize(
    mut resize_reader: MessageReader<WindowResized>,
    mut board_settings: ResMut<VisualSettings>,
) {
    for event in resize_reader.read() {
        board_settings.window_size = UVec2::new(event.width as u32, event.height as u32);
    }
}

fn resize_board(
    visual_settings: Res<VisualSettings>,
    board_query: Query<(&BoardCell, &mut Transform)>,
) {
    let cell_size = visual_settings.cell_size();

    for (cell, mut transform) in board_query {
        transform.scale = Vec3::splat(cell_size);

        let cell_position = visual_settings.cell_position(cell.row, cell.col);
        transform.translation.x = cell_position.0;
        transform.translation.y = cell_position.1;
    }
}

fn reposition_labels(
    visual_settings: Res<VisualSettings>,
    label_query: Query<(&Label, &mut Transform)>,
) {
    for (label, mut transform) in label_query {
        let label_position = match label.symbol {
            'a'..='h' => {
                let index = label.symbol as u32 - 'a' as u32;
                visual_settings.label_position(
                    index as usize,
                    LabelCategory::ColDefinition,
                    LabelDirection::StartToEnd,
                )
            }
            '1'..='8' => {
                let index = label.symbol as u32 - '1' as u32;
                visual_settings.label_position(
                    index as usize,
                    LabelCategory::RowDefinition,
                    LabelDirection::EndToStart,
                )
            }
            _ => unreachable!(),
        };

        transform.translation.x = label_position.0;
        transform.translation.y = label_position.1;
    }
}

#[derive(Component, Hash, PartialEq, Eq, Clone)]
pub struct Label {
    symbol: char,
}

#[derive(Component, Hash, PartialEq, Eq, Clone)]
pub struct BoardCell {
    row: u32,
    col: u32,
}

impl From<(u32, u32)> for BoardCell {
    fn from(value: (u32, u32)) -> Self {
        Self {
            row: value.0,
            col: value.1
        }
    }
}

#[derive(Resource)]
pub struct VisualSettings {
    window_size: UVec2,

    min_board_size: u32,
    max_board_size: u32,
}

impl VisualSettings {
    const CELLS_IN_ROW: f32 = 8.0;
    const LABEL_PADDING: f32 = 30.0;

    fn board_size(&self) -> f32 {
        std::cmp::min(self.window_size.x, self.window_size.y)
            .clamp(self.min_board_size, self.max_board_size) as f32
    }

    fn cell_board_size(&self) -> f32 {
        self.board_size() - Self::LABEL_PADDING
    }

    fn cell_size(&self) -> f32 {
        self.cell_board_size() / Self::CELLS_IN_ROW
    }

    fn cell_position(&self, row: u32, col: u32) -> (f32, f32) {
        let board_size = self.board_size();
        let cell_size = self.cell_size();

        let top_left_position = (
            -board_size / 2.0 + Self::LABEL_PADDING + cell_size / 2.0,
            board_size / 2.0 - cell_size / 2.0,
        );

        (
            top_left_position.0 + cell_size * row as f32,
            top_left_position.1 - cell_size * col as f32,
        )
    }

    fn label_position(
        &self,
        index: usize,
        label_category: LabelCategory,
        label_direction: LabelDirection,
    ) -> (f32, f32) {
        const TOTAL_CELLS_IN_DIRECTION: usize = 8;

        let correct_index = match label_direction {
            LabelDirection::StartToEnd => index,
            LabelDirection::EndToStart => TOTAL_CELLS_IN_DIRECTION - index - 1,
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

enum LabelCategory {
    RowDefinition,
    ColDefinition,
}

enum LabelDirection {
    StartToEnd,
    EndToStart,
}

#[derive(Resource)]
struct BoardMaterials {
    dark: CellMaterials,
    light: CellMaterials,
}

impl BoardMaterials {
    fn new() -> Self {
        Self {
            dark: CellMaterials::new_dark(),
            light: CellMaterials::new_light(),
        }
    }
}

struct CellMaterials {
    normal: Color,
}

impl CellMaterials {
    fn new_light() -> Self {
        Self {
            normal: Color::srgb_u8(242, 225, 195),
        }
    }

    fn new_dark() -> Self {
        Self {
            normal: Color::srgb_u8(195, 160, 130),
        }
    }
}

fn create_board(
    mut commands: Commands,
    window: Single<&Window>,
    mut chess_board: ResMut<ChessBoard>,
) {
    let board_materials = BoardMaterials::new();
    let visual_settings = VisualSettings {
        window_size: window.physical_size(),
        min_board_size: 600,
        max_board_size: 1200,
    };

    chess_board.labels = create_labels(&mut commands, &visual_settings);
    chess_board.cells = create_cells(&mut commands, &visual_settings, &board_materials);

    commands.insert_resource(visual_settings);
    commands.insert_resource(board_materials);
}

fn create_labels(
    commands: &mut Commands,
    visual_settings: &VisualSettings,
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
        let label_position = visual_settings.label_position(
            index,
            LabelCategory::RowDefinition,
            LabelDirection::EndToStart,
        );

        append_label(symbol, label_position);
    }

    for (index, symbol) in ('a'..='h').enumerate() {
        let label_position = visual_settings.label_position(
            index,
            LabelCategory::ColDefinition,
            LabelDirection::StartToEnd,
        );

        append_label(symbol, label_position);
    }

    labels
}

fn create_cells(
    commands: &mut Commands,
    visual_settings: &VisualSettings,
    board_materials: &BoardMaterials,
) -> HashMap<BoardCell, Entity> {
    let mut cells = HashMap::new();
    let cell_size = visual_settings.cell_size();

    for row in 0..8 {
        for col in 0..8 {
            let is_light = (-1i32).pow(row + col).is_positive();
            let board_cell = BoardCell { row, col };
            let cell_position = visual_settings.cell_position(row, col);

            let entity = commands.spawn((
                board_cell.clone(),
                Sprite {
                    color: if is_light {
                        board_materials.light.normal
                    } else {
                        board_materials.dark.normal
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
