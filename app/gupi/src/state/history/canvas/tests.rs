use super::*;

fn rows(parents: &[Option<usize>]) -> Vec<HistoryRow> {
    parents
        .iter()
        .enumerate()
        .map(|(i, p)| HistoryRow {
            id: i.to_string(),
            title: format!("node {i}"),
            kind: HistoryKind::AssistantProgress,
            tool: None,
            timestamp: String::new(),
            label: None,
            parent: p.map(|i| i.to_string()),
            indirect_parent: false,
            current: i + 1 == parents.len(),
        })
        .collect()
}

#[test]
fn folding_is_reversible_counts_only_interior_and_protects_navigation() {
    let parents: Vec<_> = (0_usize..12).map(|i| i.checked_sub(1)).collect();
    let mut tree = Tree::default();
    tree.replace(rows(&parents), None);
    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.edges[0].hidden, (1..11).collect::<Vec<_>>());
    tree.expand(0);
    assert_eq!(tree.nodes.len(), 12);
    tree.select("5".into());
    assert!(!tree.can_collapse(0));
    assert_eq!(tree.highlighted.len(), 6);
    tree.select("11".into());
    assert!(tree.can_collapse(0));
    tree.collapse(0);
    assert_eq!(tree.nodes.len(), 2);
    assert!(tree.select("5".into()));
    assert_eq!(tree.nodes.len(), 12);
    assert_eq!(tree.current.as_deref(), Some("11"));
    let mut appended = parents;
    appended.push(Some(11));
    tree.replace(rows(&appended), None);
    assert_eq!(tree.nodes.len(), 13);
    assert_eq!(tree.selected.as_deref(), Some("5"));
}

#[test]
fn semantic_anchors_split_chains_and_short_runs_stay_visible() {
    let parents: Vec<_> = (0_usize..20).map(|i| i.checked_sub(1)).collect();
    let mut input = rows(&parents);
    input[5].label = Some("bookmark".into());
    input[10].kind = HistoryKind::Assistant;
    input[13].kind = HistoryKind::Compaction;
    let mut tree = Tree::default();
    tree.replace(input, Some("16".into()));
    for id in [
        "0", "5", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19",
    ] {
        assert!(tree.node_for(id).is_some(), "missing {id}");
    }
    assert_eq!(
        tree.edges
            .iter()
            .filter(|e| !e.hidden.is_empty())
            .map(|e| e.hidden.len())
            .collect::<Vec<_>>(),
        vec![4, 4]
    );
}

#[test]
fn dialogue_anchors_keep_questions_and_answers_while_errors_can_fold() {
    let parents: Vec<_> = (0_usize..12).map(|i| i.checked_sub(1)).collect();
    let mut input = rows(&parents);
    for index in [0, 6] {
        input[index].kind = HistoryKind::User;
    }
    for index in [5, 11] {
        input[index].kind = HistoryKind::Assistant;
    }
    for index in [2, 9] {
        input[index].kind = HistoryKind::Failed;
    }
    let mut tree = Tree::default();
    tree.replace(input, None);
    assert_eq!(
        tree.nodes.iter().map(|node| node.row).collect::<Vec<_>>(),
        [0, 5, 6, 11]
    );
    assert_eq!(tree.edges[0].hidden, [1, 2, 3, 4]);
    assert_eq!(tree.edges[2].hidden, [7, 8, 9, 10]);
    tree.expand(0);
    let failed = tree.node_for("2").expect("expanded error remains visible");
    assert_eq!(tree.rows[tree.nodes[failed].row].kind, HistoryKind::Failed);
}

#[test]
fn long_chain_and_wide_forks_layout_without_recursion_or_overlaps() {
    for parents in [
        (0_usize..10_000)
            .map(|i| i.checked_sub(1))
            .collect::<Vec<_>>(),
        std::iter::once(None)
            .chain(std::iter::repeat_n(Some(0), 2_000))
            .collect(),
        (0..2_000)
            .map(|i| (i > 0).then(|| (i * 37 + 11) % i))
            .collect(),
    ] {
        let mut tree = Tree::default();
        tree.replace(rows(&parents), None);
        // Unfold every node to also exercise the 10k-deep layout itself.
        tree.expanded.extend(tree.rows.iter().map(|r| r.id.clone()));
        tree.rebuild();
        assert_eq!(tree.nodes.len(), parents.len());
        for level in &tree.levels {
            for pair in level.windows(2) {
                assert!(tree.nodes[pair[1]].x - tree.nodes[pair[0]].x >= X_GAP - 0.1);
            }
        }
        for (i, n) in tree.nodes.iter().enumerate() {
            assert!(n.x.is_finite() && n.y.is_finite());
            if let Some(p) = n.parent {
                assert_eq!(n.y, tree.nodes[p].y + Y_GAP);
                assert!(tree.nodes[p].children.contains(&i));
            }
        }
    }
}

#[test]
fn forest_and_clipped_edges_keep_their_real_endpoints() {
    let mut tree = Tree::default();
    tree.replace(rows(&[None, Some(0), Some(0), None, Some(3)]), None);
    assert_eq!(tree.nodes.iter().filter(|n| n.parent.is_none()).count(), 2);
    for (i, edge) in tree.edges.iter().enumerate() {
        let a = &tree.nodes[edge.from];
        let b = &tree.nodes[edge.to];
        let x = (a.x + b.x) / 2.;
        let y = (a.y + b.y) / 2.;
        let (nodes, edges) = tree.visible(x - 1., y - 1., x + 1., y + 1.);
        assert!(nodes.is_empty());
        assert!(edges.contains(&i));
    }
}
