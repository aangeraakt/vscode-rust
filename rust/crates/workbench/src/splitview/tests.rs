//! Ports of the logic tests in `src/vs/base/test/browser/ui/splitview/splitview.test.ts`, under
//! their upstream names. DOM-only tests (element counts, separator styles, pointer capture) and
//! the re-entrancy assertions have no counterpart in this model and are not ported.

use std::sync::{Arc, Mutex};

use super::*;
use crate::sash::SashState;

/// A view that records its constraints, visibility and layouts.
#[derive(Debug)]
struct TestView {
    minimum: f64,
    maximum: f64,
    priority: LayoutPriority,
    proportional: bool,
    path_independent: bool,
    snap: bool,
    visible: bool,
    size: f64,
    orthogonal: Option<f64>,
    layouts: Vec<f64>,
}

impl TestView {
    fn new(minimum: f64, maximum: f64) -> Self {
        Self {
            minimum,
            maximum,
            priority: LayoutPriority::Normal,
            proportional: true,
            path_independent: false,
            snap: false,
            visible: true,
            size: 0.0,
            orthogonal: None,
            layouts: Vec::new(),
        }
    }

    fn with_priority(minimum: f64, maximum: f64, priority: LayoutPriority) -> Self {
        Self {
            priority,
            ..Self::new(minimum, maximum)
        }
    }

    fn path_independent(minimum: f64, maximum: f64, priority: LayoutPriority) -> Self {
        Self {
            path_independent: true,
            ..Self::with_priority(minimum, maximum, priority)
        }
    }
}

impl View for TestView {
    type Context = f64;

    fn minimum_size(&self) -> f64 {
        self.minimum
    }

    fn maximum_size(&self) -> f64 {
        self.maximum
    }

    fn priority(&self) -> LayoutPriority {
        self.priority
    }

    fn proportional_layout(&self) -> bool {
        self.proportional
    }

    fn path_independent_layout(&self) -> bool {
        self.path_independent
    }

    fn snap(&self) -> bool {
        self.snap
    }

    fn layout(&mut self, size: f64, _offset: f64, context: Option<&f64>) {
        self.size = size;
        self.orthogonal = context.copied();
        self.layouts.push(size);
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

fn split() -> SplitView<TestView> {
    SplitView::new(SplitViewOptions::default())
}

fn sizes(split: &SplitView<TestView>) -> Vec<f64> {
    (0..split.len()).map(|i| split.view_size(i)).collect()
}

/// Sets both constraints of a view and reports the change, as a view's `onDidChange` would.
fn set_constraints(split: &mut SplitView<TestView>, index: usize, minimum: f64, maximum: f64) {
    let view = split.view_mut(index);
    view.minimum = minimum;
    view.maximum = maximum;
    split.view_did_change(index, None);
}

/// Takes and clears the layouts recorded by the first `count` views. Views not yet added (or
/// already removed) record nothing.
fn take_layouts(split: &mut SplitView<TestView>, count: usize) -> Vec<Vec<f64>> {
    (0..count)
        .map(|i| {
            if i < split.len() {
                std::mem::take(&mut split.view_mut(i).layouts)
            } else {
                Vec::new()
            }
        })
        .collect()
}

#[test]
fn has_views_and_sashes_as_children() {
    let mut split = split();

    split.push_view(TestView::new(20.0, 20.0), 20.0);
    split.push_view(TestView::new(20.0, 20.0), 20.0);
    split.push_view(TestView::new(20.0, 20.0), 20.0);
    assert_eq!(split.len(), 3, "split view should have 3 views");
    assert_eq!(
        split.sash_states().len(),
        2,
        "split view should have 2 sashes"
    );

    split.remove_view(2, None);
    assert_eq!(split.len(), 2);
    assert_eq!(split.sash_states().len(), 1);

    split.remove_view(0, None);
    assert_eq!(split.len(), 1);
    assert!(
        split.sash_states().is_empty(),
        "split view should have no sashes"
    );

    split.remove_view(0, None);
    assert!(split.is_empty());
    assert!(split.sash_states().is_empty());
}

#[test]
fn calls_view_methods_on_add_view_and_remove_view() {
    let mut split = split();

    split.push_view(TestView::new(20.0, 20.0), 20.0);

    assert_eq!(split.view_size(0), 20.0, "view has right size");
    assert_eq!(split.view(0).layouts, vec![20.0], "layout is called");

    let view = split.remove_view(0, None);
    assert_eq!(view.layouts, vec![20.0]);
}

#[test]
fn stretches_view_to_viewport() {
    let mut split = split();
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), 20.0);
    assert_eq!(split.view_size(0), 200.0, "view is stretched");

