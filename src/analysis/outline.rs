use super::OutlineItem;
use super::language::Language;
use std::cell::RefCell;
use tree_sitter::{Node, Parser};

const MAX_SIGNATURE_CHARS: usize = 100;
const SIGNATURE_TRAILING_CHARS: &[char] = &[' ', '{', ':', ';', '=', '(', ','];
const CONSTANT_KINDS: [&str; 4] = [
    "const_item",
    "static_item",
    "const_declaration",
    "var_declaration",
];

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

struct OutlineRules {
    items: &'static [&'static str],
    containers: &'static [&'static str],
    wrappers: &'static [(&'static str, &'static str)],
    top_level_only: &'static [&'static str],
    member_only: &'static [&'static str],
    public_only: &'static [&'static str],
    check_is_public: fn(Node, &str) -> bool,
    find_body: fn(Node) -> Option<Node>,
}

const RUST_RULES: OutlineRules = OutlineRules {
    items: &[
        "function_item",
        "function_signature_item",
        "struct_item",
        "enum_item",
        "union_item",
        "trait_item",
        "impl_item",
        "mod_item",
        "const_item",
        "static_item",
        "type_item",
        "macro_definition",
        "field_declaration",
    ],
    containers: &["impl_item", "trait_item", "mod_item", "struct_item"],
    wrappers: &[],
    top_level_only: &[],
    member_only: &["field_declaration"],
    public_only: &["field_declaration"],
    check_is_public: check_has_visibility_modifier,
    find_body: find_body_field,
};

const PYTHON_RULES: OutlineRules = OutlineRules {
    items: &[
        "function_definition",
        "class_definition",
        "expression_statement",
    ],
    containers: &["class_definition"],
    wrappers: &[("decorated_definition", "definition")],
    top_level_only: &[],
    member_only: &["expression_statement"],
    public_only: &["expression_statement"],
    check_is_public: check_is_public_python_attribute,
    find_body: find_body_field,
};

const JAVASCRIPT_RULES: OutlineRules = OutlineRules {
    items: &[
        "function_declaration",
        "generator_function_declaration",
        "class_declaration",
        "abstract_class_declaration",
        "method_definition",
        "abstract_method_signature",
        "interface_declaration",
        "type_alias_declaration",
        "enum_declaration",
        "internal_module",
        "module",
        "lexical_declaration",
        "public_field_definition",
        "field_definition",
        "property_signature",
        "method_signature",
    ],
    containers: &[
        "class_declaration",
        "abstract_class_declaration",
        "interface_declaration",
        "internal_module",
        "module",
    ],
    wrappers: &[("export_statement", "declaration")],
    top_level_only: &["lexical_declaration"],
    member_only: &[
        "public_field_definition",
        "field_definition",
        "property_signature",
        "method_signature",
    ],
    public_only: &["public_field_definition", "field_definition"],
    check_is_public: check_is_public_javascript_field,
    find_body: find_body_field,
};

const GO_RULES: OutlineRules = OutlineRules {
    items: &[
        "function_declaration",
        "method_declaration",
        "type_declaration",
        "const_declaration",
        "var_declaration",
        "field_declaration",
    ],
    containers: &["type_declaration"],
    wrappers: &[],
    top_level_only: &[],
    member_only: &["field_declaration"],
    public_only: &["field_declaration"],
    check_is_public: check_is_exported_go_field,
    find_body: find_go_body,
};

pub fn extract_outline(language: Language, source: &str) -> Vec<OutlineItem> {
    let rules = rules_of(language);
    PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        if parser.set_language(&language.grammar()).is_err() {
            return Vec::new();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        collect_items(tree.root_node(), source, rules, 0, &mut items);
        items
    })
}

fn collect_items(
    parent: Node,
    source: &str,
    rules: &OutlineRules,
    depth: usize,
    items: &mut Vec<OutlineItem>,
) {
    let mut cursor = parent.walk();
    for child in parent.named_children(&mut cursor) {
        let Some(definition) = unwrap_definition(child, rules) else {
            continue;
        };
        collect_item(child, definition, source, rules, depth, items);
    }
}

