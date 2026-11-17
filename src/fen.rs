//! https://en.wikipedia.org/wiki/Forsyth%E2%80%93Edwards_Notation
//!
//! The extented variant of FEN parser was made only for this application.
//! The idea of this FEN is allowing to use custom pieces, even if they
//! contains 2 or more letters, surrounded by parentheses.
//!
//! For instance, the Knook might be represented as NK, but it's the
//! combination of the knight and the king. So, to disambiguate need to
//! use parenteses: (NK). In this case it will be recognized as single piece.
//!
//! Also if there are custom rules of game, they can be expected at specific
//! after end of the standard FEN.

use std::num::NonZeroUsize;

use crate::chess_core::{
    board::BoardCell, pieces::{ChessPiece, ChessPieceColor, ChessPieceName},
};
use bevy::{platform::collections::HashMap, prelude::*};

#[derive(derive_builder::Builder)]
pub struct GameInformation {
    #[builder(default)]
    pub headers: Headers,
    pub pieces: HashMap<BoardCell, ChessPiece>,
    pub active_color: ChessPieceColor,
    pub available_castles: AvailableCastles,
    pub en_passant_target_square: Option<BoardCell>,
    pub halfmove_clock: usize,
    pub fullmove_number: NonZeroUsize,
}

#[derive(Default, Clone)]
struct Headers {
    orientation: ChessPieceColor,
}

#[derive(Default, Clone)]
struct AvailableCastles {
    black: Castles,
    white: Castles,
}

#[derive(Default, Clone)]
struct Castles {
    king_side: bool,
    queen_side: bool,
}

#[derive(Debug)]
pub struct FenParseError {
    pub kind: FenParseErrorKind,
    pub message: String,
}

impl std::fmt::Display for FenParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FenParseError {}

#[derive(Debug)]
pub enum FenParseErrorKind {
    MissingBody,
    MalformedHeader {
        actual: String,
        expected_format: String,
    },
    UnknownHeader {
        header_name: String,
    },
    MissingField {
        name: String,
    },
    InvalidField {
        actual: String,
        expected_format: String,
    },
}

macro_rules! expect {
    ($item:ident <== $iter:ident, name: $name:expr, message: $message:expr) => {
        let Some($item) = $iter.next() else {
            return Err(FenParseError {
                kind: FenParseErrorKind::MissingField { name: $name },
                message: $message,
            });
        };
    };
}

pub fn parse(fen: &str) -> Result<GameInformation, FenParseError> {
    let mut game_information = GameInformationBuilder::create_empty();

    for line in fen.split('\n').map(|line| line.trim()) {
        if line.is_empty() {
            continue;
        }

        if line.starts_with("header:") {
            game_information.headers(parse_headers(line)?);
            continue;
        }

        let mut encoded_game_information = line.split_whitespace();

        expect! {
            board_information <== encoded_game_information,
            name: "The board description".to_string(),
            message: "The board description is absent from an input.".to_string()
        };
        game_information.pieces(parse_pieces(board_information)?);

        expect! {
            active_color <== encoded_game_information,
            name: "The active color.".to_string(),
            message: "The active color, denoting who's going to move, is absent from an input.".to_string()
        };
        game_information.active_color(parse_active_color(active_color)?);

        expect! {
            castling_availability <== encoded_game_information,
            name: "The castling availability.".to_string(),
            message: "The castling availability is absent from an input.".to_string()
        };
        game_information.available_castles(parse_available_castles(castling_availability)?);

        expect! {
            en_passant_target_square <== encoded_game_information,
            name: "The en passant target square.".to_string(),
            message: "The en passant target squase is absent from an input.".to_string()
        };
        game_information
            .en_passant_target_square(parse_en_passant_target_square(en_passant_target_square)?);

        expect! {
            halfmove_clock <== encoded_game_information,
            name: "The halfmove clock.".to_string(),
            message: "The halfmove clock, which is used for 50-move rule, is absent from an input.".to_string()
        };
        game_information.halfmove_clock(parse_halfmove_clock(halfmove_clock)?);

        expect! {
            fullmove_number <== encoded_game_information,
            name: "The fullmove number.".to_string(),
            message: "The fullmove number is absent from an input.".to_string()
        };
        game_information.fullmove_number(parse_fullmove_nubmer(fullmove_number)?);

        break;
    }

    game_information.build().map_err(|_| FenParseError {
        kind: FenParseErrorKind::MissingBody,
        message: "The input is malformed: the body is missing.".to_string(),
    })
}

