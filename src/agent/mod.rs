use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use std::time::Duration;

const CLAUDE_COMMAND: &str = "claude";
const CLAUDE_ARGS: &[&str] = &["--output-format", "stream-json", "--verbose"];
const PATH_INPUT_KEYS: &[&str] = &["file_path", "path", "notebook_path"];
const STDERR_TAIL_LINES: usize = 8;
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Text(String),
    ToolUse {
        tool: String,
        paths: Vec<PathBuf>,
    },
    Finished {
        is_success: bool,
        summary: Option<String>,
    },
    Failed(String),
}

pub struct AgentProcess {
    child: Option<Arc<Mutex<Child>>>,
    receiver: Receiver<AgentEvent>,
    is_closed: bool,
}

impl AgentProcess {
    pub fn spawn(prompt: &str, working_directory: &Path) -> Self {
        let (sender, receiver) = channel();
        let child = match spawn_claude(prompt, working_directory) {
            Ok(child) => child,
            Err(message) => {
                let _ = sender.send(AgentEvent::Failed(message));
                return Self {
                    child: None,
                    receiver,
                    is_closed: false,
                };
            }
        };
        let child = Arc::new(Mutex::new(child));
        start_reader(child.clone(), sender, working_directory.to_path_buf());
        Self {
            child: Some(child),
            receiver,
            is_closed: false,
        }
    }

    pub fn poll_events(&mut self) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        while !self.is_closed {
            match self.receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => self.is_closed = true,
            }
        }
        events
    }

    pub fn get_is_running(&self) -> bool {
        !self.is_closed
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        let Some(child) = &self.child else {
            return;
        };
        let Ok(mut child) = child.lock() else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn parse_stream_line(line: &str, working_directory: &Path) -> Vec<AgentEvent> {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    match value["type"].as_str() {
        Some("assistant") => parse_assistant_content(&value, working_directory),
        Some("result") => vec![parse_result(&value)],
        _ => Vec::new(),
    }
}

fn parse_assistant_content(value: &Value, working_directory: &Path) -> Vec<AgentEvent> {
    let Some(blocks) = value["message"]["content"].as_array() else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter_map(|block| parse_content_block(block, working_directory))
        .collect()
}

fn parse_content_block(block: &Value, working_directory: &Path) -> Option<AgentEvent> {
    match block["type"].as_str()? {
        "text" => Some(AgentEvent::Text(block["text"].as_str()?.to_string())),
        "tool_use" => Some(AgentEvent::ToolUse {
            tool: block["name"].as_str()?.to_string(),
            paths: collect_paths(&block["input"], working_directory),
        }),
        _ => None,
    }
}

fn collect_paths(input: &Value, working_directory: &Path) -> Vec<PathBuf> {
    PATH_INPUT_KEYS
        .iter()
        .filter_map(|key| input[*key].as_str())
        .map(|path| make_relative(Path::new(path), working_directory))
        .collect()
}

fn make_relative(path: &Path, working_directory: &Path) -> PathBuf {
    match path.strip_prefix(working_directory) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => path.to_path_buf(),
    }
}

fn parse_result(value: &Value) -> AgentEvent {
    let is_error = value["is_error"].as_bool().unwrap_or(false);
    let is_success_subtype = value["subtype"].as_str() == Some("success");
    AgentEvent::Finished {
        is_success: !is_error && is_success_subtype,
        summary: value["result"].as_str().map(str::to_string),
    }
}

fn spawn_claude(prompt: &str, working_directory: &Path) -> Result<Child, String> {
    Command::new(CLAUDE_COMMAND)
        .arg("-p")
        .arg(prompt)
        .args(CLAUDE_ARGS)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start {CLAUDE_COMMAND}: {error}"))
}

fn start_reader(child: Arc<Mutex<Child>>, sender: Sender<AgentEvent>, working_directory: PathBuf) {
    let (stdout, stderr) = take_pipes(&child);
    let stderr_thread = spawn(move || collect_stderr_tail(stderr));
    spawn(move || {
        let has_result = forward_stdout(stdout, &sender, &working_directory);
        let is_exit_success = wait_for_exit(&child);
        let stderr_tail = stderr_thread.join().unwrap_or_default();
        if has_result || is_exit_success {
            return;
        }
        let _ = sender.send(AgentEvent::Failed(format_failure(&stderr_tail)));
    });
}

fn take_pipes(child: &Arc<Mutex<Child>>) -> (Option<ChildStdout>, Option<ChildStderr>) {
    let Ok(mut child) = child.lock() else {
        return (None, None);
    };
    (child.stdout.take(), child.stderr.take())
}

fn forward_stdout(
    stdout: Option<impl Read>,
    sender: &Sender<AgentEvent>,
    working_directory: &Path,
) -> bool {
    let Some(stdout) = stdout else {
        return false;
    };
    let mut has_result = false;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        for event in parse_stream_line(&line, working_directory) {
            has_result |= matches!(event, AgentEvent::Finished { .. });
            let _ = sender.send(event);
        }
    }
    has_result
}

fn collect_stderr_tail(stderr: Option<impl Read>) -> Vec<String> {
    let Some(stderr) = stderr else {
        return Vec::new();
    };
    let lines: Vec<String> = BufReader::new(stderr)
        .lines()
        .map_while(Result::ok)
        .collect();
    let start = lines.len().saturating_sub(STDERR_TAIL_LINES);
    lines[start..].to_vec()
}

fn wait_for_exit(child: &Arc<Mutex<Child>>) -> bool {
    loop {
        let Ok(mut child) = child.lock() else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {}
            Err(_) => return false,
        }
        drop(child);
        sleep(EXIT_POLL_INTERVAL);
    }
}

fn format_failure(stderr_tail: &[String]) -> String {
    match stderr_tail.is_empty() {
        true => "agent exited unsuccessfully without a result".to_string(),
        false => stderr_tail.join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn working_directory() -> PathBuf {
        PathBuf::from("/repo")
    }

    #[test]
    fn parses_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#;
        let events = parse_stream_line(line, &working_directory());
        assert!(matches!(&events[..], [AgentEvent::Text(text)] if text == "hi"));
    }

    #[test]
    fn parses_tool_use_with_relative_path() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/repo/src/main.rs","path":"/other/x"}}]}}"#;
        let events = parse_stream_line(line, &working_directory());
        let [AgentEvent::ToolUse { tool, paths }] = &events[..] else {
            panic!("unexpected events: {events:?}");
        };
        assert_eq!(tool, "Edit");
        assert_eq!(
            paths,
            &vec![PathBuf::from("src/main.rs"), PathBuf::from("/other/x")]
        );
    }

    #[test]
    fn parses_result_success() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"done"}"#;
        let events = parse_stream_line(line, &working_directory());
        assert!(matches!(
            &events[..],
            [AgentEvent::Finished { is_success: true, summary: Some(summary) }] if summary == "done"
        ));
    }

    #[test]
    fn parses_result_error() {
        let line = r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#;
        let events = parse_stream_line(line, &working_directory());
        assert!(matches!(
            &events[..],
            [AgentEvent::Finished {
                is_success: false,
                summary: None
            }]
        ));
    }

    #[test]
    fn ignores_garbage_and_other_types() {
        assert!(parse_stream_line("not json", &working_directory()).is_empty());
        assert!(parse_stream_line(r#"{"type":"system"}"#, &working_directory()).is_empty());
        assert!(parse_stream_line(r#"{"type":"user"}"#, &working_directory()).is_empty());
    }
}