fn collect_item(
    outer: Node,
    definition: Node,
    source: &str,
    rules: &OutlineRules,
    depth: usize,
    items: &mut Vec<OutlineItem>,
) {
    let kind = definition.kind();
    let is_placed = match depth {
        0 => !rules.member_only.contains(&kind),
        _ => !rules.top_level_only.contains(&kind),
    };
    let is_hidden =
        rules.public_only.contains(&kind) && !(rules.check_is_public)(definition, source);
    if !rules.items.contains(&kind) || !is_placed || is_hidden {
        return;
    }
    let index = items.len();
    items.push(OutlineItem {
        depth,
        line: outer.start_position().row,
        signature: extract_signature(definition, source, rules),
        child_count: 0,
        is_constant: CONSTANT_KINDS.contains(&kind),
    });
    let body = (rules.find_body)(definition);
    let Some(body) = body.filter(|_| rules.containers.contains(&kind)) else {
        return;
    };
    collect_items(body, source, rules, depth + 1, items);
    items[index].child_count = items[index + 1..]
        .iter()
        .filter(|item| item.depth == depth + 1)
        .count();
}

fn check_has_visibility_modifier(definition: Node, _source: &str) -> bool {
    let mut cursor = definition.walk();
    definition
        .named_children(&mut cursor)
        .any(|child| child.kind() == "visibility_modifier")
}

fn check_is_public_python_attribute(definition: Node, source: &str) -> bool {
    let target = definition
        .named_child(0)
        .filter(|assignment| assignment.kind() == "assignment")
        .and_then(|assignment| assignment.child_by_field_name("left"))
        .filter(|left| left.kind() == "identifier");
    target.is_some_and(|name| !node_text(name, source).starts_with('_'))
}

fn check_is_public_javascript_field(definition: Node, source: &str) -> bool {
    let name = definition
        .child_by_field_name("name")
        .or_else(|| definition.child_by_field_name("property"));
    let is_private_name = name.is_none_or(|name| name.kind() == "private_property_identifier");
    let mut cursor = definition.walk();
    let is_restricted = definition
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "accessibility_modifier")
        .any(|modifier| node_text(modifier, source) != "public");
    !is_private_name && !is_restricted
}

fn check_is_exported_go_field(definition: Node, source: &str) -> bool {
    let name = definition
        .child_by_field_name("name")
        .or_else(|| definition.child_by_field_name("type"));
    name.is_some_and(|name| {
        node_text(name, source)
            .trim_start_matches('*')
            .rsplit('.')
            .next()
            .and_then(|identifier| identifier.chars().next())
            .is_some_and(char::is_uppercase)
    })
}

fn find_body_field(definition: Node) -> Option<Node> {
    definition.child_by_field_name("body")
}

fn find_go_body(definition: Node) -> Option<Node> {
    let mut cursor = definition.walk();
    let struct_type = definition
        .named_children(&mut cursor)
        .find(|child| child.kind() == "type_spec")
        .and_then(|spec| spec.child_by_field_name("type"))
        .filter(|kind| kind.kind() == "struct_type")?;
    let mut cursor = struct_type.walk();
    struct_type
        .named_children(&mut cursor)
        .find(|child| child.kind() == "field_declaration_list")
}

fn node_text<'source>(node: Node, source: &'source str) -> &'source str {
    source.get(node.byte_range()).unwrap_or_default()
}

fn unwrap_definition<'tree>(node: Node<'tree>, rules: &OutlineRules) -> Option<Node<'tree>> {
    let wrapper_field = rules
        .wrappers
        .iter()
        .find(|(kind, _)| *kind == node.kind())
        .map(|(_, field)| *field);
    match wrapper_field {
        Some(field) => node.child_by_field_name(field),
        None => Some(node),
    }
}

fn extract_signature(definition: Node, source: &str, rules: &OutlineRules) -> String {
    let end = match (rules.find_body)(definition) {
        Some(body) if body.start_byte() > definition.start_byte() => body.start_byte(),
        _ => definition.end_byte(),
    };
    let head = source.get(definition.start_byte()..end).unwrap_or_default();
    let head = match definition.kind() {
        "lexical_declaration" | "const_item" | "static_item" | "var_declaration" => {
            head.lines().next().unwrap_or_default()
        }
        _ => head,
    };
    let collapsed = collapse_whitespace(head);
    truncate(collapsed.trim_end_matches(SIGNATURE_TRAILING_CHARS))
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(",)", ")")
}

fn truncate(text: &str) -> String {
    match text.chars().count() > MAX_SIGNATURE_CHARS {
        true => text
            .chars()
            .take(MAX_SIGNATURE_CHARS - 1)
            .chain(['…'])
            .collect(),
        false => text.to_string(),
    }
}

fn rules_of(language: Language) -> &'static OutlineRules {
    match language {
        Language::Rust => &RUST_RULES,
        Language::Python => &PYTHON_RULES,
        Language::JavaScript | Language::TypeScript | Language::Tsx => &JAVASCRIPT_RULES,
        Language::Go => &GO_RULES,
    }
}
