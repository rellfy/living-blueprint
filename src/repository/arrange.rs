use crate::arrangement::Arrangement;
use crate::repository::layout::content_origin;
use crate::repository::{NodeId, Repository};
use vello::kurbo::Point;

impl Repository {
    pub fn arrangement(&self) -> &Arrangement {
        &self.arrangement
    }

    pub fn set_arrangement(&mut self, arrangement: Arrangement) {
        self.arrangement = arrangement;
        self.relayout();
        self.snap_rects();
    }

    pub fn drag_to(&mut self, id: NodeId, origin: Point) {
        let Some(parent) = self.node(id).parent else {
            return;
        };
        self.seed_arrangement(parent);
        let offset = origin - content_origin(self.node(parent));
        let path = self.node(id).path.clone();
        self.arrangement.set_position(&path, offset);
        self.dragged_path = Some(path);
        self.relayout();
    }

    pub fn drop_dragged(&mut self) {
        let Some(path) = self.dragged_path.take() else {
            return;
        };
        let Some(parent) = self.find_by_path(&path).and_then(|id| self.node(id).parent) else {
            return;
        };
        self.store_child_positions(parent);
        self.prune_arrangement();
        self.relayout();
    }

    fn prune_arrangement(&mut self) {
        let ids_by_path = &self.ids_by_path;
        self.arrangement
            .retain_paths(|path| ids_by_path.contains_key(path));
    }

    fn seed_arrangement(&mut self, parent: NodeId) {
        let children = self.node(parent).children.iter();
        let paths = children.map(|child| self.node(*child).path.as_path());
        if self.arrangement.check_is_arranged(paths) {
            return;
        }
        self.store_child_positions(parent);
    }

    fn store_child_positions(&mut self, parent: NodeId) {
        let origin = content_origin(self.node(parent));
        for child in self.node(parent).children.clone() {
            let node = self.node(child);
            let offset = node.target_rect.origin() - origin;
            let path = node.path.clone();
            self.arrangement.set_position(&path, offset);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::repository::layout::CODE_FONT_SIZE;
    use crate::repository::{Detail, NodeId, Repository};
    use std::path::Path;
    use vello::kurbo::Vec2;

    fn load() -> Repository {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut repository = Repository::load(root, CODE_FONT_SIZE * 0.6).unwrap();
        repository.set_all_details(Detail::Expanded);
        repository
    }

    fn assert_children_apart(repository: &Repository, parent: NodeId) {
        let children = &repository.node(parent).children;
        for (index, first) in children.iter().enumerate() {
            for second in &children[index + 1..] {
                let overlap = repository
                    .node(*first)
                    .target_rect
                    .intersect(repository.node(*second).target_rect);
                assert!(overlap.area() <= 0.0);
            }
        }
    }

    #[test]
    fn dragging_moves_a_node_and_pushes_siblings_apart() {
        let mut repository = load();
        let id = repository.find_by_path(Path::new("src/main.rs")).unwrap();
        let parent = repository.node(id).parent.unwrap();
        let first_sibling = repository.node(parent).children[0];
        let target = repository.node(first_sibling).target_rect.origin() + Vec2::new(50.0, 0.0);
        repository.drag_to(id, target);
        assert_children_apart(&repository, parent);
        repository.drop_dragged();
        assert_children_apart(&repository, parent);
        assert!(
            repository
                .arrangement()
                .positions
                .contains_key(Path::new("src/main.rs"))
        );
    }

    #[test]
    fn dropped_positions_stay_after_relayout() {
        let mut repository = load();
        let id = repository.find_by_path(Path::new("src/main.rs")).unwrap();
        let parent = repository.node(id).parent.unwrap();
        let far = repository.node(parent).target_rect.origin() + Vec2::new(5000.0, 5000.0);
        repository.drag_to(id, far);
        repository.drop_dragged();
        let dropped = repository.node(id).target_rect;
        repository.relayout();
        assert_eq!(repository.node(id).target_rect, dropped);
        assert!(
            repository
                .node(parent)
                .target_rect
                .contains(dropped.center())
        );
    }

    #[test]
    fn arrangement_survives_reload() {
        let mut repository = load();
        let id = repository.find_by_path(Path::new("src/main.rs")).unwrap();
        let parent = repository.node(id).parent.unwrap();
        let far = repository.node(parent).target_rect.origin() + Vec2::new(5000.0, 5000.0);
        repository.drag_to(id, far);
        repository.drop_dragged();
        let mut reloaded = load();
        reloaded.inherit(&repository);
        assert_eq!(reloaded.arrangement(), repository.arrangement());
    }
}
