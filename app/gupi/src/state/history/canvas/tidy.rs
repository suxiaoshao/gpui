//! Iterative Buchheim tree layout. Adapted from d3-hierarchy/src/tree.js.
//! https://github.com/d3/d3-hierarchy/blob/main/src/tree.js
// Copyright 2010-2021 Mike Bostock
//
// Permission to use, copy, modify, and/or distribute this software for any purpose
// with or without fee is hereby granted, provided that the above copyright notice
// and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
// REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
// FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
// INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
// OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
// TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
// THIS SOFTWARE.

#[derive(Default, Clone)]
struct Node {
    parent: usize,
    children: Vec<usize>,
    sibling: usize,
    ancestor: usize,
    default_ancestor: Option<usize>,
    thread: Option<usize>,
    prelim: f64,
    modifier: f64,
    change: f64,
    shift: f64,
}

/// Input parents precede their children. A hidden root joins a forest.
pub(super) fn layout(parents: &[Option<usize>]) -> Vec<(f32, usize)> {
    let root = parents.len();
    let dummy = root + 1;
    let mut nodes = vec![Node::default(); dummy + 1];
    for i in 0..=root {
        let parent = if i == root {
            dummy
        } else {
            parents[i].unwrap_or(root)
        };
        nodes[i].parent = parent;
        nodes[i].ancestor = i;
        nodes[i].sibling = nodes[parent].children.len();
        nodes[parent].children.push(i);
    }
    let mut stack = vec![(root, false)];
    while let Some((v, visited)) = stack.pop() {
        if !visited {
            stack.push((v, true));
            stack.extend(nodes[v].children.iter().rev().map(|&c| (c, false)));
            continue;
        }
        let p = nodes[v].parent;
        let previous = nodes[v]
            .sibling
            .checked_sub(1)
            .map(|i| nodes[p].children[i]);
        if !nodes[v].children.is_empty() {
            let (mut shift, mut change) = (0., 0.);
            for i in (0..nodes[v].children.len()).rev() {
                let c = nodes[v].children[i];
                nodes[c].prelim += shift;
                nodes[c].modifier += shift;
                change += nodes[c].change;
                shift += nodes[c].shift + change;
            }
            let left = nodes[v].children[0];
            let right = *nodes[v].children.last().unwrap();
            let midpoint = (nodes[left].prelim + nodes[right].prelim) / 2.;
            if let Some(w) = previous {
                nodes[v].prelim = nodes[w].prelim + 1.;
                nodes[v].modifier = nodes[v].prelim - midpoint;
            } else {
                nodes[v].prelim = midpoint;
            }
        } else if let Some(w) = previous {
            nodes[v].prelim = nodes[w].prelim + 1.;
        }
        let ancestor = nodes[p].default_ancestor.unwrap_or(nodes[p].children[0]);
        nodes[p].default_ancestor = Some(apportion(&mut nodes, v, previous, ancestor));
    }
    nodes[dummy].modifier = -nodes[root].prelim;
    let mut result = vec![(0., 0); parents.len()];
    let mut stack = vec![(root, 0_usize)];
    while let Some((v, depth)) = stack.pop() {
        let inherited = nodes[nodes[v].parent].modifier;
        if v < root {
            result[v] = ((nodes[v].prelim + inherited) as f32, depth - 1);
        }
        nodes[v].modifier += inherited;
        stack.extend(nodes[v].children.iter().rev().map(|&c| (c, depth + 1)));
    }
    result
}

fn left(nodes: &[Node], v: usize) -> Option<usize> {
    nodes[v].children.first().copied().or(nodes[v].thread)
}
fn right(nodes: &[Node], v: usize) -> Option<usize> {
    nodes[v].children.last().copied().or(nodes[v].thread)
}

fn apportion(nodes: &mut [Node], v: usize, previous: Option<usize>, mut ancestor: usize) -> usize {
    let Some(w) = previous else { return ancestor };
    let (mut inner_left, mut inner_right) = (w, v);
    let (mut outer_left, mut outer_right) = (nodes[nodes[v].parent].children[0], v);
    let (mut il, mut ir) = (nodes[inner_left].modifier, nodes[inner_right].modifier);
    let (mut ol, mut or) = (nodes[outer_left].modifier, nodes[outer_right].modifier);
    loop {
        let next_left = right(nodes, inner_left);
        let next_right = left(nodes, inner_right);
        let (Some(l), Some(r)) = (next_left, next_right) else {
            if let Some(l) = next_left.filter(|_| right(nodes, outer_right).is_none()) {
                nodes[outer_right].thread = Some(l);
                nodes[outer_right].modifier += il - or;
            }
            if let Some(r) = next_right.filter(|_| left(nodes, outer_left).is_none()) {
                nodes[outer_left].thread = Some(r);
                nodes[outer_left].modifier += ir - ol;
                ancestor = v;
            }
            break;
        };
        inner_left = l;
        inner_right = r;
        outer_left = left(nodes, outer_left).expect("outer contour includes inner contour");
        outer_right = right(nodes, outer_right).expect("outer contour includes inner contour");
        nodes[outer_right].ancestor = v;
        let separation = if nodes[l].parent == nodes[r].parent {
            1.
        } else {
            1.5
        };
        let shift = nodes[l].prelim + il - nodes[r].prelim - ir + separation;
        if shift > 0. {
            let candidate = nodes[l].ancestor;
            let a = if nodes[candidate].parent == nodes[v].parent {
                candidate
            } else {
                ancestor
            };
            let change = shift / (nodes[v].sibling - nodes[a].sibling) as f64;
            nodes[v].change -= change;
            nodes[v].shift += shift;
            nodes[a].change += change;
            nodes[v].prelim += shift;
            nodes[v].modifier += shift;
            ir += shift;
            or += shift;
        }
        il += nodes[l].modifier;
        ir += nodes[r].modifier;
        ol += nodes[outer_left].modifier;
        or += nodes[outer_right].modifier;
    }
    ancestor
}