fn parse_headers(headers: &str) -> Result<Headers, FenParseError> {
    debug_assert!(headers.starts_with("header:"));

    let encoded_headers = headers.trim_start_matches("header:").trim().split(',');
    let mut headers = Headers::default();

    for header in encoded_headers {
        let Some((name, value)) = header.split_once('=') else {
            return Err(FenParseError {
                kind: FenParseErrorKind::MalformedHeader {
                    actual: header.to_string(),
                    expected_format: "name = value".to_string(),
                },
                message: "A header is malformed, check the syntax.".to_string(),
            });
        };

        match name.trim() {
            "orientation" => match value.trim() {
                "b" => {
                    headers.orientation = ChessPieceColor::Black;
                }
                "w" => {
                    headers.orientation = ChessPieceColor::White;
                }
                _ => {
                    return Err(FenParseError {
                        kind: FenParseErrorKind::InvalidField {
                            actual: value.trim().to_owned(),
                            expected_format: "Either w or black".to_owned(),
                        },
                        message: "Received invalid value of orientation".to_string(),
                    })
                }
            },
            _ => {
                return Err(FenParseError {
                    kind: FenParseErrorKind::UnknownHeader {
                        header_name: name.trim().to_string(),
                    },
                    message: "A header is unknown, possibly it's malformed.".to_string(),
                })
            }
        }
    }

    Ok(headers)
}

fn parse_pieces(board_information: &str) -> Result<HashMap<BoardCell, ChessPiece>, FenParseError> {
    const CELLS_IN_ROW: u32 = 8;

    fn invalid_board_dimension_err(board_information: &str) -> FenParseError {
        FenParseError {
            kind: FenParseErrorKind::InvalidField {
                actual: board_information.to_string(),
                expected_format: "The correct FEN board".to_string(),
            },
            message: "The dimension of a given board is incorrect.".to_string(),
        }
    }

    let mut pieces = HashMap::new();

    let mut row = 0;
    for pieces_in_row in board_information.split('/') {
        if row >= CELLS_IN_ROW {
            return Err(invalid_board_dimension_err(board_information));
        }

        let mut col = 0;

        let mut color;
        let mut inside_parentheses = false;
        let mut piece_name = String::new();

        for ch in pieces_in_row.chars() {
            if col >= CELLS_IN_ROW {
                return Err(invalid_board_dimension_err(board_information));
            }

            if ch == '(' {
                if inside_parentheses {
                    return Err(FenParseError {
                        kind: FenParseErrorKind::InvalidField {
                            actual: board_information.to_string(),
                            expected_format: "A correct FEN board.".to_string(),
                        },
                        message: "Nested parentheses are forbidden.".to_string(),
                    });
                }

                inside_parentheses = true;
                continue;
            }

            if ch == ')' {
                if !inside_parentheses {
                    return Err(FenParseError {
                        kind: FenParseErrorKind::InvalidField {
                            actual: board_information.to_string(),
                            expected_format: "A correct FEN board.".to_string(),
                        },
                        message: "A single closing parenthesis doesn't have a matching opening parenthesis.".to_string(),
                    });
                }

                inside_parentheses = false;
            } else {
                piece_name.push(ch);
            }

            if inside_parentheses {
                continue;
            }

            if piece_name.is_empty() {
                return Err(FenParseError {
                    kind: FenParseErrorKind::InvalidField {
                        actual: board_information.to_string(),
                        expected_format: "The correct FEN board.".to_string(),
                    },
                    message: "A piece name, surrounded by parentheses, is empty!".to_string(),
                });
            }

            if piece_name.len() == 1 && ('1'..='8').contains(&ch) {
                let skips = ch.to_digit(10).unwrap_or_default();
                col += skips;

                piece_name.clear();
                continue;
            }

            if piece_name.chars().all(char::is_lowercase) {
                color = ChessPieceColor::Black;
            } else if piece_name.chars().all(char::is_uppercase) {
                color = ChessPieceColor::White;
            } else {
                return Err(FenParseError {
                    kind: FenParseErrorKind::InvalidField {
                        actual: piece_name.trim().to_owned(),
                        expected_format: "All symbols in upper or lower case.".to_owned(),
                    },
                    message: "Received mixed case (uppercase and lowercase within the name of a piece), which is not valid.".to_string(),
                });
            }

            pieces.insert(
                ((b'8' - row as u8) as char, (b'a' + col as u8) as char).into(),
                ChessPiece {
                    name: ChessPieceName(piece_name.to_uppercase()),
                    color,
                },
            );
            piece_name.clear();

            col += 1;
        }

        if inside_parentheses {
            return Err(FenParseError {
                kind: FenParseErrorKind::InvalidField {
                    actual: board_information.to_string(),
                    expected_format: "A correct FEN board.".to_string(),
                },
                message: "An opening parenthesis doesn't have a matching closing parenthesis."
                    .to_string(),
            });
        }

        if col != CELLS_IN_ROW {
            return Err(invalid_board_dimension_err(board_information));
        }

        row += 1;
    }

    if row != CELLS_IN_ROW {
        return Err(invalid_board_dimension_err(board_information));
    }

    Ok(pieces)
}

