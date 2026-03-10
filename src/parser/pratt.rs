//! Pratt expression parser binding power definitions.
//!
//! Maps token kinds to their binding power for the Pratt parser.
//! Higher binding power = tighter binding = higher precedence.
//!
//! # Precedence Table (19 levels, lowest to highest)
//!
//! | Level | Operators | Assoc | Left BP | Right BP |
//! |-------|-----------|-------|---------|----------|
//! | 1 | `= += -=` etc | Right | special | special |
//! | 2 | `\|>` | Left | 3, 4 |
//! | 3 | `\|\|` | Left | 5, 6 |
//! | 4 | `&&` | Left | 7, 8 |
//! | 5 | `\|` | Left | 9, 10 |
//! | 6 | `^` | Left | 11, 12 |
//! | 7 | `&` | Left | 13, 14 |
//! | 8 | `== !=` | Left | 15, 16 |
//! | 9 | `< > <= >=` | Left | 17, 18 |
//! | 10 | `.. ..=` | None | 19, 20 |
//! | 11 | `<< >>` | Left | 21, 22 |
//! | 12 | `+ -` | Left | 23, 24 |
//! | 13 | `* / % @` | Left | 25, 26 |
//! | 14 | `**` | Right | 28, 27 |
//! | 15 | `as` | Left | 29, 30 |
//! | 16 | Unary prefix | Right | _, 31 |
//! | 17 | `?` | Postfix | 33 |
//! | 18 | `. () []` | Postfix | 35 |

use crate::lexer::token::TokenKind;
use crate::parser::ast::{AssignOp, BinOp, UnaryOp};

/// Returns the infix binding power for a token, if it is an infix operator.
///
/// Returns `(left_bp, right_bp)`. For left-associative operators, `right_bp > left_bp`.
/// For right-associative operators, `left_bp > right_bp`.
///
/// Returns `None` if the token is not an infix operator.
pub fn infix_binding_power(kind: &TokenKind) -> Option<(u8, u8)> {
    let bp = match kind {
        // Level 2: Pipeline (Left)
        TokenKind::PipeGt => (3, 4),

        // Level 3: Logical OR (Left)
        TokenKind::PipePipe => (5, 6),

        // Level 4: Logical AND (Left)
        TokenKind::AmpAmp => (7, 8),

        // Level 5: Bitwise OR (Left)
        TokenKind::Pipe => (9, 10),

        // Level 6: Bitwise XOR (Left)
        TokenKind::Caret => (11, 12),

        // Level 7: Bitwise AND (Left)
        TokenKind::Amp => (13, 14),

        // Level 8: Equality (Left)
        TokenKind::EqEq | TokenKind::BangEq => (15, 16),

        // Level 9: Comparison (Left)
        TokenKind::Lt | TokenKind::Gt | TokenKind::LtEq | TokenKind::GtEq => (17, 18),

        // Level 10: Range (Non-associative — same BP both sides)
        TokenKind::DotDot | TokenKind::DotDotEq => (19, 20),

        // Level 11: Bit Shift (Left)
        TokenKind::LtLt | TokenKind::GtGt => (21, 22),

        // Level 12: Addition (Left)
        TokenKind::Plus | TokenKind::Minus => (23, 24),

        // Level 13: Multiplication (Left)
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent | TokenKind::At => (25, 26),

        // Level 14: Power (Right)
        TokenKind::StarStar => (28, 27),

        // Level 15: Type Cast (Left) — `as` handled separately
        TokenKind::As => (29, 30),

        _ => return None,
    };
    Some(bp)
}

/// Returns the prefix binding power for a unary operator token.
///
/// Returns `((), right_bp)` — prefix operators only have a right binding power.
/// Returns `None` if the token is not a prefix operator.
pub fn prefix_binding_power(kind: &TokenKind) -> Option<((), u8)> {
    match kind {
        // Level 16: Unary prefix
        TokenKind::Minus
        | TokenKind::Bang
        | TokenKind::Tilde
        | TokenKind::Amp
        | TokenKind::Star => Some(((), 31)),
        _ => None,
    }
}