    split.layout(200.0, None);
    assert_eq!(split.view_size(0), 200.0, "view stayed the same");

    split.layout(100.0, None);
    assert_eq!(split.view_size(0), 100.0, "view is collapsed");

    split.layout(20.0, None);
    assert_eq!(split.view_size(0), 20.0, "view is collapsed");

    split.layout(10.0, None);
    assert_eq!(split.view_size(0), 20.0, "view is clamped");

    split.layout(200.0, None);
    assert_eq!(split.view_size(0), 200.0, "view is stretched");
}

#[test]
fn can_resize_views() {
    let mut split = split();
    split.layout(200.0, None);

    for _ in 0..3 {
        split.push_view(TestView::new(20.0, f64::INFINITY), 20.0);
    }
    assert_eq!(sizes(&split), vec![160.0, 20.0, 20.0], "view1 is stretched");

    split.resize_view(1, 40.0);
    assert_eq!(sizes(&split), vec![140.0, 40.0, 20.0]);

    split.resize_view(0, 70.0);
    assert_eq!(sizes(&split), vec![70.0, 40.0, 90.0]);

    split.resize_view(2, 40.0);
    assert_eq!(sizes(&split), vec![70.0, 90.0, 40.0]);
}

#[test]
fn reacts_to_view_changes() {
    let mut split = split();
    split.layout(200.0, None);

    for _ in 0..3 {
        split.push_view(TestView::new(20.0, f64::INFINITY), 20.0);
    }
    assert_eq!(sizes(&split), vec![160.0, 20.0, 20.0]);

    set_constraints(&mut split, 0, 20.0, 20.0);
    assert_eq!(sizes(&split), vec![20.0, 20.0, 160.0], "view1 is collapsed");

    set_constraints(&mut split, 2, 20.0, 40.0);
    assert_eq!(sizes(&split), vec![20.0, 140.0, 40.0], "view2 is stretched");

    set_constraints(&mut split, 1, 20.0, 200.0);
    assert_eq!(
        sizes(&split),
        vec![20.0, 140.0, 40.0],
        "view2 stays the same"
    );

    set_constraints(&mut split, 2, 100.0, f64::INFINITY);
    assert_eq!(sizes(&split), vec![20.0, 80.0, 100.0]);
}

#[test]
fn sashes_are_properly_enabled_disabled() {
    let mut split = split();
    split.layout(200.0, None);

    for _ in 0..3 {
        split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    }
    assert_eq!(split.sash_states(), [SashState::Enabled; 2]);

    split.layout(60.0, None);
    assert_eq!(split.sash_states(), [SashState::Disabled; 2]);

    split.layout(20.0, None);
    assert_eq!(split.sash_states(), [SashState::Disabled; 2]);

    split.layout(200.0, None);
    assert_eq!(split.sash_states(), [SashState::Enabled; 2]);

    set_constraints(&mut split, 0, 20.0, 20.0);
    assert_eq!(
        split.sash_states(),
        [SashState::Disabled, SashState::Enabled]
    );

    set_constraints(&mut split, 1, 20.0, 20.0);
    assert_eq!(
        split.sash_states(),
        [SashState::Disabled, SashState::Disabled]
    );

    set_constraints(&mut split, 0, 20.0, 300.0);
    assert_eq!(
        split.sash_states(),
        [SashState::AtMinimum, SashState::AtMinimum]
    );

    set_constraints(&mut split, 1, 20.0, 200.0);
    assert_eq!(
        split.sash_states(),
        [SashState::AtMinimum, SashState::AtMinimum]
    );

    split.resize_view(0, 40.0);
    assert_eq!(split.sash_states(), [SashState::Enabled; 2]);
}

