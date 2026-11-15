//! Ported from `src/vs/base/test/browser/ui/tree/indexTreeModel.test.ts`; test names follow
//! the upstream names. Upstream builds the list from `onDidSpliceRenderedNodes`; so does
//! [`List`] here. Regular-expression filters become substring filters (every upstream query
//! is a plain substring). Tests after the `Not upstream` marker cover behavior the upstream
//! suite does not test.

use super::*;
use std::sync::{Arc, Mutex};
use vscode_api::Disposable;

/// The rows of a list view bound to a model.
struct List {
    rows: Arc<Mutex<Vec<NodeId>>>,
    _subscription: Disposable,
}

fn bind<T, F>(model: &IndexTreeModel<T, F>) -> List {
    let rows = Arc::new(Mutex::new(Vec::new()));
    let sink = rows.clone();
    let subscription = model.on_did_splice_rendered_nodes().subscribe(move |s| {
        let mut rows = sink.lock().unwrap();
        let end = (s.start + s.delete_count).min(rows.len());
        rows.splice(s.start..end, s.elements.iter().copied());
    });
    List {
        rows,
        _subscription: subscription,
    }
}

impl List {
    fn ids(&self) -> Vec<NodeId> {
        self.rows.lock().unwrap().clone()
    }
    fn len(&self) -> usize {
        self.rows.lock().unwrap().len()
    }
    fn elements<T: Clone, F>(&self, model: &IndexTreeModel<T, F>) -> Vec<T> {
        self.ids()
            .iter()
            .map(|&id| model.node(id).unwrap().element().clone())
            .collect()
    }
    /// `(element, collapsed, depth)` per row.
    fn rows<T: Clone, F>(&self, model: &IndexTreeModel<T, F>) -> Vec<(T, bool, usize)> {
        self.ids()
            .iter()
            .map(|&id| {
                let n = model.node(id).unwrap();
                (n.element().clone(), n.collapsed(), n.depth())
            })
            .collect()
    }
}

fn e<T>(element: T) -> TreeElement<T> {
    TreeElement::new(element)
}

fn p<T>(element: T, children: Vec<TreeElement<T>>) -> TreeElement<T> {
    TreeElement::new(element).with_children(children)
}

fn model<T>(root: T) -> IndexTreeModel<T> {
    IndexTreeModel::new("test", root, IndexTreeModelOptions::default())
}

fn filtered<T: 'static>(
    root: T,
    filter: impl Fn(&T, TreeVisibility) -> TreeFilterResult<()> + 'static,
) -> IndexTreeModel<T> {
    let options = IndexTreeModelOptions {
        filter: Some(Box::new(filter)),
        ..Default::default()
    };
    IndexTreeModel::new("test", root, options)
}

/// Runs `f` with plain splice options (`withSmartSplice`; the smart-splice run is not ported
/// yet).
fn with_smart_splice(f: impl Fn(&dyn Fn() -> SpliceOptions<'static, i32, ()>)) {
    f(&SpliceOptions::default);
}

fn three_with_children() -> Vec<TreeElement<i32>> {
    vec![p(0, vec![e(10), e(11), e(12)]), e(1), e(2)]
}

fn three_collapsed_children() -> Vec<TreeElement<i32>> {
    vec![p(0, vec![e(10), e(11), e(12)]).collapsed(true), e(1), e(2)]
}

#[test]
fn ctor() {
    let model = model(-1);
    let list = bind(&model);
    assert_eq!(list.len(), 0);
}

#[test]
fn insert() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, [e(0), e(1), e(2)], options())
            .unwrap();
        assert_eq!(
            list.rows(&model),
            [(0, false, 1), (1, false, 1), (2, false, 1)]
        );
    });
}

