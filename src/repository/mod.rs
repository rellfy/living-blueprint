use crate::analysis::FileAnalysis;
use crate::repository::edges::resolve_edges;
use crate::repository::layout::apply_layout;
use crate::repository::loader::{LoadedFile, load_files};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use vello::kurbo::{Point, Rect};

pub mod edges;
pub mod layout;
pub mod loader;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

pub struct Repository {
    root: PathBuf,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    ids_by_path: HashMap<PathBuf, NodeId>,
}

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    pub rect: Rect,
    pub header_height: f64,
}

pub enum NodeKind {
    Directory,
    File(SourceFile),
}

pub struct SourceFile {
    pub text: Option<Arc<str>>,
    pub line_spans: Vec<LineSpan>,
    pub analysis: Option<FileAnalysis>,
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
        let mut repository = Self::from_files(root, files);
        repository.edges = resolve_edges(&repository.nodes);
        apply_layout(&mut repository.nodes, char_advance);
        Ok(repository)
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
        self.node(self.root_id()).rect
    }

    pub fn find_by_path(&self, path: &Path) -> Option<NodeId> {
        self.ids_by_path.get(path).copied()
    }

    pub fn directory_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::Directory))
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

    fn find_child_at(&self, id: NodeId, point: Point) -> Option<NodeId> {
        self.node(id)
            .children
            .iter()
            .copied()
            .find(|child| self.node(*child).rect.contains(point))
    }

    fn from_files(root: &Path, files: Vec<LoadedFile>) -> Self {
        let mut repository = Self {
            root: root.to_path_buf(),
            nodes: vec![create_directory_node(root_name(root), PathBuf::new(), None)],
            edges: Vec::new(),
            ids_by_path: HashMap::new(),
        };
        repository.ids_by_path.insert(PathBuf::new(), NodeId(0));
        for file in files {
            repository.insert_file(file);
        }
        repository.sort_children();
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
            rect: Rect::ZERO,
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
}

impl Node {
    pub fn source_file(&self) -> Option<&SourceFile> {
        match &self.kind {
            NodeKind::File(source_file) => Some(source_file),
            NodeKind::Directory => None,
        }
    }

    pub fn get_is_directory(&self) -> bool {
        matches!(self.kind, NodeKind::Directory)
    }
}

fn create_directory_node(name: String, path: PathBuf, parent: Option<NodeId>) -> Node {
    Node {
        name,
        path,
        parent,
        children: Vec::new(),
        kind: NodeKind::Directory,
        rect: Rect::ZERO,
        header_height: 0.0,
    }
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
