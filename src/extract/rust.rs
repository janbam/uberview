use tree_sitter::Node;

use crate::model::TextSpan;
use crate::source::SourceText;

use super::{
    DefinitionCapture, body_header_end, expand_start_with_prefix_siblings, explicit_span, node_span,
};

/// Capture one Rust definition if the node contributes structural surface area.
pub fn capture_definition<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    match node.kind() {
        "function_item"
        | "function_signature_item"
        | "impl_item"
        | "trait_item"
        | "struct_item"
        | "enum_item"
        | "mod_item" => capture_container_or_leaf(node, source),
        "const_item" | "static_item" | "type_item" | "associated_type" | "macro_definition"
        | "field_declaration" | "enum_variant" => Some(DefinitionCapture {
            span: prefixed_span(node),
            header_span: prefixed_span(node),
            body: None,
        }),
        _ => None,
    }
}

/// Capture one retained Rust snippet such as comments, returns, `?`, or tail expressions.
pub fn capture_snippet(node: Node<'_>) -> Option<TextSpan> {
    match node.kind() {
        "line_comment" | "block_comment" | "comment" | "return_expression" => Some(node_span(node)),
        "expression_statement" | "let_declaration" if contains_try_expression(node) => {
            Some(node_span(node))
        }
        kind if is_tail_expression(kind, node) => Some(node_span(node)),
        _ => None,
    }
}

/// Report whether the node is a Rust comment token.
pub fn is_comment_node(node: Node<'_>) -> bool {
    matches!(node.kind(), "line_comment" | "block_comment" | "comment")
}

/// Skip helper nodes that are retained as part of their surrounding definition headers.
pub fn should_skip_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "attribute_item" | "inner_attribute_item" | "declaration_list"
    )
}

/// Capture either a body-owning Rust item or a leaf declaration rendered whole.
fn capture_container_or_leaf<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    let span = prefixed_span(node);

    if let Some(body) = node.child_by_field_name("body") {
        return Some(DefinitionCapture {
            span: explicit_span(span.start_byte, node.end_byte()),
            header_span: explicit_span(span.start_byte, body_header_end(source, body)),
            body: Some(body),
        });
    }

    Some(DefinitionCapture {
        span,
        header_span: span,
        body: None,
    })
}

/// Expand a Rust item's span upward to include immediately attached attributes.
fn prefixed_span(node: Node<'_>) -> TextSpan {
    let start = expand_start_with_prefix_siblings(node, |sibling| {
        matches!(sibling.kind(), "attribute_item" | "inner_attribute_item")
    });

    explicit_span(start, node.end_byte())
}

/// Decide whether a retained statement contains a `?` early-exit surface.
fn contains_try_expression(node: Node<'_>) -> bool {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "try_expression" {
            return true;
        }

        if child.is_named() && contains_try_expression(child) {
            return true;
        }
    }

    false
}

/// Decide whether a node is the final tail expression of its containing block.
fn is_tail_expression(kind: &str, node: Node<'_>) -> bool {
    if !matches!(
        kind,
        "identifier"
            | "call_expression"
            | "field_expression"
            | "binary_expression"
            | "if_expression"
            | "match_expression"
            | "macro_invocation"
            | "tuple_expression"
            | "array_expression"
            | "scoped_identifier"
            | "reference_expression"
            | "await_expression"
            | "try_expression"
            | "closure_expression"
            | "block"
    ) {
        return false;
    }

    let Some(parent) = node.parent() else {
        return false;
    };

    if parent.kind() != "block" {
        return false;
    }

    let mut cursor = parent.walk();
    let last_named = parent
        .children(&mut cursor)
        .filter(|child| child.is_named())
        .last();

    last_named.is_some_and(|last| last.id() == node.id())
}