fn parse_active_color(active_color: &str) -> Result<ChessPieceColor, FenParseError> {
    match active_color {
        "w" => Ok(ChessPieceColor::White),
        "b" => Ok(ChessPieceColor::Black),
        _ => Err(FenParseError {
            kind: FenParseErrorKind::InvalidField {
                actual: active_color.to_string(),
                expected_format: "Either w or b.".to_string(),
            },
            message: "The active color, denoting who's going to move, is invalid.".to_string(),
        }),
    }
}

fn parse_available_castles(castling_availability: &str) -> Result<AvailableCastles, FenParseError> {
    let mut available_castles = AvailableCastles::default();

    if castling_availability != "-" {
        for piece_name in castling_availability.chars() {
            match piece_name {
                'K' => available_castles.white.king_side = true,
                'k' => available_castles.black.king_side = true,
                'Q' => available_castles.white.queen_side = true,
                'q' => available_castles.black.queen_side = true,
                _ => {
                    return Err(FenParseError {
                        kind: FenParseErrorKind::InvalidField {
                            actual: castling_availability.to_string(),
                            expected_format:
                                "It should contain \"K\", \"Q\", \"k\", \"q\" or only \"-\"."
                                    .to_string(),
                        },
                        message: "The castling availability contains invalid pieces.".to_string(),
                    });
                }
            }
        }
    }

    Ok(available_castles)
}

fn parse_en_passant_target_square(
    en_passant_target_square: &str,
) -> Result<Option<BoardCell>, FenParseError> {
    let mut target_square = None;

    if en_passant_target_square != "-" {
        if !en_passant_target_square.is_ascii() || en_passant_target_square.len() != 2 {
            return Err(FenParseError {
                    kind: FenParseErrorKind::InvalidField {
                        actual: en_passant_target_square.to_string(),
                        expected_format:
                            "It should contain two symbols, denoting a square in the chess board (i.e. \"e3\")"
                                .to_string(),
                    },
                    message: "The en passant target square value contains a malformed information."
                        .to_string(),
                });
        }

        let mut position = en_passant_target_square.chars();
        let col = position.next().unwrap();
        let row = position.next().unwrap();

        if !('a'..='h').contains(&col) || !('1'..='8').contains(&row) {
            return Err(FenParseError {
                    kind: FenParseErrorKind::InvalidField {
                        actual: en_passant_target_square.to_string(),
                        expected_format:
                            "It should contain two symbols, denoting a square in the chess board (i.e. \"e3\")"
                                .to_string(),
                    },
                    message: "The en passant target square value contains either an invalid row or an invalid column name."
                        .to_string(),
                });
        }

        target_square = Some((row, col).into());
    }

    Ok(target_square)
}

fn parse_halfmove_clock(halfmove_clock: &str) -> Result<usize, FenParseError> {
    halfmove_clock.parse::<usize>().map_err(|_| FenParseError {
        kind: FenParseErrorKind::InvalidField {
            actual: halfmove_clock.to_string(),
            expected_format: "A non-negative integer.".to_string(),
        },
        message: "The halfmove clock is malformed.".to_string(),
    })
}

