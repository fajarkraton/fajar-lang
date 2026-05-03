//! lsp_v3 semantic tokens coverage — P5.D2 of FAJAR_LANG_PERFECTION_PLAN.
//!
//! Plan §4 P5 D2 PASS criterion: lsp_v3 semantic tokens covered via at
//! least 1 E2E test per token kind.
//!
//! `encode_semantic_tokens` converts absolute-position `AbsoluteToken`s
//! into delta-encoded `SemanticToken`s for the LSP protocol. This file
//! exercises ALL 24 `SemanticTokenType` variants and ALL 8
//! `SemanticTokenModifier` variants through the encoding pipeline,
//! plus delta-encoding edge cases.

use fajar_lang::lsp_v3::semantic::{
    AbsoluteToken, SemanticTokenModifier, SemanticTokenType, encode_semantic_tokens,
};

fn make_token(
    line: u32,
    start: u32,
    length: u32,
    kind: SemanticTokenType,
    mods: u32,
) -> AbsoluteToken {
    AbsoluteToken {
        line,
        start,
        length,
        token_type: kind.index(),
        modifiers: mods,
    }
}

// ════════════════════════════════════════════════════════════════════════
// Token-type coverage — 24 variants, 1 test each
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d2_token_type_namespace() {
    let toks = vec![make_token(0, 0, 4, SemanticTokenType::Namespace, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Namespace.index());
}

#[test]
fn d2_token_type_type() {
    let toks = vec![make_token(0, 0, 3, SemanticTokenType::Type, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Type.index());
}

#[test]
fn d2_token_type_class() {
    let toks = vec![make_token(0, 0, 5, SemanticTokenType::Class, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Class.index());
}

#[test]
fn d2_token_type_enum() {
    let toks = vec![make_token(0, 0, 4, SemanticTokenType::Enum, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Enum.index());
}

#[test]
fn d2_token_type_interface() {
    let toks = vec![make_token(0, 0, 9, SemanticTokenType::Interface, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Interface.index());
}

#[test]
fn d2_token_type_struct() {
    let toks = vec![make_token(0, 0, 6, SemanticTokenType::Struct, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Struct.index());
}

#[test]
fn d2_token_type_type_parameter() {
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::TypeParameter, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(
        encoded[0].token_type,
        SemanticTokenType::TypeParameter.index()
    );
}

#[test]
fn d2_token_type_parameter() {
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Parameter, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Parameter.index());
}

#[test]
fn d2_token_type_variable() {
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Variable, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Variable.index());
}

#[test]
fn d2_token_type_property() {
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Property, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Property.index());
}

#[test]
fn d2_token_type_enum_member() {
    let toks = vec![make_token(0, 0, 4, SemanticTokenType::EnumMember, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::EnumMember.index());
}

#[test]
fn d2_token_type_event() {
    let toks = vec![make_token(0, 0, 4, SemanticTokenType::Event, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Event.index());
}

#[test]
fn d2_token_type_function() {
    let toks = vec![make_token(0, 0, 3, SemanticTokenType::Function, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Function.index());
}

#[test]
fn d2_token_type_method() {
    let toks = vec![make_token(0, 0, 6, SemanticTokenType::Method, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Method.index());
}

#[test]
fn d2_token_type_macro() {
    let toks = vec![make_token(0, 0, 5, SemanticTokenType::Macro, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Macro.index());
}

#[test]
fn d2_token_type_keyword() {
    let toks = vec![make_token(0, 0, 2, SemanticTokenType::Keyword, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Keyword.index());
}

#[test]
fn d2_token_type_modifier() {
    let toks = vec![make_token(0, 0, 3, SemanticTokenType::Modifier, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Modifier.index());
}

#[test]
fn d2_token_type_comment() {
    let toks = vec![make_token(0, 0, 7, SemanticTokenType::Comment, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Comment.index());
}

#[test]
fn d2_token_type_string() {
    let toks = vec![make_token(0, 0, 7, SemanticTokenType::String, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::String.index());
}

#[test]
fn d2_token_type_number() {
    let toks = vec![make_token(0, 0, 2, SemanticTokenType::Number, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Number.index());
}

#[test]
fn d2_token_type_regexp() {
    let toks = vec![make_token(0, 0, 5, SemanticTokenType::Regexp, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Regexp.index());
}

#[test]
fn d2_token_type_operator() {
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Operator, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Operator.index());
}

#[test]
fn d2_token_type_decorator() {
    let toks = vec![make_token(0, 0, 7, SemanticTokenType::Decorator, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Decorator.index());
}

#[test]
fn d2_token_type_label() {
    let toks = vec![make_token(0, 0, 4, SemanticTokenType::Label, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Label.index());
}

// One per legend entry — total 24 token-type tests.
// Verify the legend has exactly 24 entries to catch any future drift.
#[test]
fn d2_legend_has_24_token_types() {
    assert_eq!(SemanticTokenType::legend().len(), 24);
}

// ════════════════════════════════════════════════════════════════════════
// Modifier coverage — 8 variants, 1 test each
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d2_modifier_declaration() {
    let m = SemanticTokenModifier::Declaration.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Variable, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_definition() {
    let m = SemanticTokenModifier::Definition.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Function, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_readonly() {
    let m = SemanticTokenModifier::Readonly.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Variable, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_static() {
    let m = SemanticTokenModifier::Static.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Function, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_deprecated() {
    let m = SemanticTokenModifier::Deprecated.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Function, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_abstract() {
    let m = SemanticTokenModifier::Abstract.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Class, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_async() {
    let m = SemanticTokenModifier::Async.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Function, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_modification() {
    let m = SemanticTokenModifier::Modification.bitmask();
    let toks = vec![make_token(0, 0, 1, SemanticTokenType::Variable, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_ne!(encoded[0].token_modifiers & m, 0);
}

#[test]
fn d2_modifier_legend_has_8_entries() {
    assert_eq!(SemanticTokenModifier::legend().len(), 8);
}

#[test]
fn d2_modifier_bitmasks_distinct() {
    // Each modifier's bitmask must be a unique single bit so multiple
    // modifiers can be OR'd together without collision.
    let masks = [
        SemanticTokenModifier::Declaration.bitmask(),
        SemanticTokenModifier::Definition.bitmask(),
        SemanticTokenModifier::Readonly.bitmask(),
        SemanticTokenModifier::Static.bitmask(),
        SemanticTokenModifier::Deprecated.bitmask(),
        SemanticTokenModifier::Abstract.bitmask(),
        SemanticTokenModifier::Async.bitmask(),
        SemanticTokenModifier::Modification.bitmask(),
    ];
    for (i, m_i) in masks.iter().enumerate() {
        assert!(m_i.is_power_of_two(), "modifier {i} bitmask must be 1 << k");
        for (j, m_j) in masks.iter().enumerate() {
            if i != j {
                assert_eq!(
                    m_i & m_j,
                    0,
                    "modifier {i} and {j} bitmasks collide: {m_i:b} vs {m_j:b}"
                );
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
// encode_semantic_tokens — delta-encoding correctness
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d2_encode_empty_input_returns_empty() {
    assert!(encode_semantic_tokens(&[]).is_empty());
}

#[test]
fn d2_encode_single_token_uses_absolute_positions() {
    let toks = vec![make_token(5, 10, 4, SemanticTokenType::Keyword, 0)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].delta_line, 5);
    assert_eq!(encoded[0].delta_start, 10);
    assert_eq!(encoded[0].length, 4);
}

#[test]
fn d2_encode_same_line_uses_delta_start() {
    let toks = vec![
        make_token(0, 0, 3, SemanticTokenType::Keyword, 0),
        make_token(0, 5, 1, SemanticTokenType::Variable, 0),
    ];
    let encoded = encode_semantic_tokens(&toks);
    // Second token: delta_line=0 (same line), delta_start=5 (5-0).
    assert_eq!(encoded[1].delta_line, 0);
    assert_eq!(encoded[1].delta_start, 5);
}

#[test]
fn d2_encode_new_line_resets_start_to_absolute() {
    let toks = vec![
        make_token(0, 10, 3, SemanticTokenType::Keyword, 0),
        make_token(2, 4, 1, SemanticTokenType::Variable, 0),
    ];
    let encoded = encode_semantic_tokens(&toks);
    // Second token: delta_line=2 (line 2-0), delta_start=4 (absolute,
    // not 4-10, because line changed).
    assert_eq!(encoded[1].delta_line, 2);
    assert_eq!(encoded[1].delta_start, 4);
}

#[test]
fn d2_encode_preserves_token_type_and_modifiers() {
    let m =
        SemanticTokenModifier::Declaration.bitmask() | SemanticTokenModifier::Readonly.bitmask();
    let toks = vec![make_token(3, 7, 2, SemanticTokenType::Variable, m)];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded[0].token_type, SemanticTokenType::Variable.index());
    assert_eq!(encoded[0].token_modifiers, m);
    assert_eq!(encoded[0].length, 2);
}

#[test]
fn d2_encode_multiple_tokens_chained_correctly() {
    // Build a 5-token sequence and verify the cumulative deltas reconstruct
    // the original absolute positions.
    let toks = vec![
        make_token(0, 0, 3, SemanticTokenType::Keyword, 0),
        make_token(0, 4, 1, SemanticTokenType::Variable, 0),
        make_token(1, 0, 5, SemanticTokenType::Function, 0),
        make_token(1, 6, 2, SemanticTokenType::Number, 0),
        make_token(3, 0, 7, SemanticTokenType::Comment, 0),
    ];
    let encoded = encode_semantic_tokens(&toks);
    assert_eq!(encoded.len(), 5);

    // Reconstruct absolute positions.
    let mut line = 0u32;
    let mut start = 0u32;
    for (i, tok) in encoded.iter().enumerate() {
        line += tok.delta_line;
        if tok.delta_line == 0 {
            start += tok.delta_start;
        } else {
            start = tok.delta_start;
        }
        assert_eq!(line, toks[i].line, "line mismatch at index {i}");
        assert_eq!(start, toks[i].start, "start mismatch at index {i}");
    }
}
