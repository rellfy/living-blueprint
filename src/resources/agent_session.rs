use crate::agent::AgentProcess;
use crate::resources::agent_feed::AgentFeed;
use std::path::PathBuf;
use vello::kurbo::{Point, Vec2};

pub struct AgentSession {
    pub process: Option<AgentProcess>,
    pub status: AgentStatus,
    pub focus_path: Option<PathBuf>,
    pub orb: Orb,
    pub feed: AgentFeed,
    pub idle_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Working,
    Succeeded,
    Failed,
}

pub struct Orb {
    pub position: Point,
    pub velocity: Vec2,
    pub visibility: f64,
    pub phase: f64,
    pub is_placed: bool,
}

impl AgentSession {
    pub fn new() -> Self {
        Self {
            process: None,
            status: AgentStatus::Idle,
            focus_path: None,
            orb: Orb {
                position: Point::ZERO,
                velocity: Vec2::ZERO,
                visibility: 0.0,
                phase: 0.0,
                is_placed: false,
            },
            feed: AgentFeed::default(),
            idle_seconds: 0.0,
        }
    }

    pub fn get_is_working(&self) -> bool {
        self.status == AgentStatus::Working
    }
}
