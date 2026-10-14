use crate::repository::{Edge, Node, NodeId};
use std::collections::{BTreeMap, HashMap, HashSet};

const MAX_DEFINITION_AMBIGUITY: usize = 3;
const MIN_NAME_LENGTH: usize = 3;

pub fn resolve_edges(nodes: &[Node]) -> Vec<Edge> {
    let definitions = index_definitions(nodes);
    nodes
        .iter()
        .enumerate()
        .flat_map(|(index, node)| resolve_file_edges(NodeId(index), node, &definitions))
        .collect()
}

fn index_definitions(nodes: &[Node]) -> HashMap<&str, Vec<NodeId>> {
    let mut definitions: HashMap<&str, Vec<NodeId>> = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(analysis) = node.source_file().and_then(|file| file.analysis.as_ref()) else {
            continue;
        };
        let names: HashSet<&str> = analysis
            .definitions
            .iter()
            .map(|symbol| symbol.name.as_str())
            .filter(|name| name.len() >= MIN_NAME_LENGTH)
            .collect();
        for name in names {
            definitions.entry(name).or_default().push(NodeId(index));
        }
    }
    definitions
}

fn resolve_file_edges(
    id: NodeId,
    node: &Node,
    definitions: &HashMap<&str, Vec<NodeId>>,
) -> Vec<Edge> {
    let Some(analysis) = node.source_file().and_then(|file| file.analysis.as_ref()) else {
        return Vec::new();
    };
    let mut weights: BTreeMap<NodeId, u32> = BTreeMap::new();
    for reference in &analysis.references {
        let Some(targets) = definitions.get(reference.name.as_str()) else {
            continue;
        };
        if targets.len() > MAX_DEFINITION_AMBIGUITY {
            continue;
        }
        for target in targets.iter().filter(|target| **target != id) {
            *weights.entry(*target).or_default() += 1;
        }
    }
    weights
        .into_iter()
        .map(|(to, weight)| Edge {
            from: id,
            to,
            weight,
        })
        .collect()
}