#[test]
fn deep_insert() {
    // Upstream does not pass the smart-splice options to this splice either.
    with_smart_splice(|_| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, three_with_children(), SpliceOptions::default())
            .unwrap();
        assert_eq!(
            list.rows(&model),
            [
                (0, false, 1),
                (10, false, 2),
                (11, false, 2),
                (12, false, 2),
                (1, false, 1),
                (2, false, 1)
            ]
        );
    });
}

#[test]
fn deep_insert_collapsed() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, three_collapsed_children(), options())
            .unwrap();
        assert_eq!(
            list.rows(&model),
            [(0, true, 1), (1, false, 1), (2, false, 1)]
        );
    });
}

#[test]
fn delete() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, [e(0), e(1), e(2)], options())
            .unwrap();
        assert_eq!(list.len(), 3);
        model.splice(&[1], 1, [], options()).unwrap();
        assert_eq!(list.rows(&model), [(0, false, 1), (2, false, 1)]);
        model.splice(&[0], 2, [], options()).unwrap();
        assert_eq!(list.len(), 0);
    });
}

#[test]
fn nested_delete() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, three_with_children(), options())
            .unwrap();
        assert_eq!(list.len(), 6);
        model.splice(&[1], 2, [], options()).unwrap();
        assert_eq!(
            list.rows(&model),
            [
                (0, false, 1),
                (10, false, 2),
                (11, false, 2),
                (12, false, 2)
            ]
        );
    });
}

#[test]
fn deep_delete() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, three_with_children(), options())
            .unwrap();
        assert_eq!(list.len(), 6);
        model.splice(&[0], 1, [], options()).unwrap();
        assert_eq!(list.rows(&model), [(1, false, 1), (2, false, 1)]);
    });
}

#[test]
fn hidden_delete() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, three_collapsed_children(), options())
            .unwrap();
        assert_eq!(list.len(), 3);
        model.splice(&[0, 1], 1, [], options()).unwrap();
        assert_eq!(list.len(), 3);
        model.splice(&[0, 0], 2, [], options()).unwrap();
        assert_eq!(list.len(), 3);
    });
}

fn zero_to_seven() -> Vec<TreeElement<i32>> {
    vec![p(0, (1..=7).map(e).collect())]
}

#[test]
fn recursive_filter_on_initial_model() {
    let mut model = filtered(-1, |e: &i32, _| {
        if *e == 0 {
            TreeVisibility::Recurse.into()
        } else {
            TreeVisibility::Hidden.into()
        }
    });
    let list = bind(&model);
    model
        .splice(&[0], 0, [p(0, vec![e(1), e(2)])], SpliceOptions::default())
        .unwrap();
    assert!(list.elements(&model).is_empty());
}

#[test]
fn get_node_location_simple() {
    let mut model = model(-1);
    let list = bind(&model);
    model
        .splice(&[0], 0, three_with_children(), SpliceOptions::default())
        .unwrap();
    let locations: Vec<Vec<usize>> = list
        .ids()
        .into_iter()
        .map(|id| model.get_node_location(id))
        .collect();
    let expected: [&[usize]; 6] = [&[0], &[0, 0], &[0, 1], &[0, 2], &[1], &[2]];
    assert_eq!(locations, expected);
}

#[test]
fn get_node_location_with_filter() {
    let mut model = filtered(-1, |e: &i32, _| (e % 2 == 0).into());
    let list = bind(&model);
    model
        .splice(&[0], 0, zero_to_seven(), SpliceOptions::default())
        .unwrap();
    let locations: Vec<Vec<usize>> = list
        .ids()
        .into_iter()
        .map(|id| model.get_node_location(id))
        .collect();
    let expected: [&[usize]; 4] = [&[0], &[0, 1], &[0, 3], &[0, 5]];
    assert_eq!(locations, expected);
}

// ---- Not upstream ----------------------------------------------------------------------

