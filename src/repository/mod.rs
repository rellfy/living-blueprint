use crate::analysis::FileAnalysis;
use crate::arrangement::Arrangement;
use crate::repository::edges::resolve_edges;
use crate::repository::layout::{apply_layout, plan_columns};
use crate::repository::loader::{LoadedFile, load_files};
use crate::repository::view::{BodyView, LineNumbers, TextView, build_listing_view};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use vello::kurbo::{Point, Rect};

pub mod arrange;
pub mod detail;
pub mod edges;
pub mod layout;
pub mod loader;
pub mod view;

const PLANNING_DETAIL: Detail = Detail::Outline;
const RECT_ANIMATION_RATE: f64 = 14.0;
const SETTLED_RECT_DISTANCE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

pub struct Repository {
    root: PathBuf,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    ids_by_path: HashMap<PathBuf, NodeId>,
    char_advance: f64,
    arrangement: Arrangement,
    dragged_path: Option<PathBuf>,
}

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    pub detail: Detail,
    pub column_count: usize,
    pub rect: Rect,
    pub target_rect: Rect,
    pub header_height: f64,
}

pub enum NodeKind {
    Directory(TextView),
    File(SourceFile),
}

pub struct SourceFile {
    pub text: Option<Arc<str>>,
    pub line_spans: Vec<LineSpan>,
    pub analysis: Option<FileAnalysis>,
    pub outline: Option<TextView>,
    pub summary: Option<TextView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Detail {
    Collapsed,
    Summary,
    Outline,
    Expanded,
}

#[derive(Debug, Clone, Copy)]
pub struct LineSpan {
    pub indent: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub weight: u32,
}

impl Repository {
    pub fn load(root: &Path, char_advance: f64) -> io::Result<Self> {
        let files = load_files(root)?;
        let mut repository = Self::from_files(root, files, char_advance);
        repository.edges = resolve_edges(&repository.nodes);
        repository.plan_layout();
        repository.set_all_details(Detail::Collapsed);
        repository.snap_rects();
        Ok(repository)
    }

    pub fn inherit(&mut self, previous: &Repository) {
        self.arrangement = previous.arrangement.clone();
        self.dragged_path = previous.dragged_path.clone();
        let mut fresh_ids = Vec::new();
        for (index, node) in self.nodes.iter_mut().enumerate() {
            let Some(previous_id) = previous.find_by_path(&node.path) else {
                fresh_ids.push(index);
                continue;
            };
            let previous_node = previous.node(previous_id);
            node.detail = node.nearest_available_detail(previous_node.detail);
            node.rect = previous_node.rect;
        }
        self.relayout();
        for index in fresh_ids {
            self.nodes[index].rect = self.nodes[index].target_rect;
        }
    }

    pub fn details(&self) -> BTreeMap<PathBuf, Detail> {
        self.nodes
            .iter()
            .filter(|node| node.detail != Detail::Collapsed)
            .map(|node| (node.path.clone(), node.detail))
            .collect()
    }

    pub fn restore_details(&mut self, details: &BTreeMap<PathBuf, Detail>) {
        for node in &mut self.nodes {
            let detail = details
                .get(&node.path)
                .copied()
                .unwrap_or(Detail::Collapsed);
            node.detail = node.nearest_available_detail(detail);
        }
        self.relayout();
        self.snap_rects();
    }

    pub fn set_all_details(&mut self, detail: Detail) {
        for node in &mut self.nodes {
            node.detail = detail;
        }
        self.relayout();
    }

    fn plan_layout(&mut self) {
        for node in &mut self.nodes {
            node.detail = match node.get_is_directory() {
                true => Detail::Expanded,
                false => node.nearest_available_detail(PLANNING_DETAIL),
            };
        }
        plan_columns(&mut self.nodes, self.char_advance);
    }

    pub fn relayout(&mut self) {
        apply_layout(
            &mut self.nodes,
            self.char_advance,
            &self.arrangement,
            self.dragged_path.as_deref(),
        );
    }

    pub fn animate(&mut self, delta_time: f64) -> bool {
        let blend = 1.0 - (-RECT_ANIMATION_RATE * delta_time).exp();
        let mut is_animating = false;
        for node in &mut self.nodes {
            if check_is_settled(node.rect, node.target_rect) {
                node.rect = node.target_rect;
                continue;
            }
            is_animating = true;
            node.rect = lerp_rect(node.rect, node.target_rect, blend);
        }
        is_animating
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn root_id(&self) -> NodeId {
        NodeId(0)
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn bounds(&self) -> Rect {
        self.node(self.root_id()).target_rect
    }

    pub fn find_by_path(&self, path: &Path) -> Option<NodeId> {
        self.ids_by_path.get(path).copied()
    }

    pub fn directory_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.nodes
            .iter()
            .filter(|node| node.get_is_directory())
            .map(|node| self.root.join(&node.path))
    }

    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        let mut current = self.root_id();
        if !self.node(current).rect.contains(point) {
            return None;
        }
        while let Some(child) = self.find_child_at(current, point) {
            current = child;
        }
        Some(current)
    }

