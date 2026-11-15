//! Ported from `src/vs/base/test/browser/ui/tree/indexTreeModel.test.ts`; test names follow
//! the upstream names. Upstream builds the list from `onDidSpliceRenderedNodes`; so does
//! [`List`] here. Regular-expression filters become substring filters (every upstream query
//! is a plain substring). Tests after the `Not upstream` marker cover behavior the upstream
//! suite does not test.

use super::*;
use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::rc::Rc;
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

fn id_of(n: &i32) -> String {
    n.to_string()
}

/// Runs `f` with plain splice options and with `diff_identity_provider` (`withSmartSplice`).
fn with_smart_splice(f: impl Fn(&dyn Fn() -> SpliceOptions<'static, i32, ()>)) {
    f(&SpliceOptions::default);
    f(&|| SpliceOptions {
        diff_identity_provider: Some(&id_of),
        ..Default::default()
    });
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

/// `toElements`: the element, followed by its children in braces.
fn shape<T: Display, F>(model: &IndexTreeModel<T, F>, id: NodeId) -> String {
    let node = model.node(id).unwrap();
    if node.children().is_empty() {
        return node.element().to_string();
    }
    let children: Vec<String> = node.children().iter().map(|&c| shape(model, c)).collect();
    format!("{}{{{}}}", node.element(), children.join(", "))
}

#[test]
fn smart_splice_deep() {
    let identity = |s: &&str| s.to_string();
    let smart = || SpliceOptions {
        diff_identity_provider: Some(&identity),
        ..Default::default()
    };
    let mut model = model("root");
    let list = bind(&model);
    model
        .splice(&[0], 0, [e("0"), e("1"), e("2"), e("3")], smart())
        .unwrap();
    let top = |model: &IndexTreeModel<&str>| -> Vec<String> {
        list.ids()
            .into_iter()
            .filter(|&id| model.node(id).unwrap().depth() == 1)
            .map(|id| shape(model, id))
            .collect()
    };
    assert_eq!(top(&model), ["0", "1", "2", "3"]);

    let to_insert = vec![
        e("-0.5"),
        p("0", vec![e("0.1")]),
        e("1"),
        p("2", vec![e("2.1"), p("2.2", vec![e("2.21")])]),
    ];
    let options = SpliceOptions {
        diff_depth: usize::MAX,
        ..smart()
    };
    model.splice(&[0], 3, to_insert, options).unwrap();
    assert_eq!(
        top(&model),
        ["-0.5", "0{0.1}", "1", "2{2.1, 2.2{2.21}}", "3"]
    );
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

#[test]
fn collapse() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, three_with_children(), options())
            .unwrap();
        assert_eq!(list.len(), 6);
        model.set_collapsed(&[0], Some(true), false).unwrap();
        assert_eq!(
            list.rows(&model),
            [(0, true, 1), (1, false, 1), (2, false, 1)]
        );
    });
}

