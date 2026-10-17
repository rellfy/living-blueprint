use super::OutlineItem;
use super::language::Language;
use std::cell::RefCell;
use tree_sitter::{Node, Parser};

const MAX_SIGNATURE_CHARS: usize = 100;
const SIGNATURE_TRAILING_CHARS: &[char] = &[' ', '{', ':', ';', '=', '('];
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
    ],
    containers: &["impl_item", "trait_item", "mod_item"],
    wrappers: &[],
    top_level_only: &[],
};

const PYTHON_RULES: OutlineRules = OutlineRules {
    items: &["function_definition", "class_definition"],
    containers: &["class_definition"],
    wrappers: &[("decorated_definition", "definition")],
    top_level_only: &[],
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
    ],
    containers: &[
        "class_declaration",
        "abstract_class_declaration",
        "internal_module",
        "module",
    ],
    wrappers: &[("export_statement", "declaration")],
    top_level_only: &["lexical_declaration"],
};

const GO_RULES: OutlineRules = OutlineRules {
    items: &[
        "function_declaration",
        "method_declaration",
        "type_declaration",
        "const_declaration",
        "var_declaration",
    ],
    containers: &[],
    wrappers: &[],
    top_level_only: &[],
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
    let is_item =
        rules.items.contains(&kind) && (depth == 0 || !rules.top_level_only.contains(&kind));
    if !is_item {
        return;
    }
    let index = items.len();
    items.push(OutlineItem {
        depth,
        line: outer.start_position().row,
        signature: extract_signature(definition, source),
        child_count: 0,
        is_constant: CONSTANT_KINDS.contains(&kind),
    });
    let body = definition.child_by_field_name("body");
    let Some(body) = body.filter(|_| rules.containers.contains(&kind)) else {
        return;
    };
    collect_items(body, source, rules, depth + 1, items);
    items[index].child_count = items[index + 1..]
        .iter()
        .filter(|item| item.depth == depth + 1)
        .count();
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

fn extract_signature(definition: Node, source: &str) -> String {
    let end = match definition.child_by_field_name("body") {
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
