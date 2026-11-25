use bevy::{ecs::system::SystemParam, prelude::*};

use crate::chess_core::{
    BOARD_MARKS_Z, BoardAndPieces, ChessGameEngine, DRAGGING_PIECE_Z, PIECE_Z, PieceMoved,
    PieceRemoved, Square,
    board::{BoardCell, BoardVisualInformation, CellPosition},
    pieces::CandidateMove,
};

pub struct PointerInteractionPlugin;

impl Plugin for PointerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointerInteraction>()
            .init_resource::<CellHover>()
            .add_systems(
                Startup,
                (initialize_materials, initialize_outline_marker).chain(),
            )
            .add_systems(
                Update,
                resize_outline_marker.run_if(resource_changed::<BoardVisualInformation>),
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

#[derive(Resource)]
struct InteractionMaterials {
    action_circle: Handle<Mesh>,
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
        action_circle: meshes.add(Circle::new(1.0)),
        action_color: materials.add(Color::srgba(0.5, 0.5, 0.5, 0.6)),

        outline_square: meshes.add(Rectangle::new(1.0, 1.0).to_ring(0.04)),
        outline_color: materials.add(Color::WHITE),
    })
}

#[derive(Component)]
pub struct CellOutlineMarker;

#[derive(Resource, Default)]
pub struct CellHover {
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

    outline_marker.translation = board_visual_information
        .cell_position(cell)
        .extend(outline_marker.translation.z);
    outline_marker.scale = Vec3::splat(board_visual_information.cell_size());
}

#[derive(SystemParam)]
pub struct Input<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub window: Single<'w, 's, &'static Window>,
    pub camera: Single<'w, 's, (&'static Camera, &'static GlobalTransform)>,
    pub pointer_interaction: ResMut<'w, PointerInteraction>,
}

#[derive(SystemParam)]
pub struct PieceMessageBroker<'w> {
    pub piece_moved_writer: MessageWriter<'w, PieceMoved>,
    pub piece_removed_writer: MessageWriter<'w, PieceRemoved>,
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

/// Sets the dragging status for a piece to follow the mouse.
#[derive(Component)]
pub struct DraggingPiece;

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

    outline_transform.translation = board_and_pieces
        .board
        .cell_position(&cell)
        .extend(outline_transform.translation.z);
    outline_transform.scale = Vec3::splat(board_and_pieces.board.cell_size());
    *outline_visibility = Visibility::Visible;
    cell_hover.cell = Some(cell);
}

fn handle_mouse_interaction(
    commands: Commands,
    input: Input,
    message_broker: PieceMessageBroker,

    interaction_materials: Res<InteractionMaterials>,

    board_and_pieces: BoardAndPieces,
    chess_game_engine: ResMut<ChessGameEngine>,

    dragging_piece_query: Query<(&BoardCell, &mut Transform), With<DraggingPiece>>,
) {
    if input.buttons.just_pressed(MouseButton::Left) {
        on_piece_drag(
            commands,
            input,
            message_broker,
            interaction_materials,
            board_and_pieces,
            chess_game_engine,
        );
    } else if input.buttons.just_released(MouseButton::Left) {
        on_piece_release(
            commands,
            input,
            message_broker,
            board_and_pieces,
            chess_game_engine,
            dragging_piece_query,
        );
    }
}

fn on_piece_drag(
    mut commands: Commands,
    mut input: Input,
    mut message_broker: PieceMessageBroker,

    interaction_materials: Res<InteractionMaterials>,

    mut board_and_pieces: BoardAndPieces,
    mut chess_game_engine: ResMut<ChessGameEngine>,
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
            // TODO: better to use filter and figure out whether this is a single move or multiple moves
            if let Some(candidate) = candidates
                .iter()
                .find(|candidate| candidate.target_square == selected_cell)
            {
                let move_info = chess_game_engine
                    .make_move(origin_cell, candidate)
                    .expect("The move is valid");

                board_and_pieces.visual_board.process_moves(
                    move_info,
                    &board_and_pieces.board,
                    origin_cell,
                    board_and_pieces.board.cell_position(origin_cell),
                    &mut message_broker,
                    &mut commands,
                );
            }

            input.pointer_interaction.reset_to_idle(&mut commands);
            return;
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
            let candidates = chess_game_engine.generate_legal_moves(
                selected_cell.clone(),
                &board_and_pieces.piece_storage,
                &board_and_pieces.chess_piece_assets,
            );

            let handles = spawn_action_circles(
                &mut commands,
                &board_and_pieces,
                &interaction_materials,
                &candidates,
            );

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
    mut message_broker: PieceMessageBroker,

    mut board_and_pieces: BoardAndPieces,
    mut chess_game_engine: ResMut<ChessGameEngine>,

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
        return_dragging_piece(
            &mut commands,
            *selected_piece,
            origin_cell,
            &mut transform,
            &board_and_pieces.board,
        );
        input.pointer_interaction.reset_to_idle(&mut commands);
        return;
    };

    // TODO: better to use filter and figure out whether this is a single move or multiple moves
    match candidates
        .iter()
        .find(|candidate| candidate.target_square == selected_cell)
    {
        Some(candidate) => {
            let move_info = chess_game_engine
                .make_move(origin_cell, candidate)
                .expect("The move is legal");

            commands.entity(*selected_piece).remove::<DraggingPiece>();
            transform.translation.z = PIECE_Z;

            board_and_pieces.visual_board.process_moves(
                move_info,
                &board_and_pieces.board,
                origin_cell,
                transform.translation.xy(),
                &mut message_broker,
                &mut commands,
            );

            input.pointer_interaction.reset_to_idle(&mut commands);
        }
        None => {
            return_dragging_piece(
                &mut commands,
                *selected_piece,
                origin_cell,
                &mut transform,
                &board_and_pieces.board,
            );

            if *has_released && *origin_cell == selected_cell {
                input.pointer_interaction.reset_to_idle(&mut commands);
            } else {
                input.pointer_interaction.release_piece();
            }
        }
    }
}

pub fn move_dragging_piece(
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
            return_dragging_piece(
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

/// Returns the dragging piece to its intial position and removes the dragging status (component).
///
/// In fact, it has a small intersection with [`PointerInteraction`], but they solves different
/// problems. Therefore the [`PointerInteraction`] cannot manage entity's information.
pub fn return_dragging_piece(
    commands: &mut Commands,
    piece: Entity,
    board_cell: &BoardCell,
    transform: &mut Transform,
    board_visual_information: &BoardVisualInformation,
) {
    commands.entity(piece).remove::<DraggingPiece>();

    transform.translation = board_visual_information
        .cell_position(board_cell)
        .extend(PIECE_Z);
}

fn spawn_action_circles(
    commands: &mut Commands,
    board_and_pieces: &BoardAndPieces,
    interaction_materials: &InteractionMaterials,
    candidates: &[CandidateMove],
) -> Vec<Entity> {
    candidates
        .iter()
        .map(|candidate_move| {
            commands
                .spawn((
                    Mesh2d(interaction_materials.action_circle.clone()),
                    MeshMaterial2d(interaction_materials.action_color.clone()),
                    Transform::from_translation(
                        board_and_pieces
                            .board
                            .cell_position(&candidate_move.target_square)
                            .extend(BOARD_MARKS_Z),
                    )
                    .with_scale(Vec3::splat(board_and_pieces.board.cell_size() * 0.15)),
                ))
                .id()
        })
        .collect()
}