#[test]
fn expand() {
    with_smart_splice(|options| {
        let mut model = model(-1);
        let list = bind(&model);
        model
            .splice(&[0], 0, three_collapsed_children(), options())
            .unwrap();
        assert_eq!(list.len(), 3);
        model.expand_to(&[0, 1]).unwrap();
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

/// xorshift64*: deterministic stand-in for `Math.random()`.
struct Random(u64);

impl Random {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn smart_diff_consistency() {
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    for _ in 0..500 {
        let mut model = model(-1);
        let list = bind(&model);
        let mut expected: Vec<i32> = Vec::new();
        let mut counter = 0;
        let mut steps = Vec::new();
        let mut edits = random.next() * 9.0 + 1.0;
        while edits > 0.0 {
            edits -= 1.0;
            let len = list.len();
            let splice_index = (random.next() * len as f64).floor() as usize;
            let delete_count = (random.next() * (len - splice_index) as f64).ceil() as usize;
            let insert_count = (random.next() * 5.0 + 1.0).floor() as usize;
            let mut inserts: Vec<i32> = (0..insert_count)
                .map(|_| {
                    counter += 1;
                    counter - 1
                })
                .collect();
            if random.next() < 0.5 {
                // Move existing items.
                let current = list.elements(&model);
                inserts.extend_from_slice(&current[splice_index..splice_index + delete_count / 2]);
            }
            let options = SpliceOptions {
                diff_identity_provider: Some(&id_of),
                ..Default::default()
            };
            model
                .splice(
                    &[splice_index],
                    delete_count,
                    inserts.iter().map(|&i| e(i)),
                    options,
                )
                .unwrap();
            let end = (splice_index + delete_count).min(expected.len());
            expected.splice(splice_index..end, inserts.iter().copied());
            let actual = list.elements(&model);
            steps.push(format!(
                "splice({splice_index}, {delete_count}, {inserts:?}) -> {actual:?}"
            ));
            assert_eq!(expected, actual, "steps:\n{}", steps.join("\n"));
        }
    }
}

#[test]
fn collapse_should_recursively_adjust_visible_count() {
    let mut model = model(-1);
    let list = bind(&model);
    let tree = vec![p(1, vec![p(11, vec![e(111)])]), p(2, vec![e(21)])];
    model
        .splice(&[0], 0, tree, SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), [1, 11, 111, 2, 21]);
    model.set_collapsed(&[0, 0], Some(true), false).unwrap();
    assert_eq!(list.elements(&model), [1, 11, 2, 21]);
    model.set_collapsed(&[1], Some(true), false).unwrap();
    assert_eq!(list.elements(&model), [1, 11, 2]);
}

#[test]
fn set_collapsible() {
    let mut model = model(-1);
    let list = bind(&model);
    model
        .splice(&[0], 0, [p(0, vec![e(10)])], SpliceOptions::default())
        .unwrap();
    assert_eq!(list.len(), 2);
    // (element, collapsible, collapsed) per row.
    let state = |model: &IndexTreeModel<i32>| -> Vec<(i32, bool, bool)> {
        list.ids()
            .iter()
            .map(|&id| {
                let n = model.node(id).unwrap();
                (*n.element(), n.collapsible(), n.collapsed())
            })
            .collect()
    };

    model.set_collapsible(&[0], Some(false)).unwrap();
    assert_eq!(state(&model), [(0, false, false), (10, false, false)]);
    assert!(!model.set_collapsed(&[0], Some(true), false).unwrap());
    assert_eq!(state(&model), [(0, false, false), (10, false, false)]);
    assert!(!model.set_collapsed(&[0], Some(false), false).unwrap());
    assert_eq!(state(&model), [(0, false, false), (10, false, false)]);

    model.set_collapsible(&[0], Some(true)).unwrap();
    assert_eq!(state(&model), [(0, true, false), (10, false, false)]);
    assert!(model.set_collapsed(&[0], Some(true), false).unwrap());
    assert_eq!(state(&model), [(0, true, true)]);
    assert!(model.set_collapsed(&[0], Some(false), false).unwrap());
    assert_eq!(state(&model), [(0, true, false), (10, false, false)]);
}

fn zero_to_seven() -> Vec<TreeElement<i32>> {
    vec![p(0, (1..=7).map(e).collect())]
}

#[test]
fn simple_filter() {
    let mut model = filtered(-1, |e: &i32, _| (e % 2 == 0).into());
    let list = bind(&model);
    model
        .splice(&[0], 0, zero_to_seven(), SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), [0, 2, 4, 6]);
    model.set_collapsed(&[0], Some(true), false).unwrap();
    assert_eq!(list.elements(&model), [0]);
    model.set_collapsed(&[0], Some(false), false).unwrap();
    assert_eq!(list.elements(&model), [0, 2, 4, 6]);
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
fn refilter() {
    let should_filter = Rc::new(Cell::new(false));
    let flag = should_filter.clone();
    let mut model = filtered(-1, move |e: &i32, _| (!flag.get() || e % 2 == 0).into());
    let list = bind(&model);
    model
        .splice(&[0], 0, zero_to_seven(), SpliceOptions::default())
        .unwrap();
    let all: Vec<i32> = (0..=7).collect();
    assert_eq!(list.elements(&model), all);
    model.refilter();
    assert_eq!(list.elements(&model), all);
    should_filter.set(true);
    model.refilter();
    assert_eq!(list.elements(&model), [0, 2, 4, 6]);
    should_filter.set(false);
    model.refilter();
    assert_eq!(list.elements(&model), all);
}

/// A model whose filter shows elements containing the query and recurses into the others.
fn recursive_query_model(query: &Rc<RefCell<String>>) -> IndexTreeModel<&'static str> {
    let query = query.clone();
    filtered("root", move |e: &&str, _| {
        if e.contains(query.borrow().as_str()) {
            TreeVisibility::Visible.into()
        } else {
            TreeVisibility::Recurse.into()
        }
    })
}

fn repo(collapsed: bool) -> Vec<TreeElement<&'static str>> {
    let root = p(
        "vscode",
        vec![
            e(".build"),
            e("git"),
            p(
                "github",
                vec![e("calendar.yml"), e("endgame"), e("build.js")],
            ),
            p("build", vec![e("lib"), e("gulpfile.js")]),
        ],
    );
    vec![if collapsed {
        root.collapsed(true)
    } else {
        root
    }]
}

#[test]
fn recursive_filter() {
    let query = Rc::new(RefCell::new(String::new()));
    let mut model = recursive_query_model(&query);
    let list = bind(&model);
    model
        .splice(&[0], 0, repo(false), SpliceOptions::default())
        .unwrap();
    assert_eq!(list.len(), 10);
    *query.borrow_mut() = "build".into();
    model.refilter();
    let matches = ["vscode", ".build", "github", "build.js", "build"];
    assert_eq!(list.elements(&model), matches);
    model.set_collapsed(&[0], Some(true), false).unwrap();
    assert_eq!(list.elements(&model), ["vscode"]);
    model.set_collapsed(&[0], Some(false), false).unwrap();
    assert_eq!(list.elements(&model), matches);
}

#[test]
fn recursive_filter_updates_when_children_change_133272() {
    let query = Rc::new(RefCell::new(String::new()));
    let mut model = recursive_query_model(&query);
    let list = bind(&model);
    model
        .splice(&[0], 0, [p("a", vec![e("b")])], SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), ["a", "b"]);
    *query.borrow_mut() = "visible".into();
    model.refilter();
    assert!(list.elements(&model).is_empty());

    model
        .splice(&[0, 0, 0], 0, [e("visible")], SpliceOptions::default())
        .unwrap();
    // Upstream awaits the refilter microtask here.
    assert!(model.has_pending_refilter());
    model.run_pending_refilter();
    assert!(!model.has_pending_refilter());
    assert_eq!(list.elements(&model), ["a", "b", "visible"]);
}