#[test]
fn invalid_locations_are_errors() {
    let mut model = model(-1);
    model
        .splice(&[0], 0, three_with_children(), SpliceOptions::default())
        .unwrap();
    let err = model
        .splice(&[], 0, [], SpliceOptions::default())
        .unwrap_err();
    assert_eq!(err.to_string(), "TreeError [test] Invalid tree location");
    assert!(model.splice(&[5], 0, [], SpliceOptions::default()).is_err());
    assert!(
        model
            .splice(&[3, 0], 0, [], SpliceOptions::default())
            .is_err()
    );
    assert!(
        model
            .splice(&[3], 0, [e(3)], SpliceOptions::default())
            .is_ok()
    );
    assert!(model.has(&[0, 2]));
    assert!(!model.has(&[0, 3]));
    assert!(model.get_node(&[4]).is_err());
}

#[test]
fn queries_follow_upstream() {
    let mut model = model(-1);
    model
        .splice(&[0], 0, three_with_children(), SpliceOptions::default())
        .unwrap();
    assert_eq!(model.get_first_element_child(&[0]).unwrap(), Some(&10));
    assert_eq!(model.get_first_element_child(&[1]).unwrap(), None);
    assert_eq!(model.get_last_element_ancestor(&[]).unwrap(), Some(&2));
    assert_eq!(model.get_last_element_ancestor(&[0]).unwrap(), Some(&12));
    assert_eq!(model.get_parent_node_location(&[0, 1]), Some(vec![0]));
    assert_eq!(model.get_parent_node_location(&[0]), Some(vec![]));
    assert_eq!(model.get_parent_node_location(&[]), None);
    assert_eq!(model.get_list_render_count(&[0]).unwrap(), 4);
    assert_eq!(model.get_list_render_count(&[]).unwrap(), 6);
    assert_eq!(model.get_node(&[]).unwrap().element(), &-1);
    assert!(model.get_node(&[0]).unwrap().collapsible());
    assert!(!model.get_node(&[1]).unwrap().collapsible());
}

#[test]
fn deleted_node_ids_go_stale() {
    let mut model = model(-1);
    model
        .splice(&[0], 0, [e(0)], SpliceOptions::default())
        .unwrap();
    let old = model.get_node_id(&[0]).unwrap();
    model
        .splice(&[0], 1, [e(1)], SpliceOptions::default())
        .unwrap();
    let new = model.get_node_id(&[0]).unwrap();
    assert!(model.node(old).is_none());
    assert_eq!(model.node(new).unwrap().element(), &1);
}

#[test]
fn filter_data_is_stored_on_nodes() {
    let options = IndexTreeModelOptions {
        filter: Some(Box::new(|e: &i32, _| TreeFilterResult::Data {
            visibility: TreeVisibility::Visible,
            data: e * 10,
        })),
        ..Default::default()
    };
    let mut model: IndexTreeModel<i32, i32> = IndexTreeModel::new("test", -1, options);
    model
        .splice(&[0], 0, [e(4)], SpliceOptions::default())
        .unwrap();
    assert_eq!(model.get_node(&[0]).unwrap().filter_data(), Some(&40));
}

#[test]
fn create_and_delete_callbacks_visit_every_node() {
    let mut model = model(-1);
    let mut created = Vec::new();
    let mut on_create = |_: NodeId, n: &TreeNode<i32, ()>| created.push(*n.element());
    let options = SpliceOptions {
        on_did_create_node: Some(&mut on_create),
        ..Default::default()
    };
    model
        .splice(&[0], 0, three_with_children(), options)
        .unwrap();
    assert_eq!(created, [10, 11, 12, 0, 1, 2]);
    let mut deleted = Vec::new();
    let mut on_delete = |_: NodeId, n: &TreeNode<i32, ()>| deleted.push(*n.element());
    let options = SpliceOptions {
        on_did_delete_node: Some(&mut on_delete),
        ..Default::default()
    };
    model.splice(&[0], 2, [], options).unwrap();
    assert_eq!(deleted, [0, 10, 11, 12, 1]);
}