fn parse_fullmove_nubmer(fullmove_number: &str) -> Result<NonZeroUsize, FenParseError> {
    fullmove_number
        .parse::<NonZeroUsize>()
        .map_err(|_| FenParseError {
            kind: FenParseErrorKind::InvalidField {
                actual: fullmove_number.to_string(),
                expected_format: "An integer starting from 1.".to_string(),
            },
            message: "The fullmove number is malformed.".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_first_rank(first_rank: &str) -> Result<HashMap<BoardCell, ChessPiece>, FenParseError> {
        let board = format!("{first_rank}/8/8/8/8/8/8/8");

        parse_pieces(&board)
    }

    fn piece_at(pieces: &HashMap<BoardCell, ChessPiece>, row: char, col: char) -> &ChessPiece {
        let cell: BoardCell = (row, col).into();

        pieces.get(&cell).expect("Expected a piece at this cell")
    }

    #[test]
    fn parse_empty_board() {
        let pieces = parse_first_rank("8").unwrap();

        assert!(pieces.is_empty());
    }

    #[test]
    fn parse_standard_pieces() {
        let pieces = parse_first_rank("Pp2N3").unwrap();

        assert_eq!(pieces.len(), 3);

        let white_pawn = piece_at(&pieces, '8', 'a');
        assert_eq!(white_pawn.name, ChessPieceName("P".to_owned()));
        assert!(matches!(&white_pawn.color, ChessPieceColor::White));

        let black_pawn = piece_at(&pieces, '8', 'b');
        assert_eq!(black_pawn.name, ChessPieceName("P".to_owned()));
        assert!(matches!(&black_pawn.color, ChessPieceColor::Black));

        let white_knight = piece_at(&pieces, '8', 'e');
        assert_eq!(white_knight.name, ChessPieceName("N".to_owned()));
        assert!(matches!(&white_knight.color, ChessPieceColor::White));
    }

    #[test]
    fn parse_custom_pieces() {
        let pieces = parse_first_rank("(NK)7").unwrap();

        assert_eq!(pieces.len(), 1);

        let knook = piece_at(&pieces, '8', 'a');

        assert_eq!(knook.name, ChessPieceName("NK".to_owned()));
        assert!(matches!(&knook.color, ChessPieceColor::White));
    }

    #[test]
    fn parse_custom_black_pieces() {
        let pieces = parse_first_rank("(nk)7").unwrap();

        assert_eq!(pieces.len(), 1);

        let knook = piece_at(&pieces, '8', 'a');

        assert_eq!(knook.name, ChessPieceName("NK".to_owned()));
        assert!(matches!(&knook.color, ChessPieceColor::Black));
    }

    #[test]
    fn reject_malformed_parentheses() {
        let invalid_ranks = [
            "(NK",    // Missing closing parenthesis.
            "NK)",    // Missing opening parenthesis.
            "((NK))", // Nested parentheses.
            "()",     // Empty piece name.
        ];

        for rank in invalid_ranks {
            assert!(
                parse_first_rank(rank).is_err(),
                "Expected rank {rank:?} to be rejected"
            );
        }
    }

    #[test]
    fn reject_mixed_case_piece_names() {
        let invalid_ranks = ["(Nk)", "(nK)"];

        for rank in invalid_ranks {
            assert!(
                parse_first_rank(rank).is_err(),
                "Expected rank {rank:?} to be rejected"
            );
        }
    }

    #[test]
    fn reject_invalid_rank_count() {
        let seven_ranks = "8/8/8/8/8/8/8";
        let nine_ranks = "8/8/8/8/8/8/8/8/8";

        assert!(parse_pieces(seven_ranks).is_err());
        assert!(parse_pieces(nine_ranks).is_err());
    }

    #[test]
    fn reject_invalid_column_count() {
        let seven_cells = "7/8/8/8/8/8/8/8";
        let nine_cells = "8P/8/8/8/8/8/8/8";

        assert!(parse_pieces(seven_cells).is_err());
        assert!(parse_pieces(nine_cells).is_err());
    }

    #[test]
    fn reject_empty_rank() {
        let board = "8//8/8/8/8/8/8";

        assert!(parse_pieces(board).is_err());
    }

    #[test]
    fn parse_pieces_at_correct_coordinates() {
        let board = "8/8/8/3P4/8/8/8/7k";
        let pieces = parse_pieces(board).unwrap();

        assert_eq!(pieces.len(), 2);

        let pawn = piece_at(&pieces, '5', 'd');
        assert_eq!(pawn.name, ChessPieceName("P".to_owned()));
        assert!(matches!(&pawn.color, ChessPieceColor::White));

        let king = piece_at(&pieces, '1', 'h');
        assert_eq!(king.name, ChessPieceName("K".to_owned()));
        assert!(matches!(&king.color, ChessPieceColor::Black));
    }
}
