use std::ffi::OsStr;
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Language as TsLanguage;
use tree_sitter_highlight::HighlightConfiguration;
use tree_sitter_tags::TagsConfiguration;

pub const RECOGNIZED_HIGHLIGHT_NAMES: [&str; 22] = [
    "keyword",
    "string",
    "escape",
    "comment",
    "function",
    "function.macro",
    "macro",
    "type",
    "constructor",
    "tag",
    "constant",
    "boolean",
    "number",
    "attribute",
    "property",
    "operator",
    "punctuation",
    "variable",
    "label",
    "embedded",
    "module",
    "namespace",
];

const LANGUAGE_COUNT: usize = 6;

static HIGHLIGHT_CONFIGS: [OnceLock<Option<HighlightConfiguration>>; LANGUAGE_COUNT] =
    [const { OnceLock::new() }; LANGUAGE_COUNT];

static TAGS_CONFIGS: [OnceLock<Option<TagsConfiguration>>; LANGUAGE_COUNT] =
    [const { OnceLock::new() }; LANGUAGE_COUNT];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Tsx,
    Python,
    Go,
}

struct Queries {
    highlights: String,
    locals: String,
    tags: String,
}

impl Language {
    pub fn detect(path: &Path) -> Option<Self> {
        let extension = path.extension().and_then(OsStr::to_str)?;
        match extension {
            "rs" => Some(Self::Rust),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "py" => Some(Self::Python),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    pub fn highlight_configuration(self) -> Option<&'static HighlightConfiguration> {
        HIGHLIGHT_CONFIGS[self as usize]
            .get_or_init(|| self.build_highlight_configuration())
            .as_ref()
    }

    pub fn tags_configuration(self) -> Option<&'static TagsConfiguration> {
        TAGS_CONFIGS[self as usize]
            .get_or_init(|| self.build_tags_configuration())
            .as_ref()
    }

    pub fn line_comment(self) -> &'static str {
        match self {
            Self::Python => "#",
            _ => "//",
        }
    }

    fn build_highlight_configuration(self) -> Option<HighlightConfiguration> {
        let queries = self.queries();
        let mut configuration = HighlightConfiguration::new(
            self.grammar(),
            format!("{self:?}"),
            &queries.highlights,
            "",
            &queries.locals,
        )
        .ok()?;
        configuration.configure(&RECOGNIZED_HIGHLIGHT_NAMES);
        Some(configuration)
    }

    fn build_tags_configuration(self) -> Option<TagsConfiguration> {
        let queries = self.queries();
        if queries.tags.is_empty() {
            return None;
        }
        TagsConfiguration::new(self.grammar(), &queries.tags, &queries.locals).ok()
    }

    pub fn grammar(self) -> TsLanguage {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }

    fn queries(self) -> Queries {
        match self {
            Self::Rust => Queries {
                highlights: tree_sitter_rust::HIGHLIGHTS_QUERY.into(),
                locals: String::new(),
                tags: tree_sitter_rust::TAGS_QUERY.into(),
            },
            Self::JavaScript => Queries {
                highlights: join_queries(&[
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                    tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
                ]),
                locals: tree_sitter_javascript::LOCALS_QUERY.into(),
                tags: tree_sitter_javascript::TAGS_QUERY.into(),
            },
            Self::TypeScript | Self::Tsx => Queries {
                highlights: join_queries(&[
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                    tree_sitter_typescript::HIGHLIGHTS_QUERY,
                ]),
                locals: tree_sitter_typescript::LOCALS_QUERY.into(),
                tags: join_queries(&[
                    tree_sitter_javascript::TAGS_QUERY,
                    tree_sitter_typescript::TAGS_QUERY,
                ]),
            },
            Self::Python => Queries {
                highlights: tree_sitter_python::HIGHLIGHTS_QUERY.into(),
                locals: String::new(),
                tags: tree_sitter_python::TAGS_QUERY.into(),
            },
            Self::Go => Queries {
                highlights: tree_sitter_go::HIGHLIGHTS_QUERY.into(),
                locals: String::new(),
                tags: tree_sitter_go::TAGS_QUERY.into(),
            },
        }
    }
}

fn join_queries(queries: &[&str]) -> String {
    queries.join("\n")
}
