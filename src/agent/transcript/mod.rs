use crate::agent::{AgentEvent, PATH_INPUT_KEYS, make_relative};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub mod watcher;

const TURN_END_REASONS: &[&str] = &["end_turn", "max_tokens"];
const SYNTHETIC_MODEL: &str = "<synthetic>";
const COMMAND_PREFIX: char = '<';
const SHELL_SEPARATORS: &[char] = &['\'', '"', '|', ';', '&', '<', '>', '(', ')', '=', '`'];

pub fn parse_transcript_line(line: &str, repository_root: &Path) -> Vec<AgentEvent> {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let Some(working_directory) = value["cwd"].as_str().map(Path::new) else {
        return Vec::new();
    };
    if !working_directory.starts_with(repository_root) {
        return Vec::new();
    }
    match value["type"].as_str() {
        Some("user") => parse_user(&value).into_iter().collect(),
        Some("assistant") => parse_assistant(&value, working_directory, repository_root),
        _ => Vec::new(),
    }
}

fn parse_user(value: &Value) -> Option<AgentEvent> {
    let text = value["message"]["content"].as_str()?;
    let is_meta = value["isMeta"].as_bool().unwrap_or(false);
    if is_meta || text.starts_with(COMMAND_PREFIX) {
        return None;
    }
    match value["isSidechain"].as_bool().unwrap_or(false) {
        true => Some(AgentEvent::Started),
        false => Some(AgentEvent::Prompt(text.to_string())),
    }
}

fn parse_assistant(
    value: &Value,
    working_directory: &Path,
    repository_root: &Path,
) -> Vec<AgentEvent> {
    let message = &value["message"];
    let Some(blocks) = message["content"].as_array() else {
        return Vec::new();
    };
    if message["model"].as_str() == Some(SYNTHETIC_MODEL) {
        return vec![AgentEvent::Finished {
            is_success: false,
            summary: blocks
                .iter()
                .find_map(|block| block["text"].as_str())
                .map(str::to_string),
        }];
    }
    let is_turn_end = message["stop_reason"]
        .as_str()
        .is_some_and(|reason| TURN_END_REASONS.contains(&reason));
    blocks
        .iter()
        .filter_map(|block| parse_block(block, is_turn_end, working_directory, repository_root))
        .collect()
}

fn parse_block(
    block: &Value,
    is_turn_end: bool,
    working_directory: &Path,
    repository_root: &Path,
) -> Option<AgentEvent> {
    match block["type"].as_str()? {
        "text" => {
            let text = block["text"].as_str()?.to_string();
            Some(match is_turn_end {
                true => AgentEvent::Finished {
                    is_success: true,
                    summary: Some(text),
                },
                false => AgentEvent::Text(text),
            })
        }
        "tool_use" => {
            let input = &block["input"];
            let command = input["command"].as_str().unwrap_or_default();
            let paths = collect_paths(input, working_directory, repository_root)
                .into_iter()
                .chain(collect_command_paths(
                    command,
                    working_directory,
                    repository_root,
                ))
                .collect();
            Some(AgentEvent::ToolUse {
                tool: block["name"].as_str()?.to_string(),
                paths,
            })
        }
        _ => None,
    }
}

fn collect_paths(input: &Value, working_directory: &Path, repository_root: &Path) -> Vec<PathBuf> {
    PATH_INPUT_KEYS
        .iter()
        .filter_map(|key| input[*key].as_str())
        .map(|path| make_relative(&working_directory.join(path), repository_root))
        .collect()
}

fn collect_command_paths(
    command: &str,
    working_directory: &Path,
    repository_root: &Path,
) -> Vec<PathBuf> {
    command
        .split(|character: char| character.is_whitespace() || SHELL_SEPARATORS.contains(&character))
        .map(|token| token.split(':').next().unwrap_or_default())
        .filter(|token| !token.is_empty() && !token.starts_with('-'))
        .map(|token| make_relative(&working_directory.join(token), repository_root))
        .filter(|path| !path.as_os_str().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> PathBuf {
        PathBuf::from("/repo")
    }

    #[test]
    fn parses_typed_prompt() {
        let line = r#"{"type":"user","cwd":"/repo","message":{"content":"fix it"}}"#;
        let events = parse_transcript_line(line, &repository_root());
        assert!(matches!(&events[..], [AgentEvent::Prompt(prompt)] if prompt == "fix it"));
    }

    #[test]
    fn subagent_task_starts_without_prompt() {
        let line =
            r#"{"type":"user","isSidechain":true,"cwd":"/repo","message":{"content":"task"}}"#;
        let events = parse_transcript_line(line, &repository_root());
        assert!(matches!(&events[..], [AgentEvent::Started]));
    }

    #[test]
    fn ignores_tool_results_and_commands() {
        let tool_result =
            r#"{"type":"user","cwd":"/repo","message":{"content":[{"type":"tool_result"}]}}"#;
        let command = r#"{"type":"user","cwd":"/repo","message":{"content":"<command-name>/clear</command-name>"}}"#;
        assert!(parse_transcript_line(tool_result, &repository_root()).is_empty());
        assert!(parse_transcript_line(command, &repository_root()).is_empty());
    }

    #[test]
    fn parses_tool_use_relative_to_working_directory() {
        let line = r#"{"type":"assistant","cwd":"/repo/src","message":{"stop_reason":"tool_use","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"main.rs"}}]}}"#;
        let events = parse_transcript_line(line, &repository_root());
        let [AgentEvent::ToolUse { tool, paths }] = &events[..] else {
            panic!("unexpected events: {events:?}");
        };
        assert_eq!(tool, "Edit");
        assert_eq!(paths, &vec![PathBuf::from("src/main.rs")]);
    }

    #[test]
    fn extracts_candidate_paths_from_shell_commands() {
        let line = r#"{"type":"assistant","cwd":"/repo","message":{"stop_reason":"tool_use","content":[{"type":"tool_use","name":"Bash","input":{"command":"cd /repo && grep -n foo src/a.rs:12 | head -5"}}]}}"#;
        let events = parse_transcript_line(line, &repository_root());
        let [AgentEvent::ToolUse { paths, .. }] = &events[..] else {
            panic!("unexpected events: {events:?}");
        };
        assert!(paths.contains(&PathBuf::from("src/a.rs")));
        assert!(!paths.contains(&PathBuf::new()));
    }

    #[test]
    fn final_text_finishes_the_turn() {
        let line = r#"{"type":"assistant","cwd":"/repo","message":{"stop_reason":"end_turn","content":[{"type":"text","text":"done it"}]}}"#;
        let events = parse_transcript_line(line, &repository_root());
        assert!(matches!(
            &events[..],
            [AgentEvent::Finished { is_success: true, summary: Some(summary) }] if summary == "done it"
        ));
    }

    #[test]
    fn interruption_fails_the_turn() {
        let line = r#"{"type":"assistant","cwd":"/repo","message":{"model":"<synthetic>","content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#;
        let events = parse_transcript_line(line, &repository_root());
        assert!(matches!(
            &events[..],
            [AgentEvent::Finished {
                is_success: false,
                ..
            }]
        ));
    }

    #[test]
    fn ignores_other_repositories() {
        let line = r#"{"type":"user","cwd":"/repo-other","message":{"content":"hi"}}"#;
        assert!(parse_transcript_line(line, &repository_root()).is_empty());
    }
}
