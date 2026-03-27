use tree_sitter::Node;

use crate::model::{DefinitionKind, TextSpan};
use crate::source::SourceText;

use super::{
    DefinitionCapture, ExtractOptions, child_field_text, explicit_span, node_span,
    trimmed_node_text,
};

/// Capture one Python definition if the node contributes structural surface area.
pub fn capture_definition<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    match node.kind() {
        "function_definition" | "class_definition" => {
            let body = node.child_by_field_name("body");
            let header_end = body.map_or(node.end_byte(), |body| body.start_byte());

            Some(DefinitionCapture {
                kind: definition_kind(node),
                name: definition_name(node, source)?,
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
                kind: definition_kind(definition),
                name: definition_name(definition, source)?,
                span: explicit_span(node.start_byte(), definition.end_byte()),
                header_span: explicit_span(node.start_byte(), header_end),
                body,
            })
        }
        "assignment" | "augmented_assignment" | "type_alias_statement" => {
            if is_module_level_assignment(node) {
                Some(DefinitionCapture {
                    kind: assignment_kind(node),
                    name: assignment_name(node, source)?,
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

/// Capture one retained Python snippet such as a comment, docstring, or opt-in return.
pub fn capture_snippet(node: Node<'_>, options: ExtractOptions) -> Option<TextSpan> {
    match node.kind() {
        "comment" => Some(node_span(node)),
        "return_statement" if options.show_returns => Some(node_span(node)),
        "expression_statement" if is_docstring_statement(node) => Some(node_span(node)),
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

/// Decide which structural label to attach to a retained Python definition.
fn definition_kind(node: Node<'_>) -> DefinitionKind {
    match node.kind() {
        "class_definition" => DefinitionKind::Class,
        "function_definition" if is_method_definition(node) => DefinitionKind::Method,
        "function_definition" => DefinitionKind::Function,
        _ => DefinitionKind::Function,
    }
}

/// Extract the source-derived display name for a retained Python definition.
fn definition_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    child_field_text(node, "name", source)
}

/// Decide which structural label to attach to a retained Python assignment-like symbol.
fn assignment_kind(node: Node<'_>) -> DefinitionKind {
    match node.kind() {
        "type_alias_statement" => DefinitionKind::TypeAlias,
        _ => DefinitionKind::Assignment,
    }
}

/// Extract the source-derived display name for a retained Python assignment-like symbol.
fn assignment_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    for field_name in ["left", "name", "target"] {
        if let Some(name) = child_field_text(node, field_name, source) {
            return Some(name);
        }
    }

    node.named_child(0)
        .map(|child| trimmed_node_text(source, child))
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

/// Decide whether a Python function lives directly inside a class body and should read as a method.
fn is_method_definition(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    let body = if parent.kind() == "decorated_definition" {
        let Some(block) = parent.parent() else {
            return false;
        };

        block
    } else {
        parent
    };

    if body.kind() != "block" {
        return false;
    }

    body.parent().is_some_and(|grandparent| {
        grandparent.kind() == "class_definition"
            || (grandparent.kind() == "decorated_definition"
                && grandparent
                    .child_by_field_name("definition")
                    .is_some_and(|definition| definition.kind() == "class_definition"))
    })
}
