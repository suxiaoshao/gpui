//! Single-parent history graph. Inspired by Zed's active-lane allocation;
//! completed lanes are reused and edges are stored once, not once per row crossed.
use super::HistoryRow;
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub from: usize,
    pub to: usize,
    pub lane: usize,
    pub parent_lane: usize,
    pub indirect: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::history::{History, HistoryDetail};

    fn project(parents: &[Option<usize>]) -> (Vec<HistoryRow>, HistoryGraph) {
        let entries = parents
            .iter()
            .enumerate()
            .map(|(id, parent)| {
                serde_json::json!({
                    "type":"message", "id":id.to_string(), "parentId":parent.map(|p|p.to_string()),
                    "timestamp":"same time", "message":{"role":"user","content":"message"}
                })
            })
            .collect::<Vec<_>>();
        let mut history = History::default();
        history.replace(
            serde_json::from_value(serde_json::json!({"entries":entries,"leafId":null})).unwrap(),
        );
        let rows = history.tree_rows(HistoryDetail::Detailed);
        let graph = HistoryGraph::new(&rows);
        (rows, graph)
    }

    #[test]
    fn long_chain_and_completed_branches_reuse_lanes() {
        let parents = (0..10_000)
            .map(|i: usize| i.checked_sub(1))
            .collect::<Vec<_>>();
        let (_, graph) = project(&parents);
        assert_eq!(graph.lanes.len(), 1);
        assert_eq!(graph.lanes[0].len(), 9_999);
        // Separate fork groups complete before the next group begins.
        let (_, graph) = project(&[None, Some(0), Some(0), None, Some(3), Some(3)]);
        assert_eq!(graph.lanes.len(), 2);
    }

    #[test]
    fn thousands_of_siblings_store_each_edge_once_and_query_offscreen_connections() {
        let mut parents = vec![None];
        parents.extend(std::iter::repeat_n(Some(0), 2_000));
        let (_, graph) = project(&parents);
        assert_eq!(graph.lanes.len(), 2_000);
        assert_eq!(graph.lanes.iter().map(Vec::len).sum::<usize>(), 2_000);
        // A viewport in the middle of a long branch still sees passing lines.
        assert_eq!(graph.row_edges(500, 300, 5).len(), 5);
        assert_eq!(graph.row_edges(0, 0, 5).len(), 2_000);
        assert_eq!(graph.row_edges(500, 1_990, 5).len(), 0);
    }

    #[test]
    fn nested_forks_preserve_endpoints_and_never_run_through_unrelated_nodes() {
        let mut parents = vec![None];
        for i in 1..160 {
            parents.push(Some((i * 37 + 11) % i));
        }
        // Keep parents before children, including the first entry after the root.
        parents[1] = Some(0);
        let (rows, graph) = project(&parents);
        for edges in &graph.lanes {
            assert!(edges.windows(2).all(|pair| pair[0].to <= pair[1].from));
            for edge in edges {
                assert_eq!(
                    rows[edge.to].parent.as_deref(),
                    Some(rows[edge.from].id.as_str())
                );
                assert!(edge.from < edge.to);
                assert_eq!(graph.nodes[edge.to], edge.lane);
                assert_eq!(graph.nodes[edge.from], edge.parent_lane);
                for row in edge.from + 1..edge.to {
                    assert_ne!(graph.nodes[row], edge.lane);
                }
                for row in edge.from..=edge.to {
                    assert!(
                        graph
                            .row_edges(row, edge.lane, 1)
                            .iter()
                            .any(|found| std::ptr::eq(*found, edge))
                    );
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryGraph {
    pub nodes: Vec<usize>,
    pub lanes: Vec<Vec<Edge>>,
    junctions: Vec<Vec<(usize, usize)>>,
}

impl HistoryGraph {
    pub fn new(rows: &[HistoryRow]) -> Self {
        let mut graph = Self {
            nodes: vec![0; rows.len()],
            junctions: vec![vec![]; rows.len()],
            ..Self::default()
        };
        let mut pending: HashMap<&str, Vec<(usize, usize, bool)>> = HashMap::new();
        let mut free = BTreeSet::new();
        // Allocate from leaves toward roots to reuse completed lanes. Store row
        // coordinates in chronological display order, with each parent on top.
        for (row, entry) in rows.iter().enumerate().rev() {
            let incoming = pending.remove(entry.id.as_str()).unwrap_or_default();
            let lane = incoming
                .iter()
                .map(|(lane, _, _)| *lane)
                .min()
                .unwrap_or_else(|| {
                    free.pop_first().unwrap_or_else(|| {
                        graph.lanes.push(vec![]);
                        graph.lanes.len() - 1
                    })
                });
            for (source_lane, child, indirect) in incoming {
                let edges = &mut graph.lanes[source_lane];
                edges.push(Edge {
                    from: row,
                    to: child,
                    lane: source_lane,
                    parent_lane: lane,
                    indirect,
                });
                free.insert(source_lane);
            }
            free.remove(&lane);
            graph.nodes[row] = lane;
            if let Some(parent) = entry.parent.as_deref() {
                pending
                    .entry(parent)
                    .or_default()
                    .push((lane, row, entry.indirect_parent));
            } else {
                free.insert(lane);
            }
        }
        // Reverse each lane's edge index for ascending viewport queries.
        for (lane, edges) in graph.lanes.iter_mut().enumerate() {
            edges.reverse();
            for (index, edge) in edges.iter().enumerate() {
                graph.junctions[edge.from].push((lane, index));
            }
        }
        graph
    }

    /// A clipped row needs only edges in visible lanes and branches leaving a
    /// visible parent. Binary search avoids scanning the full history while scrolling.
    pub fn row_edges(&self, row: usize, start: usize, count: usize) -> Vec<&Edge> {
        let end = (start + count).min(self.lanes.len());
        let mut result = Vec::new();
        for edges in &self.lanes[start.min(end)..end] {
            let first = edges.partition_point(|edge| edge.to < row);
            result.extend(edges[first..].iter().take_while(|edge| edge.from <= row));
        }
        if let Some(junctions) = self.junctions.get(row) {
            for &(lane, edge) in junctions {
                let edge = &self.lanes[lane][edge];
                if !(start..end).contains(&lane) && (start..end).contains(&edge.parent_lane) {
                    result.push(edge);
                }
            }
        }
        result
    }
}
