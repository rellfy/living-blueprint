use crate::repository::layout::GRID_UNIT;
use crate::repository::{NodeId, Repository};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use vello::kurbo::{Point, Rect, Vec2};

const MARGIN_UNITS: f64 = 2.0;
const NO_OWNER: u32 = u32::MAX;
const BEND_COST: u32 = 4;
const HEURISTIC_WEIGHT: u32 = 2;
const MAX_EXPANSIONS: usize = 1_000_000;
const ENDPOINT_HEIGHT_CAP: f64 = 400.0;
const DIRECTIONS: [Direction; 4] = [
    Direction::Right,
    Direction::Left,
    Direction::Down,
    Direction::Up,
];

#[derive(Default)]
pub struct EdgeRoutes {
    layout_version: u64,
    grid: Option<ObstacleGrid>,
    routes: HashMap<(NodeId, NodeId), Vec<Point>>,
}

struct ObstacleGrid {
    origin: Point,
    cell_columns: usize,
    cell_rows: usize,
    owners: Vec<u32>,
    lines_x: Vec<usize>,
    lines_y: Vec<usize>,
    walls: Vec<Rect>,
    search: SearchBuffers,
}

#[derive(Default)]
struct SearchBuffers {
    costs: Vec<u32>,
    parents: Vec<u32>,
    stamps: Vec<u32>,
    stamp: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Direction {
    Right,
    Left,
    Down,
    Up,
}

#[derive(Clone, Copy)]
struct State {
    column: usize,
    row: usize,
    direction: Direction,
}

impl EdgeRoutes {
    pub fn route(&mut self, repository: &Repository, from: NodeId, to: NodeId) -> Option<&[Point]> {
        if self.layout_version != repository.layout_version() || self.grid.is_none() {
            self.layout_version = repository.layout_version();
            self.grid = Some(ObstacleGrid::build(repository));
            self.routes.clear();
        }
        if !self.routes.contains_key(&(from, to)) {
            let (start, end, direction) = compute_endpoints(repository, from, to)?;
            let grid = self.grid.as_mut()?;
            let route = grid
                .find_route(start, end, direction)
                .unwrap_or_else(|| create_fallback_route(start, end));
            self.routes.insert((from, to), route);
        }
        self.routes.get(&(from, to)).map(Vec::as_slice)
    }
}

impl ObstacleGrid {
    fn build(repository: &Repository) -> Self {
        let margin = MARGIN_UNITS * GRID_UNIT;
        let bounds = repository.bounds().inflate(margin, margin);
        let cell_columns = (bounds.width() / GRID_UNIT).round() as usize;
        let cell_rows = (bounds.height() / GRID_UNIT).round() as usize;
        let mut grid = Self {
            origin: bounds.origin(),
            cell_columns,
            cell_rows,
            owners: vec![NO_OWNER; cell_columns * cell_rows],
            lines_x: vec![0, cell_columns],
            lines_y: vec![0, cell_rows],
            walls: Vec::new(),
            search: SearchBuffers::default(),
        };
        for (index, node) in repository.nodes().iter().enumerate() {
            let is_shown = repository.visible_ancestor(NodeId(index)) == NodeId(index);
            match (is_shown, node.get_is_expanded_directory()) {
                (false, _) => {}
                (true, true) => grid.walls.push(grid.to_grid_rect(node.target_rect)),
                (true, false) => grid.occupy(node.target_rect, index as u32),
            }
        }
        for lines in [&mut grid.lines_x, &mut grid.lines_y] {
            lines.sort_unstable();
            lines.dedup();
        }
        grid
    }

    fn occupy(&mut self, rect: Rect, owner: u32) {
        let clearance = rect.inflate(GRID_UNIT, GRID_UNIT);
        let (column_start, row_start) = self.to_cell(Point::new(clearance.x0, clearance.y0));
        let (column_end, row_end) = self.to_cell(Point::new(clearance.x1, clearance.y1));
        for row in row_start..row_end.min(self.cell_rows) {
            for column in column_start..column_end.min(self.cell_columns) {
                self.owners[row * self.cell_columns + column] = owner;
            }
        }
        let (_, anchor_row) = self.to_cell(Point::new(rect.x0, compute_anchor_y(rect)));
        self.lines_x.extend([column_start, column_end]);
        self.lines_y.extend([row_start, anchor_row, row_end]);
    }