#[test]
fn issue_35497() {
    let mut split = split();
    split.layout(986.0, None);

    split.add_view(TestView::new(160.0, f64::INFINITY), 142.0, 0, false);
    assert_eq!(split.view_size(0), 986.0, "first view is stretched");

    // Upstream also asserts that resizing from inside the element getter throws; that re-entry
    // is impossible with `&mut self` and is not ported.
    split.add_view(TestView::new(66.0, 66.0), 66.0, 0, false);
    assert_eq!(split.view_size(0), 66.0, "second view is fixed");
    assert_eq!(split.view_size(1), 986.0 - 66.0, "first view is collapsed");
}

#[test]
fn automatic_size_distribution() {
    let mut split = split();
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(split.view_size(0), 200.0);

    split.push_view(TestView::new(20.0, f64::INFINITY), 50.0);
    assert_eq!(sizes(&split), vec![150.0, 50.0]);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(sizes(&split), vec![66.0, 66.0, 68.0]);

    split.remove_view(1, Some(Sizing::Distribute));
    assert_eq!(sizes(&split), vec![100.0, 100.0]);
}

#[test]
fn distribute_and_auto_sizing_only_lay_out_final_sizes_when_adding_and_removing_views() {
    for sizing in [Sizing::Distribute, Sizing::Auto(0)] {
        let mut split = split();
        split.layout(1200.0, None);
        split.push_view(
            TestView::path_independent(220.0, f64::INFINITY, LayoutPriority::Normal),
            1200.0,
        );
        split.view_mut(0).layouts.clear();

        split.add_view(
            TestView::path_independent(220.0, f64::INFINITY, LayoutPriority::Normal),
            sizing,
            1,
            false,
        );
        let added_second = take_layouts(&mut split, 3);
        split.push_view(
            TestView::path_independent(220.0, f64::INFINITY, LayoutPriority::Normal),
            sizing,
        );
        let added_third = take_layouts(&mut split, 3);
        split.remove_view(2, Some(sizing));
        let removed_third = take_layouts(&mut split, 3);
        split.layout(600.0, None);
        let resized = take_layouts(&mut split, 3);

        assert_eq!(
            added_second,
            vec![vec![600.0], vec![600.0], vec![]],
            "{sizing:?}"
        );
        assert_eq!(
            added_third,
            vec![vec![400.0], vec![400.0], vec![400.0]],
            "{sizing:?}"
        );
        assert_eq!(
            removed_third,
            vec![vec![600.0], vec![600.0], vec![]],
            "{sizing:?}"
        );
        assert_eq!(
            resized,
            vec![vec![300.0], vec![300.0], vec![]],
            "{sizing:?}"
        );
    }
}

#[test]
fn distribution_retains_the_intermediate_layout_unless_every_view_is_path_independent() {
    let mut split = split();
    split.layout(1200.0, None);
    split.push_view(
        TestView::path_independent(220.0, f64::INFINITY, LayoutPriority::Normal),
        1200.0,
    );
    take_layouts(&mut split, 1);

    split.push_view(TestView::new(220.0, f64::INFINITY), Sizing::Distribute);

    assert_eq!(
        take_layouts(&mut split, 2),
        vec![vec![980.0, 600.0], vec![220.0, 600.0]]
    );
}