#[test]
fn recursive_filter_with_collapse() {
    let query = Rc::new(RefCell::new(String::new()));
    let mut model = recursive_query_model(&query);
    let list = bind(&model);
    model
        .splice(&[0], 0, repo(false), SpliceOptions::default())
        .unwrap();
    assert_eq!(list.len(), 10);
    *query.borrow_mut() = "gulp".into();
    model.refilter();
    assert_eq!(list.elements(&model), ["vscode", "build", "gulpfile.js"]);
    model.set_collapsed(&[0, 3], Some(true), false).unwrap();
    assert_eq!(list.elements(&model), ["vscode", "build"]);
    model.set_collapsed(&[0], Some(true), false).unwrap();
    assert_eq!(list.elements(&model), ["vscode"]);
}

#[test]
fn recursive_filter_while_collapsed() {
    let query = Rc::new(RefCell::new(String::new()));
    let mut model = recursive_query_model(&query);
    let list = bind(&model);
    model
        .splice(&[0], 0, repo(true), SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), ["vscode"]);
    *query.borrow_mut() = "gulp".into();
    model.refilter();
    assert_eq!(list.elements(&model), ["vscode"]);
    model.set_collapsed(&[0], Some(false), false).unwrap();
    assert_eq!(list.elements(&model), ["vscode", "build", "gulpfile.js"]);
    model.set_collapsed(&[0], Some(true), false).unwrap();
    assert_eq!(list.elements(&model), ["vscode"]);
    query.borrow_mut().clear();
    model.refilter();
    assert_eq!(list.elements(&model), ["vscode"]);
    model.set_collapsed(&[0], Some(false), false).unwrap();
    assert_eq!(list.len(), 10);
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

fn substring_model(query: &Rc<RefCell<String>>) -> IndexTreeModel<&'static str> {
    let query = query.clone();
    filtered("root", move |e: &&str, _| {
        e.contains(query.borrow().as_str()).into()
    })
}

#[test]
fn refilter_with_filtered_out_nodes() {
    let query = Rc::new(RefCell::new(String::new()));
    let mut model = substring_model(&query);
    let list = bind(&model);
    let metals = || [e("silver"), e("gold"), e("platinum")];
    model
        .splice(&[0], 0, metals(), SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), ["silver", "gold", "platinum"]);
    *query.borrow_mut() = "platinum".into();
    model.refilter();
    assert_eq!(list.elements(&model), ["platinum"]);
    model
        .splice(&[0], usize::MAX, metals(), SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), ["platinum"]);
    model.refilter();
    assert_eq!(list.elements(&model), ["platinum"]);
}