    fn find_route(&mut self, start: Point, end: Point, direction: Direction) -> Option<Vec<Point>> {
        let stub = direction.offset() * GRID_UNIT;
        let (departure, arrival) = (start + stub, end - stub);
        let (start_column, start_row) = self.to_grid_point(departure)?;
        let goal = self.to_grid_point(arrival)?;
        let backtrack = direction.opposite();
        self.search
            .begin(self.lines_x.len() * self.lines_y.len() * DIRECTIONS.len());
        let mut queue = BinaryHeap::new();
        let state = State {
            column: start_column,
            row: start_row,
            direction,
        };
        let index = self.encode(state);
        self.search.relax(index, 0, u32::MAX);
        queue.push((Reverse(self.estimate(state, goal)), 0, index));
        let mut expansions = 0;
        while let Some((_, cost, index)) = queue.pop() {
            let state = self.decode(index);
            if (state.column, state.row) == goal && state.direction != backtrack {
                let corners = self.trace(index, departure, arrival);
                let route = [vec![start], corners, vec![end]].concat();
                return Some(remove_collinear(route));
            }
            if self.search.cost(index) < cost {
                continue;
            }
            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                return None;
            }
            for direction in DIRECTIONS {
                let Some(next) = self.step(state, direction) else {
                    continue;
                };
                let bend = match direction == state.direction {
                    true => 0,
                    false => BEND_COST,
                };
                let next_cost = cost + self.measure_step(state, next) + bend;
                let next_index = self.encode(next);
                if self.search.cost(next_index) <= next_cost {
                    continue;
                }
                self.search.relax(next_index, next_cost, index);
                queue.push((
                    Reverse(next_cost + self.estimate(next, goal)),
                    next_cost,
                    next_index,
                ));
            }
        }
        None
    }

    fn encode(&self, state: State) -> u32 {
        let point = state.row * self.lines_x.len() + state.column;
        (point * DIRECTIONS.len() + state.direction as usize) as u32
    }

    fn decode(&self, index: u32) -> State {
        let index = index as usize;
        let point = index / DIRECTIONS.len();
        State {
            column: point % self.lines_x.len(),
            row: point / self.lines_x.len(),
            direction: DIRECTIONS[index % DIRECTIONS.len()],
        }
    }

    fn step(&self, state: State, direction: Direction) -> Option<State> {
        if direction == state.direction.opposite() {
            return None;
        }
        let (column, row) = (state.column, state.row);
        let next = match direction {
            Direction::Right => (column + 1, row),
            Direction::Left => (column.checked_sub(1)?, row),
            Direction::Down => (column, row + 1),
            Direction::Up => (column, row.checked_sub(1)?),
        };
        let (Some(next_x), Some(next_y)) = (self.lines_x.get(next.0), self.lines_y.get(next.1))
        else {
            return None;
        };
        let (x, y) = (self.lines_x[column] as isize, self.lines_y[row] as isize);
        let sides = match direction {
            Direction::Right => [(x, y - 1), (x, y)],
            Direction::Left => [(*next_x as isize, y - 1), (*next_x as isize, y)],
            Direction::Down => [(x - 1, y), (x, y)],
            Direction::Up => [(x - 1, *next_y as isize), (x, *next_y as isize)],
        };
        let is_blocked = self.check_is_inside_one_owner(sides)
            || self.check_runs_along_wall((x, y), (*next_x as isize, *next_y as isize));
        (!is_blocked).then_some(State {
            column: next.0,
            row: next.1,
            direction,
        })
    }

    fn check_runs_along_wall(&self, from: (isize, isize), to: (isize, isize)) -> bool {
        let (x0, x1) = (from.0.min(to.0) as f64, from.0.max(to.0) as f64);
        let (y0, y1) = (from.1.min(to.1) as f64, from.1.max(to.1) as f64);
        self.walls.iter().any(|wall| {
            let is_on_horizontal = y0 == y1 && (y0 == wall.y0 || y0 == wall.y1);
            let is_on_vertical = x0 == x1 && (x0 == wall.x0 || x0 == wall.x1);
            match (is_on_horizontal, is_on_vertical) {
                (true, _) => x0 < wall.x1 && x1 > wall.x0,
                (_, true) => y0 < wall.y1 && y1 > wall.y0,
                _ => false,
            }
        })
    }