#[test]
fn distributed_additions_preserve_fixed_capped_hidden_and_prioritized_sizes() {
    let mut split = split();
    split.layout(1000.0, None);
    split.push_view(
        TestView::path_independent(100.0, 100.0, LayoutPriority::Normal),
        100.0,
    );
    split.push_view(
        TestView::path_independent(100.0, 200.0, LayoutPriority::Normal),
        200.0,
    );
    split.push_view(
        TestView::path_independent(100.0, f64::INFINITY, LayoutPriority::High),
        700.0,
    );
    split.push_view(
        TestView::path_independent(100.0, f64::INFINITY, LayoutPriority::Normal),
        Sizing::Invisible(250.0),
    );
    take_layouts(&mut split, 4);

    split.push_view(
        TestView::path_independent(100.0, f64::INFINITY, LayoutPriority::Normal),
        Sizing::Distribute,
    );

    assert_eq!(
        take_layouts(&mut split, 5),
        vec![
            vec![100.0],
            vec![200.0],
            vec![400.0],
            vec![0.0],
            vec![300.0]
        ]
    );
    assert_eq!(sizes(&split), vec![100.0, 200.0, 400.0, 0.0, 300.0]);
    assert!(!split.is_view_visible(3));
    assert_eq!(split.view_cached_visible_size(3), Some(250.0));
    assert_eq!(split.content_size(), 1000.0);
}

#[test]
fn skip_layout_defers_a_distributed_addition_without_notifying_views() {
    let mut split = split();
    split.layout(200.0, None);
    split.push_view(TestView::new(20.0, f64::INFINITY), 200.0);
    take_layouts(&mut split, 1);

    split.add_view(
        TestView::new(20.0, f64::INFINITY),
        Sizing::Distribute,
        1,
        true,
    );
    assert_eq!(
        take_layouts(&mut split, 2),
        vec![Vec::<f64>::new(), Vec::new()],
        "deferred calls"
    );

    split.layout(200.0, None);

    assert_eq!(
        take_layouts(&mut split, 2),
        vec![vec![180.0], vec![20.0]],
        "calls"
    );
    assert_eq!(split.content_size(), 200.0);
}

#[test]
fn add_views_before_layout() {
    let mut split = split();

    split.push_view(TestView::new(20.0, f64::INFINITY), 100.0);
    split.push_view(TestView::new(20.0, f64::INFINITY), 75.0);
    split.push_view(TestView::new(20.0, f64::INFINITY), 25.0);

    split.layout(200.0, None);
    assert_eq!(sizes(&split), vec![67.0, 67.0, 66.0]);
}

#[test]
fn split_sizing() {
    let mut split = split();
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(split.view_size(0), 200.0);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Split(0));
    assert_eq!(sizes(&split), vec![100.0, 100.0]);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Split(1));
    assert_eq!(sizes(&split), vec![100.0, 50.0, 50.0]);
}

#[test]
fn split_sizing_2() {
    let mut split = split();
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(split.view_size(0), 200.0);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Split(0));
    assert_eq!(sizes(&split), vec![100.0, 100.0]);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Split(0));
    assert_eq!(sizes(&split), vec![50.0, 100.0, 50.0]);
}

#[test]
fn proportional_layout() {
    let mut split = split();
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(sizes(&split), vec![100.0, 100.0]);

    split.layout(100.0, None);
    assert_eq!(sizes(&split), vec![50.0, 50.0]);
}

#[test]
fn proportional_layout_keeps_a_view_that_keeps_its_size_within_its_bounds_such_as_one_restored_at_a_size_out_of_them()
 {
    let mut fixed = TestView::new(30.0, 30.0);
    fixed.proportional = false;
    let mut split = SplitView::from_descriptor(
        SplitViewOptions::default(),
        300.0,
        vec![
            ViewDescriptor {
                view: fixed,
                size: 100.0,
                visible: true,
            },
            ViewDescriptor {
                view: TestView::new(20.0, f64::INFINITY),
                size: 100.0,
                visible: true,
            },
            ViewDescriptor {
                view: TestView::new(20.0, f64::INFINITY),
                size: 100.0,
                visible: true,
            },
        ],
    );

    split.layout(300.0, None);
    assert_eq!(sizes(&split), vec![30.0, 135.0, 135.0]);

    split.layout(600.0, None);
    assert_eq!(sizes(&split), vec![30.0, 285.0, 285.0], "grown");
}