/// Returns the postfix binding power for a postfix operator token.
///
/// Returns `(left_bp, ())` — postfix operators only have a left binding power.
/// Returns `None` if the token is not a postfix operator.
pub fn postfix_binding_power(kind: &TokenKind) -> Option<(u8, ())> {
    match kind {
        // Level 17: Try / Error propagation
        TokenKind::Question => Some((33, ())),

        // Level 18: Postfix (call, index, field)
        TokenKind::LParen | TokenKind::LBracket | TokenKind::Dot => Some((35, ())),

        _ => None,
    }
}

/// Converts a token kind to a [`BinOp`].
///
/// Returns `None` if the token is not a binary operator.
pub fn token_to_binop(kind: &TokenKind) -> Option<BinOp> {
    match kind {
        TokenKind::Plus => Some(BinOp::Add),
        TokenKind::Minus => Some(BinOp::Sub),
        TokenKind::Star => Some(BinOp::Mul),
        TokenKind::Slash => Some(BinOp::Div),
        TokenKind::Percent => Some(BinOp::Rem),
        TokenKind::StarStar => Some(BinOp::Pow),
        TokenKind::At => Some(BinOp::MatMul),
        TokenKind::EqEq => Some(BinOp::Eq),
        TokenKind::BangEq => Some(BinOp::Ne),
        TokenKind::Lt => Some(BinOp::Lt),
        TokenKind::Gt => Some(BinOp::Gt),
        TokenKind::LtEq => Some(BinOp::Le),
        TokenKind::GtEq => Some(BinOp::Ge),
        TokenKind::AmpAmp => Some(BinOp::And),
        TokenKind::PipePipe => Some(BinOp::Or),
        TokenKind::Amp => Some(BinOp::BitAnd),
        TokenKind::Pipe => Some(BinOp::BitOr),
        TokenKind::Caret => Some(BinOp::BitXor),
        TokenKind::LtLt => Some(BinOp::Shl),
        TokenKind::GtGt => Some(BinOp::Shr),
        _ => None,
    }
}

/// Converts a token kind to a [`UnaryOp`] for prefix position.
///
/// Returns `None` if the token is not a prefix unary operator.
pub fn token_to_unaryop(kind: &TokenKind) -> Option<UnaryOp> {
    match kind {
        TokenKind::Minus => Some(UnaryOp::Neg),
        TokenKind::Bang => Some(UnaryOp::Not),
        TokenKind::Tilde => Some(UnaryOp::BitNot),
        TokenKind::Amp => Some(UnaryOp::Ref),
        TokenKind::Star => Some(UnaryOp::Deref),
        _ => None,
    }
}

