use crate::State;
use crate::agent::AgentEvent;
use crate::agent::process::AgentProcess;
use crate::resources::agent_feed::{AgentFeed, FeedKind};
use crate::resources::agent_sessions::{AgentSessions, AgentStatus};
use std::path::Path;

pub mod state;

const BUSY_MESSAGE: &str = "busy, wait for the current run to finish";
const FINISHED_MESSAGE: &str = "done";
const FAILED_MESSAGE: &str = "failed";

pub fn submit_prompt(state: &mut State) {
    let prompt = state.prompt.text().trim().to_string();
    if prompt.is_empty() {
        return;
    }
    if state.agent_state.process.is_some() {
        state.agent_feed.push(FeedKind::Failure, BUSY_MESSAGE);
        return;
    }
    state.prompt.reset();
    let process = AgentProcess::spawn(&prompt, &state.repository_root);
    let session_id = process.session_id().to_string();
    state.agent_state.process = Some(process);
    state
        .agent_state
        .spawned_session_ids
        .insert(session_id.clone());
    let session = state.agent_sessions.get_or_create(&session_id);
    session.status = AgentStatus::Working;
    state.agent_feed.push(FeedKind::Prompt, &prompt);
}

pub fn update_agent(state: &mut State) {
    poll_process(state);
    poll_transcripts(state);
    state.agent_sessions.update(state.delta_time);
    state.agent_feed.update(state.delta_time);
}

fn poll_process(state: &mut State) {
    let Some(process) = state.agent_state.process.as_mut() else {
        return;
    };
    let events = process.poll_events();
    let is_running = process.get_is_running();
    let session_id = process.session_id().to_string();
    for event in events {
        apply_event(
            &mut state.agent_sessions,
            &mut state.agent_feed,
            &session_id,
            event,
        );
    }
    if is_running {
        return;
    }
    state.agent_state.process = None;
    let session = state.agent_sessions.get_or_create(&session_id);
    if session.status == AgentStatus::Working {
        session.status = AgentStatus::Succeeded;
    }
}

fn poll_transcripts(state: &mut State) {
    let agent_state = &state.agent_state;
    for (session_id, event) in agent_state.transcript_watcher.poll_events() {
        if agent_state.spawned_session_ids.contains(&session_id) {
            continue;
        }
        apply_event(
            &mut state.agent_sessions,
            &mut state.agent_feed,
            &session_id,
            event,
        );
    }
}

fn apply_event(
    sessions: &mut AgentSessions,
    feed: &mut AgentFeed,
    session_id: &str,
    event: AgentEvent,
) {
    let session = sessions.get_or_create(session_id);
    session.silent_seconds = 0.0;
    match event {
        AgentEvent::Started => {
            session.status = AgentStatus::Working;
            session.idle_seconds = 0.0;
        }
        AgentEvent::Prompt(prompt) => {
            session.status = AgentStatus::Working;
            session.idle_seconds = 0.0;
            feed.push(FeedKind::Prompt, &prompt);
        }
        AgentEvent::Text(text) => feed.push(FeedKind::Message, &text),
        AgentEvent::ToolUse { tool, paths } => {
            session.status = AgentStatus::Working;
            feed.push(
                FeedKind::ToolUse,
                &describe_tool_use(&tool, paths.first().map(|path| path.as_path())),
            );
            if let Some(path) = paths.into_iter().next() {
                session.focus_path = Some(path);
            }
        }
        AgentEvent::Finished {
            is_success,
            summary,
        } => {
            if !session.get_is_working() {
                return;
            }
            session.status = match is_success {
                true => AgentStatus::Succeeded,
                false => AgentStatus::Failed,
            };
            push_finish_entry(feed, is_success, summary);
        }
        AgentEvent::Failed(message) => {
            session.status = AgentStatus::Failed;
            feed.push(FeedKind::Failure, &message);
        }
    }
}

fn push_finish_entry(feed: &mut AgentFeed, is_success: bool, summary: Option<String>) {
    let kind = match is_success {
        true => FeedKind::Success,
        false => FeedKind::Failure,
    };
    let last_message = feed
        .entries
        .iter()
        .rev()
        .find(|entry| entry.kind == FeedKind::Message)
        .map(|entry| entry.text.clone());
    let summary = summary.unwrap_or_default();
    let is_repeated = last_message.is_some_and(|message| message == summary.trim());
    let text = match (is_success, summary.trim().is_empty() || is_repeated) {
        (true, true) => FINISHED_MESSAGE.to_string(),
        (false, true) => FAILED_MESSAGE.to_string(),
        (_, false) => summary,
    };
    feed.push(kind, &text);
}

fn describe_tool_use(tool: &str, path: Option<&Path>) -> String {
    match path {
        Some(path) => format!("{tool} {}", path.display()),
        None => tool.to_string(),
    }
}