    fn measure_step(&self, from: State, to: State) -> u32 {
        let width = self.lines_x[from.column].abs_diff(self.lines_x[to.column]);
        let height = self.lines_y[from.row].abs_diff(self.lines_y[to.row]);
        (width + height) as u32
    }

    fn check_is_inside_one_owner(&self, sides: [(isize, isize); 2]) -> bool {
        let [first, second] = sides.map(|cell| self.owner_of(cell));
        first != NO_OWNER && first == second
    }

    fn owner_of(&self, (column, row): (isize, isize)) -> u32 {
        let is_inside = column >= 0
            && row >= 0
            && (column as usize) < self.cell_columns
            && (row as usize) < self.cell_rows;
        match is_inside {
            true => self.owners[row as usize * self.cell_columns + column as usize],
            false => NO_OWNER,
        }
    }

    fn estimate(&self, state: State, goal: (usize, usize)) -> u32 {
        let width = self.lines_x[state.column].abs_diff(self.lines_x[goal.0]);
        let height = self.lines_y[state.row].abs_diff(self.lines_y[goal.1]);
        let is_diagonal = width > 0 && height > 0;
        let minimum = match is_diagonal {
            true => width + height + BEND_COST as usize,
            false => width + height,
        };
        minimum as u32 * HEURISTIC_WEIGHT
    }

    fn trace(&self, goal: u32, start: Point, end: Point) -> Vec<Point> {
        let mut corners = vec![end];
        let mut current = goal;
        while let Some(parent) = self.search.parent(current) {
            let (parent_state, current_state) = (self.decode(parent), self.decode(current));
            if parent_state.direction != current_state.direction {
                corners.push(self.to_world(parent_state.column, parent_state.row));
            }
            current = parent;
        }
        corners.push(start);
        corners.reverse();
        corners.dedup();
        corners
    }

    fn to_cell(&self, point: Point) -> (usize, usize) {
        let offset = point - self.origin;
        (
            (offset.x / GRID_UNIT).round().max(0.0) as usize,
            (offset.y / GRID_UNIT).round().max(0.0) as usize,
        )
    }

    fn to_grid_rect(&self, rect: Rect) -> Rect {
        let (x0, y0) = self.to_cell(Point::new(rect.x0, rect.y0));
        let (x1, y1) = self.to_cell(Point::new(rect.x1, rect.y1));
        Rect::new(x0 as f64, y0 as f64, x1 as f64, y1 as f64)
    }

    fn to_grid_point(&self, point: Point) -> Option<(usize, usize)> {
        let (column, row) = self.to_cell(point);
        let column = self.lines_x.binary_search(&column).ok()?;
        let row = self.lines_y.binary_search(&row).ok()?;
        Some((column, row))
    }

    fn to_world(&self, column: usize, row: usize) -> Point {
        self.origin
            + (
                self.lines_x[column] as f64 * GRID_UNIT,
                self.lines_y[row] as f64 * GRID_UNIT,
            )
    }
}

impl Direction {
    fn opposite(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Left => Self::Right,
            Self::Down => Self::Up,
            Self::Up => Self::Down,
        }
    }

    fn offset(self) -> Vec2 {
        match self {
            Self::Right => Vec2::new(1.0, 0.0),
            Self::Left => Vec2::new(-1.0, 0.0),
            Self::Down => Vec2::new(0.0, 1.0),
            Self::Up => Vec2::new(0.0, -1.0),
        }
    }
}

impl SearchBuffers {
    fn begin(&mut self, state_count: usize) {
        if self.stamps.len() != state_count {
            self.costs = vec![0; state_count];
            self.parents = vec![u32::MAX; state_count];
            self.stamps = vec![0; state_count];
            self.stamp = 0;
        }
        self.stamp += 1;
    }

    fn cost(&self, index: u32) -> u32 {
        match self.stamps[index as usize] == self.stamp {
            true => self.costs[index as usize],
            false => u32::MAX,
        }
    }

    fn parent(&self, index: u32) -> Option<u32> {
        let parent = self.parents[index as usize];
        (self.stamps[index as usize] == self.stamp && parent != u32::MAX).then_some(parent)
    }

