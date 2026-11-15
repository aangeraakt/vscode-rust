//! Ports of the logic tests in `src/vs/base/test/browser/ui/splitview/splitview.test.ts`, under
//! their upstream names. DOM-only tests (element counts, separator styles, pointer capture) and
//! the re-entrancy assertions have no counterpart in this model and are not ported.
//!
//! This first PR of the stack holds a smoke subset; the remaining tests follow.

use super::*;

/// A view that records its constraints.
#[derive(Debug)]
struct TestView {
    minimum: f64,
    maximum: f64,
}

impl TestView {
    fn new(minimum: f64, maximum: f64) -> Self {
        Self { minimum, maximum }
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

    fn layout(&mut self, _size: f64, _offset: f64, _context: Option<&f64>) {}
}

fn split() -> SplitView<TestView> {
    SplitView::new(SplitViewOptions::default())
}

fn sizes(split: &SplitView<TestView>) -> Vec<f64> {
    (0..split.len()).map(|i| split.view_size(i)).collect()
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