#[test]
fn explicit_hidden_nodes_should_have_render_node_count_0_issue_83211() {
    let query = Rc::new(RefCell::new(String::new()));
    let mut model = substring_model(&query);
    let list = bind(&model);
    let tree = vec![p("a", vec![e("aa")]), p("b", vec![e("bb")])];
    model
        .splice(&[0], 0, tree, SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), ["a", "aa", "b", "bb"]);
    let indices = |model: &IndexTreeModel<&str>| -> Vec<Option<usize>> {
        [&[0][..], &[0, 0], &[1], &[1, 0]]
            .iter()
            .map(|l| model.get_list_index(l).unwrap())
            .collect()
    };
    assert_eq!(indices(&model), [Some(0), Some(1), Some(2), Some(3)]);
    *query.borrow_mut() = "b".into();
    model.refilter();
    assert_eq!(list.elements(&model), ["b", "bb"]);
    assert_eq!(indices(&model), [None, None, Some(0), Some(1)]);
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
    assert!(model.set_collapsed(&[9], None, false).is_err());
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
    assert!(model.is_collapsible(&[0]).unwrap());
    assert!(!model.is_collapsible(&[1]).unwrap());
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
fn collapse_events_are_buffered_until_the_call_returns() {
    let mut model = model(-1);
    model
        .splice(
            &[0],
            0,
            [p(0, vec![p(1, vec![e(2)])])],
            SpliceOptions::default(),
        )
        .unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (a, b) = (seen.clone(), seen.clone());
    let _c = model
        .on_did_change_collapse_state()
        .subscribe(move |e| a.lock().unwrap().push(format!("collapse deep={}", e.deep)));
    let _r = model
        .on_did_change_render_node_count()
        .subscribe(move |_| b.lock().unwrap().push("count".to_owned()));
    let _l = model.on_did_splice_rendered_nodes().subscribe(move |_| {});
    assert!(model.set_collapsed(&[0], Some(true), true).unwrap());
    let seen = seen.lock().unwrap();
    assert_eq!(seen[..2], ["collapse deep=false", "collapse deep=true"]);
    assert!(seen[2..].iter().all(|s| s == "count"));
}

#[test]
fn recursive_collapse_of_root_keeps_counts() {
    // `collapseAll` collapses the root recursively.
    let mut model = model(-1);
    let list = bind(&model);
    model
        .splice(&[0], 0, three_with_children(), SpliceOptions::default())
        .unwrap();
    assert!(model.set_collapsed(&[], Some(true), true).unwrap());
    assert_eq!(list.elements(&model), [0, 1, 2]);
    assert_eq!(model.get_list_render_count(&[]).unwrap(), 3);
    model.refilter();
    assert_eq!(list.elements(&model), [0, 1, 2]);
}

#[test]
fn auto_expand_single_children() {
    let options = IndexTreeModelOptions {
        auto_expand_single_children: true,
        collapse_by_default: true,
        ..Default::default()
    };
    let mut model: IndexTreeModel<i32> = IndexTreeModel::new("test", -1, options);
    let list = bind(&model);
    let tree = vec![p(0, vec![p(1, vec![e(2), e(3)])])];
    model
        .splice(&[0], 0, tree, SpliceOptions::default())
        .unwrap();
    assert_eq!(list.elements(&model), [0]);
    model.set_collapsed(&[0], Some(false), false).unwrap();
    assert_eq!(list.elements(&model), [0, 1, 2, 3]);
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
fn smart_splice_keeps_unchanged_nodes() {
    let smart = || SpliceOptions {
        diff_identity_provider: Some(&id_of),
        ..Default::default()
    };
    let mut model = model(-1);
    let list = bind(&model);
    model
        .splice(&[0], 0, three_with_children(), smart())
        .unwrap();
    model.set_collapsed(&[0], Some(true), false).unwrap();
    let kept = model.get_node_id(&[0]).unwrap();
    model
        .splice(&[0], usize::MAX, [p(0, vec![e(10)]), e(5), e(2)], smart())
        .unwrap();
    assert_eq!(model.get_node_id(&[0]).unwrap(), kept);
    assert_eq!(
        list.rows(&model),
        [(0, true, 1), (5, false, 1), (2, false, 1)]
    );
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

#[test]
fn ten_thousand_rows_expand_and_collapse() {
    // M2 Explorer scenario: a folder with 10,000 files.
    let mut model = model(-1);
    let list = bind(&model);
    let files = (0..10_000).map(e).collect();
    let folder = p(-2, files).collapsed(true);
    model
        .splice(&[0], 0, [folder], SpliceOptions::default())
        .unwrap();
    assert_eq!(list.len(), 1);
    model.set_collapsed(&[0], Some(false), false).unwrap();
    assert_eq!(list.len(), 10_001);
    assert_eq!(model.get_list_index(&[0, 9_999]).unwrap(), Some(10_000));
    model.set_collapsed(&[0], Some(true), false).unwrap();
    assert_eq!(list.len(), 1);
}
