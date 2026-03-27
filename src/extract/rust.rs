use tree_sitter::Node;

use crate::model::{DefinitionKind, TextSpan};
use crate::source::SourceText;

use super::{
    DefinitionCapture, body_header_end, child_field_text, expand_start_with_prefix_siblings,
    explicit_span, node_span, trimmed_node_text,
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
            kind: definition_kind(node),
            name: definition_name(node, source)?,
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
    let kind = definition_kind(node);
    let name = definition_name(node, source)?;

    if let Some(body) = node.child_by_field_name("body") {
        return Some(DefinitionCapture {
            kind,
            name,
            span: explicit_span(span.start_byte, node.end_byte()),
            header_span: explicit_span(span.start_byte, body_header_end(source, body)),
            body: Some(body),
        });
    }

    Some(DefinitionCapture {
        kind,
        name,
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

/// Decide which synthetic label should describe a retained Rust definition.
fn definition_kind(node: Node<'_>) -> DefinitionKind {
    match node.kind() {
        "function_item" | "function_signature_item" if is_associated_function(node) => {
            DefinitionKind::Method
        }
        "function_item" | "function_signature_item" => DefinitionKind::Function,
        "impl_item" => DefinitionKind::Impl,
        "trait_item" => DefinitionKind::Trait,
        "struct_item" => DefinitionKind::Struct,
        "enum_item" => DefinitionKind::Enum,
        "mod_item" => DefinitionKind::Module,
        "const_item" | "static_item" => DefinitionKind::Constant,
        "type_item" | "associated_type" => DefinitionKind::TypeAlias,
        "macro_definition" => DefinitionKind::Macro,
        "field_declaration" => DefinitionKind::Field,
        "enum_variant" => DefinitionKind::Variant,
        _ => DefinitionKind::Function,
    }
}

/// Extract the source-derived display name for a retained Rust definition.
fn definition_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    match node.kind() {
        "impl_item" => impl_name(node, source),
        "macro_definition" => macro_name(node, source),
        _ => {
            for field_name in ["name", "type"] {
                if let Some(name) = child_field_text(node, field_name, source) {
                    return Some(name);
                }
            }

            node.named_child(0)
                .map(|child| trimmed_node_text(source, child))
        }
    }
}

/// Build a readable `impl` label from the trait and self type when available.
fn impl_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    let self_type = child_field_text(node, "type", source);
    let trait_name = child_field_text(node, "trait", source);

    match (trait_name, self_type) {
        (Some(trait_name), Some(self_type)) => Some(format!("{trait_name} for {self_type}")),
        (None, Some(self_type)) => Some(self_type),
        _ => fallback_impl_name(node, source),
    }
}

/// Strip the leading `macro_rules!` prefix down to the macro identifier.
fn macro_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    let header = trimmed_node_text(source, node);
    let header = header.lines().next()?.trim();
    let header = header.strip_prefix("macro_rules!")?.trim();
    let header = header.strip_suffix('{').unwrap_or(header).trim();

    if header.is_empty() {
        None
    } else {
        Some(header.to_owned())
    }
}

/// Fall back to the visible `impl ...` header when tree-sitter fields are absent.
fn fallback_impl_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    let header = trimmed_node_text(source, node);
    let header = header.lines().next()?.trim();
    let header = header.strip_suffix('{').unwrap_or(header).trim();
    let header = header.strip_prefix("impl")?.trim();

    if header.is_empty() {
        None
    } else {
        Some(header.to_owned())
    }
}

/// Decide whether a Rust function lives under an impl or trait item and reads as a method.
fn is_associated_function(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if parent.kind() != "declaration_list" {
        return false;
    }

    parent
        .parent()
        .is_some_and(|grandparent| matches!(grandparent.kind(), "impl_item" | "trait_item"))
}
