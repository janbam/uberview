use tree_sitter::Node;

use crate::model::{DefinitionKind, TextSpan};
use crate::source::SourceText;

use super::{
    DefinitionCapture, ExtractOptions, child_field_text, explicit_span, node_span,
    trimmed_node_text,
};

/// Capture one Lua definition if the node contributes structural surface area.
pub fn capture_definition<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    match node.kind() {
        "function_declaration" => capture_function_declaration(node, source),
        "variable_declaration" | "assignment_statement" => {
            capture_assignment_like_definition(node, source)
        }
        "field" => capture_function_field(node, source),
        _ => None,
    }
}

/// Capture one retained Lua snippet such as comments, shebangs, or opt-in returns.
pub fn capture_snippet(node: Node<'_>, options: ExtractOptions) -> Option<TextSpan> {
    match node.kind() {
        "comment" | "hash_bang_line" => Some(node_span(node)),
        "return_statement" if options.show_returns => Some(node_span(node)),
        _ => None,
    }
}

/// Report whether the node is a Lua comment token.
pub fn is_comment_node(node: Node<'_>) -> bool {
    node.kind() == "comment"
}

/// Skip helper nodes that are rendered through a surrounding Lua definition.
pub fn should_skip_node(_node: Node<'_>) -> bool {
    // Lua declaration wrappers are omitted by normal descent; no child helper needs pre-capture pruning.
    false
}

/// Capture a direct Lua function declaration, including local/global declaration aliases.
fn capture_function_declaration<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    let name_node = node.child_by_field_name("name")?;

    Some(DefinitionCapture {
        kind: function_name_kind(name_node),
        name: trimmed_node_text(source, name_node),
        exported: false,
        span: node_span(node),
        header_span: explicit_span(node.start_byte(), function_header_end(node)),
        body: Some(node),
    })
}

/// Capture top-level symbols and function-valued assignments.
fn capture_assignment_like_definition<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    let assignment = assignment_node(node)?;
    let name = first_variable_name(assignment, source)?;

    if let Some(function) = first_assigned_function(assignment) {
        return Some(capture_function_value(node, function, name));
    }

    if is_top_level_assignment_like(node) {
        return Some(DefinitionCapture {
            kind: assignment_kind(node),
            name,
            exported: false,
            span: node_span(node),
            header_span: node_span(node),
            body: None,
        });
    }

    None
}

/// Capture function-valued fields in table constructors.
fn capture_function_field<'tree>(
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    let value = node.child_by_field_name("value")?;

    if value.kind() != "function_definition" {
        return None;
    }

    let name = child_field_text(node, "name", source).or_else(|| {
        node.named_child(0)
            .map(|child| trimmed_node_text(source, child))
    })?;

    Some(capture_function_value(node, value, name))
}

/// Build a definition around a function expression assigned to a Lua symbol.
fn capture_function_value<'tree>(
    owner: Node<'tree>,
    function: Node<'tree>,
    name: String,
) -> DefinitionCapture<'tree> {
    DefinitionCapture {
        kind: DefinitionKind::Function,
        name,
        exported: false,
        span: node_span(owner),
        header_span: explicit_span(owner.start_byte(), function_header_end(function)),
        body: Some(function),
    }
}

/// Return the assignment node regardless of whether Lua wrapped it in a local/global declaration.
fn assignment_node(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "assignment_statement" {
        return Some(node);
    }

    let mut cursor = node.walk();

    node.children(&mut cursor)
        .find(|child| child.kind() == "assignment_statement" || child.kind() == "variable_list")
}

/// Extract the first assigned symbol name from a Lua assignment-like node.
fn first_variable_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    let variable_list = first_named_child_of_kind(node, "variable_list")?;
    let mut cursor = variable_list.walk();

    variable_list
        .children(&mut cursor)
        .find(|child| child.is_named() && is_variable_name_node(*child))
        .map(|child| trimmed_node_text(source, child))
}

/// Return the first function expression assigned by a Lua assignment.
fn first_assigned_function(node: Node<'_>) -> Option<Node<'_>> {
    let expression_list = first_named_child_of_kind(node, "expression_list")?;
    let mut cursor = expression_list.walk();

    expression_list
        .children(&mut cursor)
        .find(|child| child.kind() == "function_definition")
}

/// Find the first direct named child with the requested syntax kind.
fn first_named_child_of_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();

    node.children(&mut cursor)
        .find(|child| child.is_named() && child.kind() == kind)
}

/// Decide whether a syntax node is useful as a Lua symbol name.
fn is_variable_name_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier" | "global" | "dot_index_expression" | "method_index_expression"
    )
}

/// Decide whether a Lua assignment-like node sits where top-level symbols should be surfaced.
fn is_top_level_assignment_like(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if parent.kind() == "chunk" {
        return true;
    }

    parent
        .parent()
        .is_some_and(|grandparent| parent.kind() == "declaration" && grandparent.kind() == "chunk")
}

/// Decide whether a Lua assignment reads as a local variable or broader assignment.
fn assignment_kind(node: Node<'_>) -> DefinitionKind {
    if node.kind() == "variable_declaration" {
        DefinitionKind::Variable
    } else {
        DefinitionKind::Assignment
    }
}

/// Mark colon-defined Lua functions as methods while leaving dotted functions as normal functions.
fn function_name_kind(name: Node<'_>) -> DefinitionKind {
    if name.kind() == "method_index_expression" {
        DefinitionKind::Method
    } else {
        DefinitionKind::Function
    }
}

/// End a retained Lua function header after its parameter list, before leading body comments.
fn function_header_end(function: Node<'_>) -> usize {
    function
        .child_by_field_name("parameters")
        .map_or(function.end_byte(), |parameters| parameters.end_byte())
}