#[test]
fn disable_proportional_layout() {
    let mut split = SplitView::new(SplitViewOptions {
        proportional_layout: false,
        ..SplitViewOptions::default()
    });
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(sizes(&split), vec![100.0, 100.0]);

    split.layout(100.0, None);
    assert_eq!(sizes(&split), vec![80.0, 20.0]);
}

#[test]
fn high_layout_priority() {
    let mut split = SplitView::new(SplitViewOptions {
        proportional_layout: false,
        ..SplitViewOptions::default()
    });
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(
        TestView::with_priority(20.0, f64::INFINITY, LayoutPriority::High),
        Sizing::Distribute,
    );
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(sizes(&split), vec![66.0, 68.0, 66.0]);

    split.layout(180.0, None);
    assert_eq!(sizes(&split), vec![66.0, 48.0, 66.0]);

    split.layout(124.0, None);
    assert_eq!(sizes(&split), vec![66.0, 20.0, 38.0]);

    split.layout(60.0, None);
    assert_eq!(sizes(&split), vec![20.0, 20.0, 20.0]);

    split.layout(200.0, None);
    assert_eq!(sizes(&split), vec![20.0, 160.0, 20.0]);
}

#[test]
fn low_layout_priority() {
    let mut split = SplitView::new(SplitViewOptions {
        proportional_layout: false,
        ..SplitViewOptions::default()
    });
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(
        TestView::with_priority(20.0, f64::INFINITY, LayoutPriority::Low),
        Sizing::Distribute,
    );
    assert_eq!(sizes(&split), vec![66.0, 68.0, 66.0]);

    split.layout(180.0, None);
    assert_eq!(sizes(&split), vec![66.0, 48.0, 66.0]);

    split.layout(132.0, None);
    assert_eq!(sizes(&split), vec![46.0, 20.0, 66.0]);

    split.layout(60.0, None);
    assert_eq!(sizes(&split), vec![20.0, 20.0, 20.0]);

    split.layout(200.0, None);
    assert_eq!(sizes(&split), vec![20.0, 160.0, 20.0]);
}

// Upstream "view changing its size constraints while a sibling is toggled does not squeeze that
// sibling (#334167)" is not ported: the activity bar's constraint change arrives from inside the
// side bar's `setVisible`, which needs re-entrant notification. See the module docs.

#[test]
fn context_propagates_to_views() {
    let mut split = split();
    split.layout(200.0, None);

    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(
        TestView::with_priority(20.0, f64::INFINITY, LayoutPriority::Low),
        Sizing::Distribute,
    );

    split.layout(200.0, Some(100.0));
    let orthogonal: Vec<Option<f64>> = (0..3).map(|i| split.view(i).orthogonal).collect();
    assert_eq!(orthogonal, vec![Some(100.0); 3]);
}

#[test]
fn descriptor_keeps_hidden_views_at_their_cached_visible_size() {
    let split = SplitView::from_descriptor(
        SplitViewOptions::default(),
        200.0,
        vec![
            ViewDescriptor {
                view: TestView::new(20.0, f64::INFINITY),
                size: 100.0,
                visible: true,
            },
            ViewDescriptor {
                view: TestView::new(20.0, f64::INFINITY),
                size: 50.0,
                visible: false,
            },
        ],
    );

    assert!(split.is_view_visible(0));
    assert!(!split.is_view_visible(1));
    assert_eq!(split.view_cached_visible_size(1), Some(50.0));
    assert_eq!(split.content_size(), 100.0);
}

