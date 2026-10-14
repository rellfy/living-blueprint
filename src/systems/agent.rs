use crate::State;
use crate::agent::{AgentEvent, AgentProcess};
use crate::resources::agent_feed::FeedKind;
use crate::resources::agent_session::{AgentSession, AgentStatus};
use crate::systems::input::viewport_size;
use std::path::Path;
use vello::kurbo::{Point, Rect, Vec2};

const ORB_FOLLOW_RATE: f64 = 2.2;
const FADE_RATE: f64 = 4.0;
const ORB_LINGER_SECONDS: f64 = 1.5;
const WANDER_SPEED: f64 = 0.6;
const WANDER_RATIO: f64 = 0.18;
const FREE_WANDER_PX: f64 = 60.0;
const BUSY_MESSAGE: &str = "busy, wait for the current run to finish";
const FINISHED_MESSAGE: &str = "done";
const FAILED_MESSAGE: &str = "failed";

pub fn submit_prompt(state: &mut State) {
    let prompt = state.prompt.text().trim().to_string();
    if prompt.is_empty() {
        return;
    }
    let session = &mut state.agent_session;
    if session.get_is_working() {
        session.feed.push(FeedKind::Failure, BUSY_MESSAGE);
        return;
    }
    state.prompt.clear();
    session.process = Some(AgentProcess::spawn(&prompt, &state.repository_root));
    session.status = AgentStatus::Working;
    session.feed.push(FeedKind::Prompt, &prompt);
    session.focus_path = None;
    session.idle_seconds = 0.0;
}

pub fn update_agent(state: &mut State) {
    let delta_time = state.delta_time;
    poll_process(&mut state.agent_session);
    let session = &mut state.agent_session;
    session.idle_seconds = match session.get_is_working() {
        true => 0.0,
        false => session.idle_seconds + delta_time,
    };
    session.feed.update(delta_time);
    animate_orb(state);
}

fn poll_process(session: &mut AgentSession) {
    let Some(process) = session.process.as_mut() else {
        return;
    };
    let events = process.poll_events();
    let is_running = process.get_is_running();
    for event in events {
        apply_event(session, event);
    }
    if is_running {
        return;
    }
    session.process = None;
    if session.status == AgentStatus::Working {
        session.status = AgentStatus::Succeeded;
    }
}

fn apply_event(session: &mut AgentSession, event: AgentEvent) {
    match event {
        AgentEvent::Text(text) => session.feed.push(FeedKind::Message, &text),
        AgentEvent::ToolUse { tool, paths } => {
            session.feed.push(
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
            session.status = match is_success {
                true => AgentStatus::Succeeded,
                false => AgentStatus::Failed,
            };
            push_finish_entry(session, is_success, summary);
        }
        AgentEvent::Failed(message) => {
            session.status = AgentStatus::Failed;
            session.feed.push(FeedKind::Failure, &message);
        }
    }
}

fn push_finish_entry(session: &mut AgentSession, is_success: bool, summary: Option<String>) {
    let kind = match is_success {
        true => FeedKind::Success,
        false => FeedKind::Failure,
    };
    let last_message = session
        .feed
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
    session.feed.push(kind, &text);
}

fn animate_orb(state: &mut State) {
    let delta_time = state.delta_time;
    let target = compute_orb_target(state);
    let session = &mut state.agent_session;
    let is_orb_visible = session.get_is_working() || session.idle_seconds < ORB_LINGER_SECONDS;
    let orb = &mut session.orb;
    orb.phase += delta_time;
    orb.visibility = approach(
        orb.visibility,
        is_orb_visible && session.status != AgentStatus::Idle,
        delta_time,
    );
    if !orb.is_placed {
        orb.position = target;
        orb.is_placed = orb.visibility > 0.0;
        return;
    }
    let blend = 1.0 - (-ORB_FOLLOW_RATE * delta_time).exp();
    orb.position = orb.position.lerp(target, blend);
    orb.is_placed = orb.visibility > 0.01;
}

fn compute_orb_target(state: &State) -> Point {
    let phase = state.agent_session.orb.phase * WANDER_SPEED;
    let wander = Vec2::new(
        phase.sin() + (phase * 0.37).cos() * 0.5,
        (phase * 1.3).cos() * 0.7,
    );
    match find_focus_rect(state) {
        Some(rect) => {
            let extent = rect.width().min(rect.height());
            let anchor = Point::new(rect.center().x, rect.y0 + extent * 0.5);
            anchor + wander * extent * WANDER_RATIO
        }
        None => {
            let visible = state.camera.visible_world_rect(viewport_size(state));
            let anchor = Point::new(visible.center().x, visible.y0 + visible.height() * 0.3);
            anchor + wander * FREE_WANDER_PX / state.camera.zoom()
        }
    }
}

fn find_focus_rect(state: &State) -> Option<Rect> {
    let repository = state.repository.as_ref()?;
    let path = state.agent_session.focus_path.as_ref()?;
    let id = repository.find_by_path(path)?;
    Some(repository.node(id).rect)
}

fn approach(value: f64, is_shown: bool, delta_time: f64) -> f64 {
    let target = match is_shown {
        true => 1.0,
        false => 0.0,
    };
    value + (target - value) * (1.0 - (-FADE_RATE * delta_time).exp())
}

fn describe_tool_use(tool: &str, path: Option<&Path>) -> String {
    match path {
        Some(path) => format!("{tool} {}", path.display()),
        None => tool.to_string(),
    }
}
