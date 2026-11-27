use bevy::{
    ecs::{query::QueryData, system::SystemParam},
    prelude::*,
};

use crate::chess_core::{
    BOARD_MARKS_Z, BoardAndPieces, BoardEntities, ChessGameEngine, DRAGGING_PIECE_Z,
    OUTLINE_MARK_Z, PIECE_Z, PieceMoved, PieceRemoved, Square,
    board::{BoardCell, BoardGeometry, CellPosition},
    pieces::CandidateMove,
};

pub struct PointerInteractionPlugin;

impl Plugin for PointerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointerInteraction>()
            .init_resource::<HoverCell>()
            .add_systems(
                Startup,
                (initialize_materials, initialize_outline_marker).chain(),
            )
            .add_systems(
                Update,
                (
                    (resize_outline_marker, resize_action_marks)
                        .run_if(resource_changed::<BoardGeometry>),
                    action_mark_appearance,
                ),
            )
            .add_systems(PostUpdate, (handle_mouse_interaction, move_dragging_piece))
            .add_observer(outline_square_on_hover)
            .add_observer(on_piece_drag)
            .add_observer(spawn_action_marks)
            .add_observer(despawn_action_marks)
            .add_observer(on_piece_release)
            .add_observer(process_piece_move)
            .add_observer(return_dragging_piece);
    }
}

#[derive(Resource)]
struct InteractionMaterials {
    action_circle: Handle<Mesh>,
    action_ring: Handle<Mesh>,
    action_color: Handle<ColorMaterial>,

    outline_square: Handle<Mesh>,
    outline_color: Handle<ColorMaterial>,
}

pub fn initialize_materials(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(InteractionMaterials {
        action_circle: meshes.add(Circle::new(0.5)),
        action_ring: meshes.add(Circle::new(0.5).to_ring(0.08)),
        action_color: materials.add(Color::srgba(0.5, 0.5, 0.5, 0.6)),

        outline_square: meshes.add(Rectangle::new(1.0, 1.0).to_ring(0.04)),
        outline_color: materials.add(Color::WHITE),
    })
}

#[derive(Component)]
pub struct CellOutlineMarker;

fn initialize_outline_marker(
    mut commands: Commands,
    interaction_materials: Res<InteractionMaterials>,
) {
    commands.spawn((
        CellOutlineMarker,
        Mesh2d(interaction_materials.outline_square.clone()),
        MeshMaterial2d(interaction_materials.outline_color.clone()),
        Transform::from_xyz(0.0, 0.0, OUTLINE_MARK_Z),
        Visibility::Hidden,
    ));
}

fn resize_outline_marker(
    board_geometry: Res<BoardGeometry>,
    hover_cell: Res<HoverCell>,
    mut outline_marker: Single<&mut Transform, With<CellOutlineMarker>>,
) {
    let Some(cell) = &hover_cell.cell else {
        return;
    };

    outline_marker.translation = board_geometry
        .cell_position(cell)
        .extend(outline_marker.translation.z);
    outline_marker.scale = Vec3::splat(board_geometry.cell_size());
}

#[derive(SystemParam)]
pub struct Input<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub window: Single<'w, 's, &'static Window>,
    pub camera: Single<'w, 's, (&'static Camera, &'static GlobalTransform)>,
}

#[derive(SystemParam)]
pub struct PieceMessageBroker<'w> {
    pub piece_moved_writer: MessageWriter<'w, PieceMoved>,
    pub piece_removed_writer: MessageWriter<'w, PieceRemoved>,
}

#[derive(Event)]
struct LmbPressed {
    position: Vec2,
}

#[derive(Event)]
struct LmbReleased {
    position: Vec2,
}

#[derive(Event)]
struct ProcessPieceMove {
    origin_cell: BoardCell,
    origin_piece_translation: Vec2,
    candidate: CandidateMove,
}

#[derive(Event)]
struct ReturnDraggingPiece;

#[derive(Event)]
struct SpawnActionMarks {
    cells: Vec<BoardCell>,
}

#[derive(Event)]
struct DespawnActionMarks;

#[derive(Event, Resource, Default)]
pub struct HoverCell {
    pub cell: Option<BoardCell>,
}