    fn relax(&mut self, index: u32, cost: u32, parent: u32) {
        self.stamps[index as usize] = self.stamp;
        self.costs[index as usize] = cost;
        self.parents[index as usize] = parent;
    }
}

fn compute_endpoints(
    repository: &Repository,
    from: NodeId,
    to: NodeId,
) -> Option<(Point, Point, Direction)> {
    let from = repository.node(from).target_rect;
    let to = repository.node(to).target_rect;
    if from.is_zero_area() || to.is_zero_area() {
        return None;
    }
    let is_target_right = to.center().x >= from.center().x;
    let (start_x, end_x, direction) = match is_target_right {
        true => (from.x1, to.x0, Direction::Right),
        false => (from.x0, to.x1, Direction::Left),
    };
    Some((
        Point::new(start_x, compute_anchor_y(from)),
        Point::new(end_x, compute_anchor_y(to)),
        direction,
    ))
}

fn compute_anchor_y(rect: Rect) -> f64 {
    snap_to_grid(rect.y0 + rect.height().min(ENDPOINT_HEIGHT_CAP) * 0.5)
}

fn remove_collinear(points: Vec<Point>) -> Vec<Point> {
    let mut route: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if route.last() == Some(&point) {
            continue;
        }
        let is_collinear = match route.as_slice() {
            [.., before, middle] => {
                (before.x == middle.x && middle.x == point.x)
                    || (before.y == middle.y && middle.y == point.y)
            }
            _ => false,
        };
        if is_collinear {
            route.pop();
        }
        route.push(point);
    }
    route
}

fn create_fallback_route(start: Point, end: Point) -> Vec<Point> {
    let middle_x = snap_to_grid((start.x + end.x) * 0.5);
    vec![
        start,
        Point::new(middle_x, start.y),
        Point::new(middle_x, end.y),
        end,
    ]
}

fn snap_to_grid(value: f64) -> f64 {
    (value / GRID_UNIT).round() * GRID_UNIT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::Detail;
    use crate::repository::layout::CODE_FONT_SIZE;
    use std::path::Path;

    fn assert_routes_avoid_nodes(repository: &Repository) {
        let mut routes = EdgeRoutes::default();
        for edge in repository.edges() {
            let from = repository.visible_ancestor(edge.from);
            let to = repository.visible_ancestor(edge.to);
            if from == to {
                continue;
            }
            let route = routes.route(repository, from, to).unwrap().to_vec();
            for segment in route.windows(2) {
                let middle = segment[0].midpoint(segment[1]);
                let is_crossing = repository.nodes().iter().enumerate().any(|(index, node)| {
                    let rect = node.target_rect;
                    let is_shown = repository.visible_ancestor(NodeId(index)) == NodeId(index);
                    let is_inside = !node.get_is_expanded_directory()
                        && rect.inflate(-1.0, -1.0).contains(middle);
                    is_shown && (is_inside || check_runs_along(segment, rect))
                });
                assert!(
                    !is_crossing,
                    "{:?} -> {:?} {route:?} {:?} {:?}",
                    repository.node(from).path,
                    repository.node(to).path,
                    repository.node(from).target_rect,
                    repository.node(to).target_rect
                );
            }
        }
    }

    fn check_runs_along(segment: &[Point], rect: Rect) -> bool {
        let (start, end) = (segment[0], segment[1]);
        let (x0, x1) = (start.x.min(end.x), start.x.max(end.x));
        let (y0, y1) = (start.y.min(end.y), start.y.max(end.y));
        let is_on_horizontal = y0 == y1 && (y0 == rect.y0 || y0 == rect.y1);
        let is_on_vertical = x0 == x1 && (x0 == rect.x0 || x0 == rect.x1);
        (is_on_horizontal && x0 < rect.x1 && x1 > rect.x0)
            || (is_on_vertical && y0 < rect.y1 && y1 > rect.y0)
    }

    #[test]
    fn routes_avoid_nodes_at_every_level() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut repository = Repository::load(root, CODE_FONT_SIZE * 0.6).unwrap();
        repository.set_all_details(Detail::Expanded);
        let root_id = repository.root_id();
        loop {
            assert_routes_avoid_nodes(&repository);
            if !repository.compress(root_id) {
                break;
            }
        }
    }
}
