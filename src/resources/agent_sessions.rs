use std::collections::HashMap;
use std::path::PathBuf;

const STALE_SECONDS: f64 = 300.0;
const EXPIRY_SECONDS: f64 = 3600.0;

#[derive(Default)]
pub struct AgentSessions {
    pub sessions: HashMap<String, AgentSession>,
}

pub struct AgentSession {
    pub status: AgentStatus,
    pub focus_path: Option<PathBuf>,
    pub idle_seconds: f64,
    pub silent_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Working,
    Succeeded,
    Failed,
}

impl AgentSessions {
    pub fn get_or_create(&mut self, session_id: &str) -> &mut AgentSession {
        self.sessions
            .entry(session_id.to_string())
            .or_insert_with(AgentSession::new)
    }

    pub fn update(&mut self, delta_time: f64) {
        for session in self.sessions.values_mut() {
            session.update(delta_time);
        }
        self.sessions
            .retain(|_, session| session.idle_seconds < EXPIRY_SECONDS);
    }
}

impl AgentSession {
    pub fn new() -> Self {
        Self {
            status: AgentStatus::Idle,
            focus_path: None,
            idle_seconds: 0.0,
            silent_seconds: 0.0,
        }
    }

    fn update(&mut self, delta_time: f64) {
        self.silent_seconds += delta_time;
        if self.get_is_working() && self.silent_seconds > STALE_SECONDS {
            self.status = AgentStatus::Succeeded;
        }
        self.idle_seconds = match self.get_is_working() {
            true => 0.0,
            false => self.idle_seconds + delta_time,
        };
    }

    pub fn get_is_working(&self) -> bool {
        self.status == AgentStatus::Working
    }
}
