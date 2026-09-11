//! A reversible projection of detailed history. No synthetic session entries.
#[cfg(test)]
mod tests;
mod tidy;
use super::{HistoryKind, HistoryRow};
use std::collections::{HashMap, HashSet};

pub(crate) const X_GAP: f32 = 80.;
pub(crate) const Y_GAP: f32 = 64.;

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub row: usize,
    pub x: f32,
    pub y: f32,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}
#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub from: usize,
    pub to: usize,
    pub hidden: Vec<usize>,
}
#[derive(Clone, Debug)]
pub(crate) struct Segment {
    pub from: usize,
    pub to: usize,
    pub inner: Vec<usize>,
}
#[derive(Default)]
pub(crate) struct Tree {
    pub rows: Vec<HistoryRow>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub segments: Vec<Segment>,
    pub selected: Option<String>,
    pub preview: Option<String>,
    pub current: Option<String>,
    pub highlighted: HashSet<usize>,
    parents: Vec<Option<usize>>,
    index: HashMap<String, usize>,
    node_index: HashMap<usize, usize>,
    expanded: HashSet<String>,
    levels: Vec<Vec<usize>>,
    edge_levels: Vec<Vec<usize>>,
    segment_ends: Vec<Vec<usize>>,
}
impl Tree {
    pub fn replace(&mut self, rows: Vec<HistoryRow>, preview: Option<String>) {
        self.rows = rows;
        self.preview = preview;
        self.index = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.clone(), i))
            .collect();
        self.expanded.retain(|id| self.index.contains_key(id));
        self.current = self.rows.iter().find(|r| r.current).map(|r| r.id.clone());
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !self.index.contains_key(id))
        {
            self.selected = self
                .preview
                .clone()
                .or_else(|| self.current.clone())
                .or_else(|| self.rows.last().map(|r| r.id.clone()));
        }
        self.parents = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.parent
                    .as_ref()
                    .and_then(|p| self.index.get(p))
                    .copied()
                    .filter(|&p| p < i)
            })
            .collect();
        let mut children = vec![vec![]; self.rows.len()];
        for (i, p) in self.parents.iter().enumerate() {
            if let Some(p) = p {
                children[*p].push(i);
            }
        }
        let anchors: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                self.parents[i].is_none()
                    || children[i].len() != 1
                    || r.label.is_some()
                    || matches!(
                        r.kind,
                        HistoryKind::User
                            | HistoryKind::Assistant
                            | HistoryKind::Compaction
                            | HistoryKind::BranchSummary
                    )
            })
            .collect();
        self.segments.clear();
        for (from, branches) in children.iter().enumerate().filter(|(i, _)| anchors[*i]) {
            for &child in branches {
                let mut to = child;
                let mut inner = vec![];
                while !anchors[to] {
                    inner.push(to);
                    to = children[to][0];
                }
                self.segments.push(Segment { from, to, inner });
            }
        }
        self.segment_ends = vec![vec![]; self.rows.len()];
        for (i, segment) in self.segments.iter().enumerate() {
            if segment.inner.len() >= 3 {
                self.segment_ends[segment.from].push(i);
                self.segment_ends[segment.to].push(i);
            }
        }
        self.rebuild();
    }
    pub fn segments_at(&self, row: usize) -> &[usize] {
        self.segment_ends
            .get(row)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
    fn pinned(&self, row: usize) -> bool {
        let id = &self.rows[row].id;
        [
            self.selected.as_ref(),
            self.preview.as_ref(),
            self.current.as_ref(),
        ]
        .contains(&Some(id))
    }
    pub fn select(&mut self, id: String) -> bool {
        let Some(&row) = self.index.get(&id) else {
            return false;
        };
        self.selected = Some(id);
        let hidden = !self.node_index.contains_key(&row);
        if hidden {
            // External navigation reveals the entire containing collapsed edge.
            if let Some(edge) = self.edges.iter().find(|e| e.hidden.contains(&row)) {
                self.expanded
                    .extend(edge.hidden.iter().map(|&r| self.rows[r].id.clone()));
            }
            self.rebuild();
        } else {
            self.highlight();
        }
        hidden
    }
    pub fn set_preview(&mut self, preview: Option<String>) {
        if self.preview != preview {
            self.preview = preview;
            // Preserve what is currently visible when a preview pin moves.
            self.expanded
                .extend(self.nodes.iter().map(|n| self.rows[n.row].id.clone()));
            self.rebuild();
        }
    }
    pub fn selected_node(&self) -> Option<usize> {
        self.selected.as_ref().and_then(|id| self.node_for(id))
    }
    pub fn node_for(&self, id: &str) -> Option<usize> {
        self.index
            .get(id)
            .and_then(|r| self.node_index.get(r))
            .copied()
    }
    pub fn expand(&mut self, edge: usize) {
        if let Some(edge) = self.edges.get(edge) {
            self.expanded
                .extend(edge.hidden.iter().map(|&r| self.rows[r].id.clone()));
            self.rebuild();
        }
    }
    pub fn can_collapse(&self, segment: usize) -> bool {
        self.segments.get(segment).is_some_and(|s| {
            s.inner.len() >= 3
                && s.inner
                    .iter()
                    .any(|&r| self.expanded.contains(&self.rows[r].id))
                && !s.inner.iter().any(|&r| self.pinned(r))
        })
    }
    pub fn collapse(&mut self, segment: usize) {
        if self.can_collapse(segment) {
            for &r in &self.segments[segment].inner {
                self.expanded.remove(&self.rows[r].id);
            }
            self.rebuild();
        }
    }
    fn rebuild(&mut self) {
        let mut visible = vec![true; self.rows.len()];
        for segment in &self.segments {
            let mut start = 0;
            for end in 0..=segment.inner.len() {
                if end == segment.inner.len() || {
                    let r = segment.inner[end];
                    self.pinned(r) || self.expanded.contains(&self.rows[r].id)
                } {
                    if end - start >= 3 {
                        for &r in &segment.inner[start..end] {
                            visible[r] = false;
                        }
                    }
                    start = end + 1;
                }
            }
        }
        self.nodes.clear();
        self.edges.clear();
        self.node_index.clear();
        let mut nearest = vec![None; self.rows.len()];
        for (row, &show) in visible.iter().enumerate() {
            let parent = self.parents[row].and_then(|p| nearest[p]);
            if !show {
                nearest[row] = parent;
                continue;
            }
            let i = self.nodes.len();
            nearest[row] = Some(i);
            self.node_index.insert(row, i);
            self.nodes.push(Node {
                row,
                x: 0.,
                y: 0.,
                parent,
                children: vec![],
            });
            if let Some(from) = parent {
                self.nodes[from].children.push(i);
                let mut hidden = vec![];
                let mut cursor = self.parents[row];
                while let Some(p) = cursor.filter(|&p| !visible[p]) {
                    hidden.push(p);
                    cursor = self.parents[p];
                }
                hidden.reverse();
                self.edges.push(Edge {
                    from,
                    to: i,
                    hidden,
                });
            }
        }
        let positions = tidy::layout(&self.nodes.iter().map(|n| n.parent).collect::<Vec<_>>());
        self.levels.clear();
        self.edge_levels.clear();
        for (i, (x, depth)) in positions.into_iter().enumerate() {
            self.nodes[i].x = x * X_GAP;
            self.nodes[i].y = depth as f32 * Y_GAP;
            self.levels
                .resize_with(self.levels.len().max(depth + 1), Vec::new);
            self.levels[depth].push(i);
        }
        for level in &mut self.levels {
            level.sort_by(|&a, &b| self.nodes[a].x.total_cmp(&self.nodes[b].x));
        }
        self.edge_levels.resize_with(self.levels.len(), Vec::new);
        for (i, e) in self.edges.iter().enumerate() {
            self.edge_levels[(self.nodes[e.from].y / Y_GAP).round() as usize].push(i);
        }
        self.highlight();
    }
    fn highlight(&mut self) {
        self.highlighted.clear();
        let mut next = self.selected_node();
        while let Some(i) = next {
            self.highlighted.insert(i);
            next = self.nodes[i].parent;
        }
    }
    /// Bounds are in world coordinates, padded by the caller for labels/hit areas.
    pub fn visible(
        &self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) -> (Vec<usize>, Vec<usize>) {
        let start = (top / Y_GAP).floor().max(0.) as usize;
        let end = ((bottom / Y_GAP).ceil().max(0.) as usize + 1).min(self.levels.len());
        let mut nodes = vec![];
        for level in self.levels.get(start..end).unwrap_or_default() {
            let first = level.partition_point(|&i| self.nodes[i].x < left);
            nodes.extend(
                level[first..]
                    .iter()
                    .copied()
                    .take_while(|&i| self.nodes[i].x <= right)
                    .filter(|&i| self.nodes[i].y >= top && self.nodes[i].y <= bottom),
            );
        }
        let mut edges = vec![];
        for level in self
            .edge_levels
            .get(start.saturating_sub(1)..end)
            .unwrap_or_default()
        {
            edges.extend(level.iter().copied().filter(|&i| {
                let e = &self.edges[i];
                let a = &self.nodes[e.from];
                let b = &self.nodes[e.to];
                a.x.min(b.x) <= right && a.x.max(b.x) >= left && a.y <= bottom && b.y >= top
            }));
        }
        (nodes, edges)
    }
}
