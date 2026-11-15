use bevy::{platform::collections::HashMap, prelude::*};

use crate::chess_core::{board::BoardCell, ChessBoard};

pub struct ChessPiecesPlugin;

impl Plugin for ChessPiecesPlugin {
    fn build(&self, app: &mut App) {
        todo!()
    }
}

#[derive(Component)]
pub struct ChessPiece {
    name: String,
    color: ChessPieceColor,
}

#[derive(Default)]
enum ChessPieceColor {
    Dark,
    #[default]
    Light,
}

/// https://en.wikipedia.org/wiki/Forsyth%E2%80%93Edwards_Notation
///
/// The extented variant of FEN parser was made only for this application.
/// The idea of this FEN is allowing to use custom pieces, even if they
/// contains 2 or more letters, surrounded by parentheses.
///
/// For instance, the Knook might be represented as NK, but it's the
/// combination of the knight and the king. So, to disambiguate need to
/// use parenteses: (NK). In this case it will be recognized as single piece.
///
/// Also if there are custom rules of game, they can be expected at specific
/// after end of the standard FEN.
pub struct ExtendedFenParser;

impl ExtendedFenParser {
    fn parse(fen: String) {
        let mut game_information = GameInformation::default();

        for line in fen.split('\n').map(|line| line.trim()) {
            if line.starts_with("header:") {
                let header = line.trim_start_matches("header:").trim();
                let Some((name, value)) = header.split_once('=') else {
                    warn!("Invalid header: {header}. Skipped.");
                    continue;
                };

                match name {
                    "orientation" => match value {
                        "b" => {
                            game_information.orientation = ChessPieceColor::Dark;
                        }
                        "w" => {
                            game_information.orientation = ChessPieceColor::Light;
                        }
                        _ => {
                            warn!("Unknown oreientation: {value}");
                        }
                    },
                    _ => {
                        warn!("Unknown header name: {name}. Skipped.");
                    }
                }

                continue;
            }

            let Some((pieces_positions, remaining_information)) = line.split_once(' ') else {
                warn!("Expected correct information of FEN body: missing spaces.");
                continue;
            };

            for (row, pieces_in_row) in pieces_positions.split('/').enumerate() {
                let mut col = 0;

                let mut color;
                let mut is_complete = true;
                let mut piece_name = String::new();
                for ch in pieces_in_row.chars() {
                    if ch == '(' {
                        is_complete = false;
                        continue;
                    }

                    if ch == ')' {
                        is_complete = true;
                    }

                    if ch.is_lowercase() {
                        color = ChessPieceColor::Dark;
                    } else {
                        color = ChessPieceColor::Light;
                    }

                    if is_complete {
                        if ('1'..='8').contains(&ch) {
                            let skips = ch.to_digit(10).unwrap_or_default();
                            col += skips;
                        }

                        if piece_name.is_empty() {
                            warn!("The piece is unknown: {piece_name}! Ignored.");
                        } else {
                            game_information.pieces.insert(
                                (row as u32, col).into(),
                                ChessPiece {
                                    name: piece_name.to_uppercase(),
                                    color,
                                },
                            );
                            piece_name.clear();
                        }

                        col += 1;
                    }
                }
            }

            break;
        }
    }
}

#[derive(Default)]
struct GameInformation {
    orientation: ChessPieceColor,
    pieces: HashMap<BoardCell, ChessPiece>,
    active_color: ChessPieceColor,
    available_castles: AvailableCastles,
    en_passant_target_destination: Option<BoardCell>,
    halfmove_clock: usize,
    full_moves: usize,
}

#[derive(Default)]
struct AvailableCastles {
    dark: Castles,
    light: Castles,
}

struct Castles {
    king_side: bool,
    queen_side: bool,
}

impl Default for Castles {
    fn default() -> Self {
        Self {
            king_side: true,
            queen_side: true,
        }
    }
}

fn clear_board(commands: &mut Commands, chess_board: Res<ChessBoard>) {
    for entity in chess_board.cells.values() {
        commands.entity(*entity).remove::<ChessPiece>();
    }
}