#[test]
fn move_view_keeps_the_size_of_the_moved_view() {
    let mut split = split();
    split.layout(200.0, None);
    split.push_view(TestView::new(10.0, f64::INFINITY), 50.0);
    split.push_view(TestView::new(30.0, f64::INFINITY), 150.0);

    split.move_view(0, 1);

    assert_eq!(
        split.view(0).minimum,
        30.0,
        "the other view moved to the front"
    );
    assert_eq!(split.view(1).minimum, 10.0);
    assert_eq!(sizes(&split), vec![150.0, 50.0]);
}

#[test]
fn swap_views_exchanges_the_views_and_keeps_the_sizes_at_their_positions() {
    let mut split = split();
    split.layout(200.0, None);
    split.push_view(TestView::new(10.0, f64::INFINITY), 50.0);
    split.push_view(TestView::new(30.0, f64::INFINITY), 150.0);

    split.swap_views(0, 1);

    assert_eq!(split.view(0).minimum, 30.0);
    assert_eq!(sizes(&split), vec![50.0, 150.0]);
}

#[test]
fn sash_drag_moves_space_between_neighbours_and_reports_the_end() {
    let mut split = split();
    split.layout(200.0, None);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(sizes(&split), vec![100.0, 100.0]);

    let ended = Arc::new(Mutex::new(Vec::new()));
    let sink = ended.clone();
    let _subscription = split
        .on_did_sash_change()
        .subscribe(move |index| sink.lock().unwrap().push(*index));

    split.sash_start(0, 100.0, false);
    split.sash_change(130.0);
    assert_eq!(sizes(&split), vec![130.0, 70.0]);

    // The drag is clamped by the minimum of the view it shrinks.
    split.sash_change(-1000.0);
    assert_eq!(sizes(&split), vec![20.0, 180.0]);

    split.sash_end(0);
    assert_eq!(*ended.lock().unwrap(), vec![0]);
}

#[test]
fn sash_drag_snaps_a_view_hidden_past_its_half_minimum() {
    let mut split = split();
    split.layout(200.0, None);
    let mut snapping = TestView::new(20.0, f64::INFINITY);
    snapping.snap = true;
    split.push_view(snapping, Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);

    split.sash_start(0, 100.0, false);
    // `sash_change` takes the absolute position: 5.0 is a delta of -95, past the snap threshold
    // of -90 (the minimum minus half of it), so the view hides.
    split.sash_change(5.0);

    assert!(!split.is_view_visible(0));
    assert_eq!(sizes(&split), vec![0.0, 200.0]);
}

#[test]
fn reset_sash_is_suppressed_while_a_snap_keeps_a_view_hidden() {
    let mut split = split();
    split.layout(200.0, None);
    let mut snapping = TestView::new(20.0, f64::INFINITY);
    snapping.snap = true;
    split.push_view(snapping, Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    split.set_view_visible(0, false);

    let resets = Arc::new(Mutex::new(0_usize));
    let counter = resets.clone();
    let _subscription = split
        .on_did_sash_reset()
        .subscribe(move |_| *counter.lock().unwrap() += 1);

    split.reset_sash(0);
    assert_eq!(*resets.lock().unwrap(), 0);

    split.set_view_visible(0, true);
    split.reset_sash(0);
    assert_eq!(*resets.lock().unwrap(), 1);
}

#[test]
fn hidden_snapping_view_at_the_start_makes_the_sash_at_minimum_unless_start_snapping_is_off() {
    let mut split = split();
    split.layout(200.0, None);
    let mut snapping = TestView::new(20.0, f64::INFINITY);
    snapping.snap = true;
    split.push_view(snapping, Sizing::Distribute);
    split.push_view(TestView::new(20.0, f64::INFINITY), Sizing::Distribute);
    assert_eq!(split.sash_states(), [SashState::Enabled]);

    split.set_view_visible(0, false);
    assert_eq!(split.sash_states(), [SashState::AtMinimum]);

    split.set_start_snapping_enabled(false);
    assert_eq!(split.sash_states(), [SashState::Disabled]);
}
