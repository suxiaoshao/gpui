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
    assert!(tree.can_collapse(0));
    assert_eq!(tree.collapse_count(0), 9);
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
    assert_eq!(tree.nodes.len(), 12);
    for id in 1..11 {
        assert!(tree.node_for(&id.to_string()).is_some());
    }
    assert_eq!(tree.selected.as_deref(), Some("5"));
}

#[test]
fn collapse_keeps_selection_preview_and_execution_but_hides_other_process_nodes() {
    let parents: Vec<_> = (0_usize..8).map(|i| i.checked_sub(1)).collect();
    let mut input = rows(&parents);
    input[0].kind = HistoryKind::User;
    input[7].kind = HistoryKind::Assistant;
    input[7].current = false;
    input[5].current = true;
    let mut tree = Tree::default();
    tree.replace(input, Some("4".into()));
    while let Some(edge) = tree.edges.iter().position(|edge| !edge.hidden.is_empty()) {
        tree.expand(edge);
    }
    tree.select("2".into());
    assert_eq!(tree.collapse_count(0), 3);
    tree.collapse(0);
    assert_eq!(
        tree.nodes.iter().map(|n| n.row).collect::<Vec<_>>(),
        [0, 2, 4, 5, 7]
    );
    assert_eq!(tree.selected.as_deref(), Some("2"));
    assert_eq!(tree.preview.as_deref(), Some("4"));
    assert_eq!(tree.current.as_deref(), Some("5"));
    assert_eq!(
        tree.edges
            .iter()
            .map(|edge| edge.hidden.len())
            .sum::<usize>(),
        3
    );
    assert_eq!(tree.collapse_count(0), 0);
    assert!(!tree.can_collapse(0));
    tree.expand(0);
    assert_eq!(tree.collapse_count(0), 1);
    tree.collapse(0);
    assert_eq!(tree.nodes.len(), 5);
}

#[test]
fn short_runs_fold_including_summaries_and_labeled_records() {
    let parents: Vec<_> = (0_usize..8).map(|i| i.checked_sub(1)).collect();
    let mut input = rows(&parents);
    input[0].kind = HistoryKind::User;
    input[1].kind = HistoryKind::Compaction;
    input[1].label = Some("bookmark".into());
    input[2].kind = HistoryKind::Assistant;
    input[3].kind = HistoryKind::Thinking;
    input[4].kind = HistoryKind::BranchSummary;
    input[5].kind = HistoryKind::Assistant;
    input[6].kind = HistoryKind::Failed;
    input[7].kind = HistoryKind::User;
    let mut tree = Tree::default();
    tree.replace(input, None);
    assert_eq!(
        tree.nodes.iter().map(|n| n.row).collect::<Vec<_>>(),
        [0, 2, 5, 7]
    );
    assert_eq!(
        tree.edges
            .iter()
            .map(|e| e.hidden.clone())
            .collect::<Vec<_>>(),
        [vec![1], vec![3, 4], vec![6]]
    );
    tree.set_preview(Some("4".into()));
    assert!(tree.node_for("4").is_some());
    assert!(tree.node_for("3").is_none());
    assert!(!tree.can_collapse(1));
    tree.select("6".into());
    assert!(tree.node_for("6").is_some());
    assert!(!tree.can_collapse(2));
}

#[test]
fn structural_process_nodes_keep_forks_roots_and_leaves_connected() {
    let input = rows(&[None, Some(0), Some(1), Some(1), Some(2), Some(3)]);
    let mut tree = Tree::default();
    tree.replace(input, None);
    assert_eq!(
        tree.nodes.iter().map(|n| n.row).collect::<Vec<_>>(),
        [0, 1, 4, 5]
    );
    let fork = tree.node_for("1").unwrap();
    assert_eq!(tree.nodes[fork].children.len(), 2);
    assert_eq!(
        tree.edges
            .iter()
            .map(|e| e.hidden.clone())
            .collect::<Vec<_>>(),
        [vec![], vec![2], vec![3]]
    );
}

#[test]
fn content_filter_roundtrip_preserves_expansion_until_source_entries_disappear() {
    let mut input = rows(&[None, Some(0), Some(1), Some(2)]);
    input[0].kind = HistoryKind::User;
    input[3].kind = HistoryKind::Assistant;
    let source_ids = input.iter().map(|r| r.id.clone()).collect();
    let mut tree = Tree::default();
    tree.replace(input.clone(), None);
    tree.expand(0);
    let mut brief = vec![input[0].clone(), input[3].clone()];
    brief[1].parent = Some("0".into());
    tree.retain_entries(&source_ids);
    tree.replace(brief, None);
    assert_eq!(tree.nodes.len(), 2);
    tree.replace(input.clone(), None);
    assert_eq!(tree.nodes.len(), 4);
    assert!(tree.can_collapse(0));
    tree.collapse(0);
    assert_eq!(tree.nodes.len(), 2);
    tree.expand(0);
    tree.retain_entries(&HashSet::from(["0".into(), "3".into()]));
    tree.replace(input, None);
    assert_eq!(tree.nodes.len(), 2);
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