#[derive(Resource, Default)]
pub enum PointerInteraction {
    #[default]
    Idle,
    WaitingMove {
        selected_piece: Entity,
        has_released: bool,

        origin_cell: BoardCell,
        candidates: Vec<CandidateMove>,
    },
}

impl PointerInteraction {
    fn reset_to_idle(&mut self, commands: &mut Commands) {
        match self {
            PointerInteraction::Idle => (),
            PointerInteraction::WaitingMove { .. } => {
                commands.trigger(DespawnActionMarks);
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

/// Sets the dragging status for a piece to follow the mouse.
#[derive(Component)]
pub struct DraggingPiece;

#[derive(QueryData)]
#[query_data(mutable)]
struct DraggingPieceData {
    entity: Entity,
    cell: &'static BoardCell,
    transform: &'static mut Transform,
}

fn handle_mouse_interaction(
    mut commands: Commands,
    input: Input,
    board_geometry: Res<BoardGeometry>,
) {
    let (camera, camera_transform) = input.camera.into_inner();
    let Some(position) = input.window.cursor_position().and_then(|cursor_position| {
        camera
            .viewport_to_world_2d(camera_transform, cursor_position)
            .ok()
    }) else {
        return;
    };

    commands.trigger(HoverCell {
        cell: board_geometry.locate_cell(position),
    });

    if input.buttons.just_pressed(MouseButton::Left) {
        commands.trigger(LmbPressed { position });
    } else if input.buttons.just_released(MouseButton::Left) {
        commands.trigger(LmbReleased { position });
    }
}

fn outline_square_on_hover(
    event: On<HoverCell>,

    pointer_interaction: Res<PointerInteraction>,
    board_geometry: Res<BoardGeometry>,
    board_entities: Res<BoardEntities>,

    mut hover_cell: ResMut<HoverCell>,
    outline_marker: Single<(&mut Transform, &mut Visibility), With<CellOutlineMarker>>,

    dragging_piece_query: Query<(), With<DraggingPiece>>,
) {
    let (mut outline_transform, mut outline_visibility) = outline_marker.into_inner();

    let mut hide_outline = |hover_cell: &mut HoverCell| {
        *outline_visibility = Visibility::Hidden;
        hover_cell.cell = None;
    };

    let Some(cell) = &event.cell else {
        if hover_cell.cell.is_some() {
            hide_outline(&mut hover_cell);
        }

        return;
    };

    match (&*pointer_interaction, board_entities.pieces.get(cell)) {
        (PointerInteraction::WaitingMove { selected_piece, .. }, None)
            if dragging_piece_query.get(*selected_piece).is_ok() => {}
        (_, None) => {
            hide_outline(&mut hover_cell);
            return;
        }
        _ => (),
    }

    if hover_cell
        .cell
        .as_ref()
        .is_some_and(|last_hovered| last_hovered == cell)
    {
        return;
    }

    outline_transform.translation = board_geometry.cell_position(cell).extend(OUTLINE_MARK_Z);
    outline_transform.scale = Vec3::splat(board_geometry.cell_size());
    *outline_visibility = Visibility::Visible;

    hover_cell.cell = Some(cell.clone());
}

fn on_piece_drag(
    event: On<LmbPressed>,

    mut commands: Commands,
    mut pointer_interaction: ResMut<PointerInteraction>,

    board_and_pieces: BoardAndPieces,
    mut chess_game_engine: ResMut<ChessGameEngine>,
) {
    let Some(selected_cell) = board_and_pieces.geometry.locate_cell(event.position) else {
        return;
    };

    match &*pointer_interaction {
        PointerInteraction::WaitingMove {
            origin_cell,
            candidates,
            ..
        } if *origin_cell != selected_cell => {
            // TODO: better to use filter and figure out whether this is a single move or multiple moves
            if let Some(candidate) = candidates
                .iter()
                .find(|candidate| candidate.target_square == selected_cell)
            {
                commands.trigger(ProcessPieceMove {
                    origin_cell: origin_cell.clone(),
                    candidate: candidate.clone(),
                    origin_piece_translation: board_and_pieces.geometry.cell_position(origin_cell),
                });
            }

            pointer_interaction.reset_to_idle(&mut commands);
        }
        _ => (),
    }

    let Square::Occupied(piece) = &chess_game_engine.board[&selected_cell] else {
        return;
    };

    if piece.color != chess_game_engine.active_color {
        return;
    }

    let Some(piece_entity) = board_and_pieces.entities.pieces.get(&selected_cell) else {
        return;
    };
    commands.entity(*piece_entity).insert(DraggingPiece);

    match &mut *pointer_interaction {
        PointerInteraction::Idle => {
            let candidates = chess_game_engine.generate_legal_moves(
                selected_cell.clone(),
                &board_and_pieces.piece_storage,
                &board_and_pieces.chess_piece_assets,
            );

            commands.trigger(SpawnActionMarks {
                cells: candidates
                    .iter()
                    .map(|candidate| candidate.target_square.clone())
                    .collect(),
            });

            *pointer_interaction = PointerInteraction::WaitingMove {
                selected_piece: *piece_entity,
                has_released: false,
                origin_cell: selected_cell,
                candidates,
            };
        }
        PointerInteraction::WaitingMove { has_released, .. } => {
            *has_released = true;
        }
    }
}

fn on_piece_release(
    event: On<LmbReleased>,

    mut commands: Commands,
    mut pointer_interaction: ResMut<PointerInteraction>,

    board_geometry: Res<BoardGeometry>,
    mut dragging_piece_query: Query<DraggingPieceData, With<DraggingPiece>>,
) {
    let PointerInteraction::WaitingMove {
        selected_piece,
        has_released,
        candidates,
        ..
    } = &mut *pointer_interaction
    else {
        return;
    };

    let Ok(mut dragging_piece_data) = dragging_piece_query.get_mut(*selected_piece) else {
        return;
    };

    let Some(selected_cell) = board_geometry.locate_cell(event.position) else {
        commands.trigger(ReturnDraggingPiece);
        pointer_interaction.reset_to_idle(&mut commands);
        return;
    };

    // TODO: better to use filter and figure out whether this is a single move or multiple moves
    match candidates
        .iter()
        .find(|candidate| candidate.target_square == selected_cell)
    {
        Some(candidate) => {
            commands.entity(*selected_piece).remove::<DraggingPiece>();
            dragging_piece_data.transform.translation.z = PIECE_Z;

            commands.trigger(ProcessPieceMove {
                origin_cell: dragging_piece_data.cell.clone(),
                origin_piece_translation: dragging_piece_data.transform.translation.xy(),
                candidate: candidate.clone(),
            });

            pointer_interaction.reset_to_idle(&mut commands);
        }
        None => {
            commands.trigger(ReturnDraggingPiece);

            if *has_released && *dragging_piece_data.cell == selected_cell {
                pointer_interaction.reset_to_idle(&mut commands);
            } else {
                pointer_interaction.release_piece();
            }
        }
    }
}

fn process_piece_move(
    event: On<ProcessPieceMove>,

    mut commands: Commands,
    mut message_broker: PieceMessageBroker,

    board_geometry: Res<BoardGeometry>,
    mut board_entities: ResMut<BoardEntities>,

    mut chess_game_engine: ResMut<ChessGameEngine>,
) {
    let move_info = chess_game_engine
        .make_move(&event.origin_cell, &event.candidate)
        .expect("The move is legal");

    board_entities.process_moves(
        move_info,
        &board_geometry,
        &event.origin_cell,
        event.origin_piece_translation,
        &mut message_broker,
        &mut commands,
    );
}

fn move_dragging_piece(
    mut commands: Commands,
    input: Input,
    mut pointer_interaction: ResMut<PointerInteraction>,
    dragging_piece: Single<DraggingPieceData, With<DraggingPiece>>,
) {
    if !input.buttons.pressed(MouseButton::Left) {
        return;
    }

    let (camera, camera_transform) = input.camera.into_inner();
    let mut dragging_piece_data = dragging_piece.into_inner();

    let cursor_position = match input.window.cursor_position() {
        Some(position) => position,
        None => {
            commands.trigger(ReturnDraggingPiece);
            pointer_interaction.release_piece();
            return;
        }
    };

    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    dragging_piece_data.transform.translation = world_position.extend(DRAGGING_PIECE_Z);
}

/// Returns the dragging piece to its intial position and removes the dragging status (component).
///
/// In fact, it has a small intersection with [`PointerInteraction`], but they solves different
/// problems. Therefore the [`PointerInteraction`] cannot manage entity's information.
fn return_dragging_piece(
    _event: On<ReturnDraggingPiece>,

    mut commands: Commands,

    board_geometry: Res<BoardGeometry>,
    mut dragging_piece: Single<DraggingPieceData, With<DraggingPiece>>,
) {
    commands
        .entity(dragging_piece.entity)
        .remove::<DraggingPiece>();

    dragging_piece.transform.translation = board_geometry
        .cell_position(dragging_piece.cell)
        .extend(PIECE_Z);
}

#[derive(Component)]
enum ActionMark {
    Circle,
    Ring,
}

impl ActionMark {
    fn scale_by_cell_size(&self, cell_size: f32) -> f32 {
        match self {
            ActionMark::Circle => cell_size * 0.3,
            ActionMark::Ring => cell_size,
        }
    }
}

#[derive(Component)]
struct ActionMarkAppearance {
    scale: f32,
    duration: f32,
    elapsed: f32,
    direction: AnimationDirection,
}

enum AnimationDirection {
    Enter,
    Exit,
}

fn spawn_action_marks(
    event: On<SpawnActionMarks>,
    mut commands: Commands,

    board_geometry: Res<BoardGeometry>,
    board_entities: Res<BoardEntities>,
    interaction_materials: Res<InteractionMaterials>,
) {
    let cell_size = board_geometry.cell_size();

    for cell in &event.cells {
        let (mesh, mark) = if board_entities.pieces.get(cell).is_some() {
            (
                Mesh2d(interaction_materials.action_ring.clone()),
                ActionMark::Ring,
            )
        } else {
            (
                Mesh2d(interaction_materials.action_circle.clone()),
                ActionMark::Circle,
            )
        };

        commands.spawn((
            mesh,
            cell.clone(),
            MeshMaterial2d(interaction_materials.action_color.clone()),
            Transform::from_translation(board_geometry.cell_position(cell).extend(BOARD_MARKS_Z)),
            ActionMarkAppearance {
                scale: mark.scale_by_cell_size(cell_size),
                duration: 0.08,
                elapsed: 0.0,
                direction: AnimationDirection::Enter,
            },
            mark,
        ));
    }
}

fn despawn_action_marks(
    _event: On<DespawnActionMarks>,
    mut commands: Commands,
    action_marks: Query<(Entity, &Transform), With<ActionMark>>,
) {
    for (mark_entity, transform) in action_marks {
        commands.entity(mark_entity).insert(ActionMarkAppearance {
            scale: transform.scale.x,
            duration: 0.08,
            elapsed: 0.0,
            direction: AnimationDirection::Exit,
        });
    }
}

fn action_mark_appearance(
    mut commands: Commands,
    marks: Query<(Entity, &mut ActionMarkAppearance, &mut Transform), With<ActionMark>>,
    time: Res<Time>,
) {
    let delta_secs = time.delta_secs();

    for (mark_entity, mut animation_info, mut transform) in marks {
        animation_info.elapsed = (animation_info.elapsed + delta_secs).min(animation_info.duration);

        let mut progress = animation_info.elapsed / animation_info.duration;

        if matches!(animation_info.direction, AnimationDirection::Exit) {
            progress = 1.0 - progress;
        }

        transform.scale = Vec3::splat(progress * animation_info.scale);

        if animation_info.elapsed == animation_info.duration {
            match animation_info.direction {
                AnimationDirection::Enter => {
                    commands
                        .entity(mark_entity)
                        .remove::<ActionMarkAppearance>();
                }
                AnimationDirection::Exit => commands.entity(mark_entity).despawn(),
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn resize_action_marks(
    board_geometry: Res<BoardGeometry>,
    action_marks: Query<
        (&ActionMark, &BoardCell, &mut Transform),
        (With<ActionMark>, Without<ActionMarkAppearance>),
    >,
) {
    for (mark, cell, mut transform) in action_marks {
        transform.translation = board_geometry.cell_position(cell).extend(BOARD_MARKS_Z);
        transform.scale = Vec3::splat(mark.scale_by_cell_size(board_geometry.cell_size()));
    }
}