    pub fn check_is_within(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut current = Some(id);
        while let Some(node_id) = current {
            if node_id == ancestor {
                return true;
            }
            current = self.node(node_id).parent;
        }
        false
    }

    pub fn visible_ancestor(&self, id: NodeId) -> NodeId {
        let mut visible = id;
        let mut current = self.node(id).parent;
        while let Some(ancestor) = current {
            if self.node(ancestor).get_is_collapsed_directory() {
                visible = ancestor;
            }
            current = self.node(ancestor).parent;
        }
        visible
    }

    pub fn snap_rects(&mut self) {
        for node in &mut self.nodes {
            node.rect = node.target_rect;
        }
    }

    fn find_child_at(&self, id: NodeId, point: Point) -> Option<NodeId> {
        let node = self.node(id);
        if node.get_is_collapsed_directory() {
            return None;
        }
        node.children
            .iter()
            .copied()
            .find(|child| self.node(*child).rect.contains(point))
    }

    fn from_files(root: &Path, files: Vec<LoadedFile>, char_advance: f64) -> Self {
        let mut repository = Self {
            root: root.to_path_buf(),
            nodes: vec![create_directory_node(root_name(root), PathBuf::new(), None)],
            edges: Vec::new(),
            ids_by_path: HashMap::new(),
            char_advance,
            arrangement: Arrangement::default(),
            dragged_path: None,
        };
        repository.ids_by_path.insert(PathBuf::new(), NodeId(0));
        for file in files {
            repository.insert_file(file);
        }
        repository.sort_children();
        repository.build_listings();
        repository
    }

    fn insert_file(&mut self, file: LoadedFile) {
        let parent = self.ensure_directory(file.path.parent().unwrap_or(Path::new("")));
        let name = file_name(&file.path);
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            name,
            path: file.path.clone(),
            parent: Some(parent),
            children: Vec::new(),
            kind: NodeKind::File(file.source),
            detail: Detail::Expanded,
            column_count: 1,
            rect: Rect::ZERO,
            target_rect: Rect::ZERO,
            header_height: 0.0,
        });
        self.nodes[parent.0].children.push(id);
        self.ids_by_path.insert(file.path, id);
    }

    fn ensure_directory(&mut self, path: &Path) -> NodeId {
        if let Some(id) = self.ids_by_path.get(path) {
            return *id;
        }
        let parent = self.ensure_directory(path.parent().unwrap_or(Path::new("")));
        let id = NodeId(self.nodes.len());
        self.nodes.push(create_directory_node(
            file_name(path),
            path.to_path_buf(),
            Some(parent),
        ));
        self.nodes[parent.0].children.push(id);
        self.ids_by_path.insert(path.to_path_buf(), id);
        id
    }

    fn sort_children(&mut self) {
        for index in 0..self.nodes.len() {
            let mut children = std::mem::take(&mut self.nodes[index].children);
            children.sort_by_key(|child| sort_key(&self.nodes[child.0]));
            self.nodes[index].children = children;
        }
    }

    fn build_listings(&mut self) {
        for index in 0..self.nodes.len() {
            if !self.nodes[index].get_is_directory() {
                continue;
            }
            let children: Vec<&Node> = self.nodes[index]
                .children
                .iter()
                .map(|child| &self.nodes[child.0])
                .collect();
            self.nodes[index].kind = NodeKind::Directory(build_listing_view(&children));
        }
    }
}

impl Node {
    pub fn source_file(&self) -> Option<&SourceFile> {
        match &self.kind {
            NodeKind::File(source_file) => Some(source_file),
            NodeKind::Directory(_) => None,
        }
    }

    pub fn get_is_directory(&self) -> bool {
        matches!(self.kind, NodeKind::Directory(_))
    }

    pub fn get_is_collapsed_directory(&self) -> bool {
        self.get_is_directory() && self.detail == Detail::Collapsed
    }

    pub fn get_is_expanded_directory(&self) -> bool {
        self.get_is_directory() && self.detail == Detail::Expanded
    }

    pub fn body(&self) -> Option<BodyView<'_>> {
        match (&self.kind, self.detail) {
            (NodeKind::Directory(listing), Detail::Collapsed) => Some(listing.as_body()),
            (NodeKind::Directory(_), _) => None,
            (NodeKind::File(source_file), detail) => source_file.body(detail),
        }
    }

    pub fn available_details(&self) -> Vec<Detail> {
        let NodeKind::File(source_file) = &self.kind else {
            return vec![Detail::Collapsed, Detail::Expanded];
        };
        [
            (Detail::Collapsed, true),
            (Detail::Summary, source_file.summary.is_some()),
            (Detail::Outline, source_file.outline.is_some()),
            (Detail::Expanded, true),
        ]
        .into_iter()
        .filter(|(_, is_available)| *is_available)
        .map(|(detail, _)| detail)
        .collect()
    }

    fn nearest_available_detail(&self, detail: Detail) -> Detail {
        self.available_details()
            .into_iter()
            .rev()
            .find(|available| *available <= detail)
            .unwrap_or(Detail::Collapsed)
    }
}

