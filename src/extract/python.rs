use tree_sitter::Node;

use crate::model::TextSpan;
use crate::source::SourceText;

use super::{DefinitionCapture, explicit_span, node_span};

/// Capture one Python definition if the node contributes structural surface area.
pub fn capture_definition<'tree>(
    node: Node<'tree>,
    _source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    match node.kind() {
        "function_definition" | "class_definition" => {
            let body = node.child_by_field_name("body");
            let header_end = body.map_or(node.end_byte(), |body| body.start_byte());

            Some(DefinitionCapture {
                span: node_span(node),
                header_span: explicit_span(node.start_byte(), header_end),
                body,
            })
        }
        "decorated_definition" => {
            let definition = node.child_by_field_name("definition")?;
            let body = definition.child_by_field_name("body");
            let header_end = body.map_or(node.end_byte(), |body| body.start_byte());

            Some(DefinitionCapture {
                span: explicit_span(node.start_byte(), definition.end_byte()),
                header_span: explicit_span(node.start_byte(), header_end),
                body,
            })
        }
        "assignment" | "augmented_assignment" | "type_alias_statement" => {
            if is_module_level_assignment(node) {
                Some(DefinitionCapture {
                    span: node_span(node),
                    header_span: node_span(node),
                    body: None,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Capture one retained Python snippet such as a comment, docstring, or exit surface.
pub fn capture_snippet(node: Node<'_>) -> Option<TextSpan> {
    match node.kind() {
        "comment" | "return_statement" | "raise_statement" => Some(node_span(node)),
        "expression_statement"
            if is_docstring_statement(node) || statement_contains_yield(node) =>
        {
            Some(node_span(node))
        }
        _ => None,
    }
}

/// Report whether the node is a Python comment token.
pub fn is_comment_node(node: Node<'_>) -> bool {
    node.kind() == "comment"
}

/// Skip helper nodes that are rendered through a surrounding definition.
pub fn should_skip_node(node: Node<'_>) -> bool {
    matches!(node.kind(), "decorator")
}

/// Decide whether an expression statement is the canonical docstring for its scope.
fn is_docstring_statement(node: Node<'_>) -> bool {
    if node.kind() != "expression_statement" {
        return false;
    }

    let Some(first_child) = node.named_child(0) else {
        return false;
    };

    if !matches!(first_child.kind(), "string" | "concatenated_string") {
        return false;
    }

    let Some(parent) = node.parent() else {
        return false;
    };

    let mut cursor = parent.walk();

    // Only treat the first substantive statement in the scope as its docstring.
    for sibling in parent.children(&mut cursor) {
        if sibling.id() == node.id() {
            return true;
        }

        if sibling.kind() == "comment" {
            continue;
        }

        if sibling.is_named() {
            return false;
        }
    }

    false
}

/// Decide whether a statement contains a `yield` or `yield from` exit surface.
fn statement_contains_yield(node: Node<'_>) -> bool {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "yield" | "yield_from") {
            return true;
        }

        if child.is_named() && statement_contains_yield(child) {
            return true;
        }
    }

    false
}

/// Decide whether an assignment sits at module scope and should be surfaced as structure.
fn is_module_level_assignment(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if parent.kind() == "module" {
        return true;
    }

    parent.parent().is_some_and(|grandparent| {
        parent.kind() == "expression_statement" && grandparent.kind() == "module"
    })
}
