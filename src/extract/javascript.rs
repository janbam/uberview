use tree_sitter::Node;

use crate::language::LanguageKind;
use crate::model::TextSpan;
use crate::source::SourceText;

use super::{DefinitionCapture, body_header_end, explicit_span, node_span};

/// Capture one JavaScript or TypeScript definition if the node contributes structural surface area.
pub fn capture_definition<'tree>(
    language: LanguageKind,
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "generator_function"
        | "method_definition"
        | "class_declaration"
        | "class"
        | "abstract_class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "type_alias_declaration"
        | "module"
        | "internal_module"
        | "method_signature"
        | "abstract_method_signature"
        | "function_signature"
        | "property_signature"
        | "public_field_definition" => capture_container_or_leaf(node, source),
        "lexical_declaration" | "variable_declaration" => capture_variable_definition(node, source),
        "export_statement" => capture_export_definition(language, node, source),
        "pair" => capture_object_pair_definition(node, source),
        _ => None,
    }
}

/// Capture one retained JavaScript or TypeScript snippet.
pub fn capture_snippet(node: Node<'_>) -> Option<TextSpan> {
    match node.kind() {
        "comment" | "return_statement" | "throw_statement" => Some(node_span(node)),
        _ => None,
    }
}

/// Report whether the node is a JavaScript-family comment token.
pub fn is_comment_node(node: Node<'_>) -> bool {
    node.kind() == "comment"
}

/// Skip helper nodes that should be rendered through a surrounding definition.
pub fn should_skip_node(node: Node<'_>) -> bool {
    matches!(node.kind(), "variable_declarator" | "decorator")
        || node.parent().is_some_and(|parent| {
            matches!(
                parent.kind(),
                "lexical_declaration" | "variable_declaration" | "export_statement"
            )
        })
}

/// Capture either a container definition with a walkable body or a leaf definition rendered whole.
fn capture_container_or_leaf<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    if let Some(body) = node.child_by_field_name("body") {
        return Some(DefinitionCapture {
            span: node_span(node),
            header_span: explicit_span(node.start_byte(), body_header_end(source, body)),
            body: Some(body),
        });
    }

    Some(DefinitionCapture {
        span: node_span(node),
        header_span: node_span(node),
        body: None,
    })
}

/// Capture a definition-like variable declaration such as an arrow-function constant.
fn capture_variable_definition<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    let declarator = first_declarator(node)?;
    let value = declarator.child_by_field_name("value")?;

    if matches!(
        value.kind(),
        "arrow_function"
            | "function_expression"
            | "generator_function"
            | "class"
            | "class_declaration"
    ) && let Some(body) = value.child_by_field_name("body")
    {
        if value.kind() == "arrow_function" && body.kind() != "statement_block" {
            return Some(DefinitionCapture {
                span: node_span(node),
                header_span: node_span(node),
                body: None,
            });
        }

        return Some(DefinitionCapture {
            span: node_span(node),
            header_span: explicit_span(node.start_byte(), body_header_end(source, body)),
            body: Some(body),
        });
    }

    if matches!(
        value.kind(),
        "arrow_function"
            | "function_expression"
            | "generator_function"
            | "class"
            | "class_declaration"
    ) {
        return Some(DefinitionCapture {
            span: node_span(node),
            header_span: node_span(node),
            body: None,
        });
    }

    if is_top_level_const(node) {
        return Some(DefinitionCapture {
            span: node_span(node),
            header_span: node_span(node),
            body: None,
        });
    }

    None
}

/// Capture exported definitions while preserving the `export` prefix in the retained header.
fn capture_export_definition<'tree>(
    language: LanguageKind,
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    if let Some(declaration) = node.child_by_field_name("declaration") {
        return match declaration.kind() {
            "lexical_declaration" | "variable_declaration" => {
                capture_variable_definition(node, source).or_else(|| {
                    capture_variable_definition(declaration, source).map(|capture| {
                        DefinitionCapture {
                            span: node_span(node),
                            header_span: explicit_span(
                                node.start_byte(),
                                capture.header_span.end_byte,
                            ),
                            body: capture.body,
                        }
                    })
                })
            }
            _ => capture_container_or_leaf(declaration, source).map(|capture| DefinitionCapture {
                span: node_span(node),
                header_span: explicit_span(node.start_byte(), capture.header_span.end_byte),
                body: capture.body,
            }),
        };
    }

    let value = node.child_by_field_name("value")?;

    if language == LanguageKind::JavaScript && value.is_named() {
        return Some(DefinitionCapture {
            span: node_span(node),
            header_span: node_span(node),
            body: None,
        });
    }

    None
}

/// Capture object properties whose values are nested function-like definitions.
fn capture_object_pair_definition<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    let value = node.child_by_field_name("value")?;

    if matches!(
        value.kind(),
        "arrow_function" | "function_expression" | "generator_function"
    ) && let Some(body) = value.child_by_field_name("body")
    {
        return Some(DefinitionCapture {
            span: node_span(node),
            header_span: explicit_span(node.start_byte(), body_header_end(source, body)),
            body: Some(body),
        });
    }

    None
}

/// Return the first declarator in a variable-style definition node.
fn first_declarator(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();

    node.children(&mut cursor)
        .find(|child| child.kind() == "variable_declarator")
}

/// Decide whether a variable declaration should be surfaced as a top-level constant.
fn is_top_level_const(node: Node<'_>) -> bool {
    matches!(node.kind(), "lexical_declaration" | "variable_declaration")
        && node.parent().is_some_and(|parent| {
            matches!(
                parent.kind(),
                "program" | "statement_block" | "module" | "export_statement"
            )
        })
}