/// Converts a token kind to an [`AssignOp`].
///
/// Returns `None` if the token is not an assignment operator.
pub fn token_to_assignop(kind: &TokenKind) -> Option<AssignOp> {
    match kind {
        TokenKind::Eq => Some(AssignOp::Assign),
        TokenKind::PlusEq => Some(AssignOp::AddAssign),
        TokenKind::MinusEq => Some(AssignOp::SubAssign),
        TokenKind::StarEq => Some(AssignOp::MulAssign),
        TokenKind::SlashEq => Some(AssignOp::DivAssign),
        TokenKind::PercentEq => Some(AssignOp::RemAssign),
        TokenKind::AmpEq => Some(AssignOp::BitAndAssign),
        TokenKind::PipeEq => Some(AssignOp::BitOrAssign),
        TokenKind::CaretEq => Some(AssignOp::BitXorAssign),
        TokenKind::LtLtEq => Some(AssignOp::ShlAssign),
        TokenKind::GtGtEq => Some(AssignOp::ShrAssign),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addition_is_left_associative() {
        let (l, r) = infix_binding_power(&TokenKind::Plus).unwrap();
        assert!(r > l, "left-assoc means right_bp > left_bp");
    }

    #[test]
    fn power_is_right_associative() {
        let (l, r) = infix_binding_power(&TokenKind::StarStar).unwrap();
        assert!(l > r, "right-assoc means left_bp > right_bp");
    }

    #[test]
    fn mul_binds_tighter_than_add() {
        let (_, add_r) = infix_binding_power(&TokenKind::Plus).unwrap();
        let (mul_l, _) = infix_binding_power(&TokenKind::Star).unwrap();
        assert!(mul_l > add_r, "mul should bind tighter than add");
    }

    #[test]
    fn comparison_binds_looser_than_add() {
        let (_, cmp_r) = infix_binding_power(&TokenKind::Lt).unwrap();
        let (add_l, _) = infix_binding_power(&TokenKind::Plus).unwrap();
        assert!(add_l > cmp_r, "add should bind tighter than comparison");
    }

    #[test]
    fn logical_and_binds_tighter_than_or() {
        let (_, or_r) = infix_binding_power(&TokenKind::PipePipe).unwrap();
        let (and_l, _) = infix_binding_power(&TokenKind::AmpAmp).unwrap();
        assert!(and_l > or_r, "AND should bind tighter than OR");
    }

    #[test]
    fn pipeline_binds_loosest_among_infix() {
        let (pipe_l, _) = infix_binding_power(&TokenKind::PipeGt).unwrap();
        let (or_l, _) = infix_binding_power(&TokenKind::PipePipe).unwrap();
        assert!(or_l > pipe_l, "pipeline should bind looser than logical OR");
    }

    #[test]
    fn prefix_operators_have_binding_power() {
        assert!(prefix_binding_power(&TokenKind::Minus).is_some());
        assert!(prefix_binding_power(&TokenKind::Bang).is_some());
        assert!(prefix_binding_power(&TokenKind::Tilde).is_some());
        assert!(prefix_binding_power(&TokenKind::Amp).is_some());
        assert!(prefix_binding_power(&TokenKind::Star).is_some());
    }

    #[test]
    fn postfix_operators_have_binding_power() {
        assert!(postfix_binding_power(&TokenKind::Question).is_some());
        assert!(postfix_binding_power(&TokenKind::LParen).is_some());
        assert!(postfix_binding_power(&TokenKind::LBracket).is_some());
        assert!(postfix_binding_power(&TokenKind::Dot).is_some());
    }

    #[test]
    fn token_to_binop_mappings() {
        assert_eq!(token_to_binop(&TokenKind::Plus), Some(BinOp::Add));
        assert_eq!(token_to_binop(&TokenKind::Minus), Some(BinOp::Sub));
        assert_eq!(token_to_binop(&TokenKind::Star), Some(BinOp::Mul));
        assert_eq!(token_to_binop(&TokenKind::At), Some(BinOp::MatMul));
        assert_eq!(token_to_binop(&TokenKind::StarStar), Some(BinOp::Pow));
        assert_eq!(token_to_binop(&TokenKind::EqEq), Some(BinOp::Eq));
        assert_eq!(token_to_binop(&TokenKind::AmpAmp), Some(BinOp::And));
        assert_eq!(token_to_binop(&TokenKind::LtLt), Some(BinOp::Shl));
    }

    #[test]
    fn token_to_unaryop_mappings() {
        assert_eq!(token_to_unaryop(&TokenKind::Minus), Some(UnaryOp::Neg));
        assert_eq!(token_to_unaryop(&TokenKind::Bang), Some(UnaryOp::Not));
        assert_eq!(token_to_unaryop(&TokenKind::Tilde), Some(UnaryOp::BitNot));
        assert_eq!(token_to_unaryop(&TokenKind::Amp), Some(UnaryOp::Ref));
        assert_eq!(token_to_unaryop(&TokenKind::Star), Some(UnaryOp::Deref));
    }

    #[test]
    fn token_to_assignop_mappings() {
        assert_eq!(token_to_assignop(&TokenKind::Eq), Some(AssignOp::Assign));
        assert_eq!(
            token_to_assignop(&TokenKind::PlusEq),
            Some(AssignOp::AddAssign)
        );
        assert_eq!(
            token_to_assignop(&TokenKind::MinusEq),
            Some(AssignOp::SubAssign)
        );
        assert_eq!(
            token_to_assignop(&TokenKind::LtLtEq),
            Some(AssignOp::ShlAssign)
        );
        assert_eq!(
            token_to_assignop(&TokenKind::GtGtEq),
            Some(AssignOp::ShrAssign)
        );
    }

    #[test]
    fn non_operator_tokens_return_none() {
        assert_eq!(infix_binding_power(&TokenKind::Let), None);
        assert_eq!(prefix_binding_power(&TokenKind::Let), None);
        assert_eq!(postfix_binding_power(&TokenKind::Let), None);
        assert_eq!(token_to_binop(&TokenKind::Let), None);
        assert_eq!(token_to_unaryop(&TokenKind::Let), None);
        assert_eq!(token_to_assignop(&TokenKind::Let), None);
    }
}
