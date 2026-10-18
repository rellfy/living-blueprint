use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use vello::kurbo::Vec2;

pub mod resolve;
pub mod storage;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arrangement {
    pub positions: BTreeMap<PathBuf, Position>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Arrangement {
    pub fn position(&self, path: &Path) -> Option<Vec2> {
        self.positions
            .get(path)
            .map(|position| Vec2::new(position.x, position.y))
    }

    pub fn set_position(&mut self, path: &Path, offset: Vec2) {
        self.positions.insert(
            path.to_path_buf(),
            Position {
                x: offset.x,
                y: offset.y,
            },
        );
    }

    pub fn retain_paths(&mut self, mut check_is_kept: impl FnMut(&Path) -> bool) {
        self.positions.retain(|path, _| check_is_kept(path));
    }

    pub fn check_is_arranged<'a>(&self, mut paths: impl Iterator<Item = &'a Path>) -> bool {
        paths.any(|path| self.positions.contains_key(path))
    }
}
