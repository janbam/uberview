use tree_sitter::Node;

use crate::language::LanguageKind;
use crate::model::{DefinitionKind, TextSpan};
use crate::source::SourceText;

use super::{
    DefinitionCapture, ExtractOptions, body_header_end, child_field_text, explicit_span, node_span,
    trimmed_node_text,
};

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
pub fn capture_snippet(node: Node<'_>, options: ExtractOptions) -> Option<TextSpan> {
    match node.kind() {
        "comment" => Some(node_span(node)),
        "return_statement" if options.show_returns => Some(node_span(node)),
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
    let (kind, name) = definition_identity(node, source)?;

    if let Some(body) = node.child_by_field_name("body") {
        return Some(DefinitionCapture {
            kind,
            name,
            span: node_span(node),
            header_span: explicit_span(node.start_byte(), body_header_end(source, body)),
            body: Some(body),
        });
    }

    Some(DefinitionCapture {
        kind,
        name,
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
    let name = child_field_text(declarator, "name", source)?;
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
        let kind = callable_or_class_kind(value);

        if value.kind() == "arrow_function" && body.kind() != "statement_block" {
            return Some(DefinitionCapture {
                kind,
                name,
                span: node_span(node),
                header_span: node_span(node),
                body: None,
            });
        }

        return Some(DefinitionCapture {
            kind,
            name,
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
            kind: callable_or_class_kind(value),
            name,
            span: node_span(node),
            header_span: node_span(node),
            body: None,
        });
    }

    if is_top_level_const(node) {
        return Some(DefinitionCapture {
            kind: variable_definition_kind(node, source),
            name,
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
                capture_variable_definition(declaration, source).map(|capture| DefinitionCapture {
                    kind: capture.kind,
                    name: capture.name,
                    span: node_span(node),
                    header_span: explicit_span(node.start_byte(), capture.header_span.end_byte),
                    body: capture.body,
                })
            }
            _ => capture_container_or_leaf(declaration, source).map(|capture| DefinitionCapture {
                kind: capture.kind,
                name: capture.name,
                span: node_span(node),
                header_span: explicit_span(node.start_byte(), capture.header_span.end_byte),
                body: capture.body,
            }),
        };
    }

    let value = node.child_by_field_name("value")?;

    if language == LanguageKind::JavaScript && value.is_named() {
        return Some(DefinitionCapture {
            kind: export_value_kind(value),
            name: export_value_name(value, source),
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
    let name = child_field_text(node, "key", source).or_else(|| {
        node.named_child(0)
            .map(|child| trimmed_node_text(source, child))
    })?;

    if matches!(
        value.kind(),
        "arrow_function" | "function_expression" | "generator_function"
    ) && let Some(body) = value.child_by_field_name("body")
    {
        return Some(DefinitionCapture {
            kind: DefinitionKind::Function,
            name,
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

/// Derive the user-facing kind and name for a directly retained JavaScript-family node.
fn definition_identity(node: Node<'_>, source: &SourceText) -> Option<(DefinitionKind, String)> {
    let kind = match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "generator_function"
        | "function_signature" => DefinitionKind::Function,
        "method_definition" | "method_signature" | "abstract_method_signature" => {
            DefinitionKind::Method
        }
        "class_declaration" | "class" | "abstract_class_declaration" => DefinitionKind::Class,
        "interface_declaration" => DefinitionKind::Interface,
        "enum_declaration" => DefinitionKind::Enum,
        "type_alias_declaration" => DefinitionKind::TypeAlias,
        "module" | "internal_module" => DefinitionKind::Namespace,
        "property_signature" => DefinitionKind::Property,
        "public_field_definition" => DefinitionKind::Field,
        _ => return None,
    };

    let name = ["name", "property"]
        .into_iter()
        .find_map(|field_name| child_field_text(node, field_name, source))
        .or_else(|| fallback_name_from_header(node, source))?;

    Some((kind, name))
}

/// Decide whether a wrapped value reads as a function-like or class-like definition.
fn callable_or_class_kind(node: Node<'_>) -> DefinitionKind {
    match node.kind() {
        "class" | "class_declaration" => DefinitionKind::Class,
        _ => DefinitionKind::Function,
    }
}

/// Decide whether a top-level variable declaration reads as a constant or variable.
fn variable_definition_kind(node: Node<'_>, source: &SourceText) -> DefinitionKind {
    if trimmed_node_text(source, node).starts_with("const ") {
        DefinitionKind::Constant
    } else {
        DefinitionKind::Variable
    }
}

/// Decide which synthetic kind to show for export statements without an inline declaration.
fn export_value_kind(node: Node<'_>) -> DefinitionKind {
    match node.kind() {
        "class" | "class_declaration" => DefinitionKind::Class,
        "function" | "function_declaration" | "function_expression" => DefinitionKind::Function,
        _ => DefinitionKind::Constant,
    }
}

/// Derive a readable export label when the export does not carry a normal declaration form.
fn export_value_name(node: Node<'_>, source: &SourceText) -> String {
    child_field_text(node, "name", source)
        .or_else(|| fallback_name_from_header(node, source))
        .unwrap_or_else(|| "default".to_owned())
}

/// Fall back to the first source line when no dedicated name field is available.
fn fallback_name_from_header(node: Node<'_>, source: &SourceText) -> Option<String> {
    trimmed_node_text(source, node)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}