impl SourceFile {
    fn body(&self, detail: Detail) -> Option<BodyView<'_>> {
        match detail {
            Detail::Expanded => self.code_body(),
            Detail::Outline => self.outline.as_ref().map(TextView::as_body),
            Detail::Summary => self.summary.as_ref().map(TextView::as_body),
            Detail::Collapsed => None,
        }
    }

    fn code_body(&self) -> Option<BodyView<'_>> {
        let highlights = self
            .analysis
            .as_ref()
            .map(|analysis| analysis.highlights.as_slice())
            .unwrap_or_default();
        Some(BodyView {
            text: self.text.as_deref()?,
            line_spans: &self.line_spans,
            highlights,
            line_numbers: LineNumbers::Sequential,
        })
    }
}

fn create_directory_node(name: String, path: PathBuf, parent: Option<NodeId>) -> Node {
    Node {
        name,
        path,
        parent,
        children: Vec::new(),
        kind: NodeKind::Directory(TextView::new(String::new(), Vec::new())),
        detail: Detail::Expanded,
        column_count: 1,
        rect: Rect::ZERO,
        target_rect: Rect::ZERO,
        header_height: 0.0,
    }
}

fn check_is_settled(rect: Rect, target: Rect) -> bool {
    (rect.origin() - target.origin()).hypot() < SETTLED_RECT_DISTANCE
        && (rect.size() - target.size()).to_vec2().hypot() < SETTLED_RECT_DISTANCE
}

fn lerp_rect(rect: Rect, target: Rect, blend: f64) -> Rect {
    Rect::from_points(
        rect.origin().lerp(target.origin(), blend),
        Point::new(rect.x1, rect.y1).lerp(Point::new(target.x1, target.y1), blend),
    )
}

fn sort_key(node: &Node) -> (bool, String) {
    (!node.get_is_directory(), node.name.to_lowercase())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn root_name(root: &Path) -> String {
    match file_name(root).is_empty() {
        true => root.to_string_lossy().into_owned(),
        false => file_name(root),
    }
}

#[cfg(test)]
mod tests {
    use crate::repository::layout::CODE_FONT_SIZE;
    use crate::repository::{Detail, NodeKind, Repository};
    use std::path::Path;

    fn load_expanded() -> Repository {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut repository = Repository::load(root, CODE_FONT_SIZE * 0.6).unwrap();
        repository.set_all_details(Detail::Expanded);
        repository.animate(10.0);
        repository
    }

    #[test]
    fn loads_fully_collapsed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repository = Repository::load(root, CODE_FONT_SIZE * 0.6).unwrap();
        let is_all_collapsed = repository
            .nodes()
            .iter()
            .all(|node| node.detail == Detail::Collapsed);
        assert!(is_all_collapsed);
        assert_eq!(
            repository.hit_test(repository.bounds().center()),
            Some(repository.root_id())
        );
    }

    #[test]
    fn loads_own_repository_with_nested_layout() {
        let repository = load_expanded();
        let main_id = repository.find_by_path(Path::new("src/main.rs")).unwrap();
        let main = repository.node(main_id);
        assert!(matches!(main.kind, NodeKind::File(_)));
        assert!(repository.bounds().contains(main.rect.center()));
        let parent = repository.node(main.parent.unwrap());
        assert!(parent.rect.contains(main.rect.origin()));
        assert_eq!(repository.hit_test(main.rect.center()), Some(main_id));
        assert!(!repository.edges().is_empty());
    }

    #[test]
    fn restores_captured_details() {
        let mut repository = load_expanded();
        let main = Path::new("src/main.rs");
        let id = repository.find_by_path(main).unwrap();
        repository.compress(id);
        let details = repository.details();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut restored = Repository::load(root, CODE_FONT_SIZE * 0.6).unwrap();
        restored.restore_details(&details);
        assert_eq!(restored.details(), details);
        let restored_id = restored.find_by_path(main).unwrap();
        assert_eq!(
            restored.node(restored_id).target_rect,
            restored.node(restored_id).rect
        );
    }

    #[test]
    fn siblings_do_not_overlap() {
        let repository = load_expanded();
        for node in repository.nodes() {
            for (index, first) in node.children.iter().enumerate() {
                for second in &node.children[index + 1..] {
                    let overlap = repository
                        .node(*first)
                        .rect
                        .intersect(repository.node(*second).rect);
                    assert!(overlap.area() <= 0.0);
                }
            }
        }
    }
}
