use crate::repository::{Detail, NodeId, Repository};

impl Repository {
    pub fn compress(&mut self, id: NodeId) -> bool {
        let is_changed = self.compress_node(id);
        if is_changed {
            self.relayout();
        }
        is_changed
    }

    pub fn expand(&mut self, id: NodeId) -> bool {
        let is_changed = self.expand_node(id);
        if is_changed {
            self.relayout();
        }
        is_changed
    }

    fn compress_node(&mut self, id: NodeId) -> bool {
        let node = self.node(id);
        let is_minimal = node.detail == Detail::Collapsed;
        match (node.get_is_directory(), is_minimal) {
            (true, false) => self.compress_directory(id),
            (false, false) => self.step_detail(id, -1),
            (_, true) => node
                .parent
                .is_some_and(|parent| self.compress_directory(parent)),
        }
    }

    fn expand_node(&mut self, id: NodeId) -> bool {
        match self.node(id).get_is_directory() {
            true => self.expand_directory(id),
            false => self.step_detail(id, 1),
        }
    }

    fn compress_directory(&mut self, id: NodeId) -> bool {
        if self.node(id).detail == Detail::Collapsed {
            return self.compress_node(id);
        }
        let descendants = self.collect_visible_descendants(id);
        let files = self.filter_files(&descendants);
        let most_detailed = files
            .iter()
            .map(|(file, _)| self.node(*file).detail)
            .filter(|detail| *detail > Detail::Collapsed)
            .max();
        if let Some(detail) = most_detailed {
            self.step_files_at(&files, detail, -1);
            return true;
        }
        let deepest_expanded = descendants
            .iter()
            .filter(|(node, _)| self.node(*node).get_is_expanded_directory())
            .map(|(_, depth)| *depth)
            .max();
        match deepest_expanded {
            Some(depth) => self.set_directories_at(&descendants, depth, Detail::Collapsed),
            None => self.nodes[id.0].detail = Detail::Collapsed,
        }
        true
    }

    fn expand_directory(&mut self, id: NodeId) -> bool {
        if self.node(id).detail == Detail::Collapsed {
            self.nodes[id.0].detail = Detail::Expanded;
            return true;
        }
        let descendants = self.collect_visible_descendants(id);
        let shallowest_collapsed = descendants
            .iter()
            .filter(|(node, _)| self.node(*node).get_is_collapsed_directory())
            .map(|(_, depth)| *depth)
            .min();
        if let Some(depth) = shallowest_collapsed {
            self.set_directories_at(&descendants, depth, Detail::Expanded);
            return true;
        }
        let files = self.filter_files(&descendants);
        let least_detailed = files
            .iter()
            .map(|(file, _)| self.node(*file).detail)
            .filter(|detail| *detail < Detail::Expanded)
            .min();
        let Some(detail) = least_detailed else {
            return false;
        };
        self.step_files_at(&files, detail, 1);
        true
    }

    fn step_files_at(&mut self, files: &[(NodeId, usize)], detail: Detail, direction: isize) {
        for (file, _) in files {
            if self.node(*file).detail == detail {
                self.step_detail(*file, direction);
            }
        }
    }

    fn set_directories_at(&mut self, nodes: &[(NodeId, usize)], depth: usize, detail: Detail) {
        for (node, node_depth) in nodes {
            if *node_depth == depth && self.node(*node).get_is_directory() {
                self.nodes[node.0].detail = detail;
            }
        }
    }

    fn step_detail(&mut self, id: NodeId, direction: isize) -> bool {
        let node = self.node(id);
        let available = node.available_details();
        let Some(index) = available.iter().position(|detail| *detail == node.detail) else {
            return false;
        };
        let Some(next) = index
            .checked_add_signed(direction)
            .and_then(|next| available.get(next))
        else {
            return false;
        };
        self.nodes[id.0].detail = *next;
        true
    }

    fn collect_visible_descendants(&self, id: NodeId) -> Vec<(NodeId, usize)> {
        let mut descendants = Vec::new();
        let mut stack: Vec<(NodeId, usize)> = self
            .node(id)
            .children
            .iter()
            .map(|child| (*child, 1))
            .collect();
        while let Some((node, depth)) = stack.pop() {
            descendants.push((node, depth));
            if !self.node(node).get_is_expanded_directory() {
                continue;
            }
            stack.extend(
                self.node(node)
                    .children
                    .iter()
                    .map(|child| (*child, depth + 1)),
            );
        }
        descendants
    }

    fn filter_files(&self, nodes: &[(NodeId, usize)]) -> Vec<(NodeId, usize)> {
        nodes
            .iter()
            .copied()
            .filter(|(node, _)| !self.node(*node).get_is_directory())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::repository::layout::CODE_FONT_SIZE;
    use crate::repository::{Detail, Repository};
    use std::path::Path;

    fn load() -> Repository {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        Repository::load(root, CODE_FONT_SIZE * 0.6).unwrap()
    }

    #[test]
    fn file_steps_through_its_details() {
        let mut repository = load();
        let id = repository
            .find_by_path(Path::new("src/resources/camera.rs"))
            .unwrap();
        let expanded_height = repository.node(id).target_rect.height();
        let mut details = vec![repository.node(id).detail];
        while repository.node(id).detail != Detail::Collapsed {
            assert!(repository.compress(id));
            details.push(repository.node(id).detail);
        }
        assert_eq!(
            details,
            vec![
                Detail::Expanded,
                Detail::Outline,
                Detail::Summary,
                Detail::Collapsed
            ]
        );
        assert!(repository.node(id).target_rect.height() < expanded_height);
        while repository.expand(id) {}
        assert_eq!(repository.node(id).detail, Detail::Expanded);
    }

    #[test]
    fn compressing_a_collapsed_file_compresses_its_directory() {
        let mut repository = load();
        let id = repository
            .find_by_path(Path::new("src/resources/camera.rs"))
            .unwrap();
        let parent = repository.node(id).parent.unwrap();
        while repository.node(parent).detail != Detail::Collapsed {
            assert!(repository.compress(id));
        }
        repository.animate(10.0);
        let parent_rect = repository.node(parent).target_rect;
        assert_eq!(repository.node(id).target_rect, parent_rect);
        assert_eq!(repository.visible_ancestor(id), parent);
        assert_eq!(repository.hit_test(parent_rect.center()), Some(parent));
        assert!(repository.expand(parent));
        assert_eq!(repository.node(id).detail, Detail::Collapsed);
    }

    #[test]
    fn root_compresses_fully_then_stops() {
        let mut repository = load();
        let root = repository.root_id();
        let mut steps = 0;
        while repository.compress(root) {
            steps += 1;
            assert!(steps < 100);
        }
        assert_eq!(repository.node(root).detail, Detail::Collapsed);
        while repository.expand(root) {}
        let all_expanded = repository
            .nodes()
            .iter()
            .all(|node| node.detail == Detail::Expanded);
        assert!(all_expanded);
    }
}
