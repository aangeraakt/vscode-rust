//! Ports of `src/vs/editor/test/common/viewLayout/linesLayout.test.ts`.
//!
//! Test names follow the upstream titles. Repeated assertions are grouped into tables.

use super::*;

fn insert(
    layout: &mut LinesLayout,
    after: i32,
    ordinal: i32,
    height: i32,
    min_width: i32,
) -> String {
    let mut id = String::new();
    layout.change_whitespace(|a| id = a.insert_whitespace(after, ordinal, height, min_width));
    id
}

fn change_one(layout: &mut LinesLayout, id: &str, after: i32, height: i32) {
    layout.change_whitespace(|a| a.change_one_whitespace(id, after, height));
}

fn remove(layout: &mut LinesLayout, id: &str) {
    layout.change_whitespace(|a| a.remove_whitespace(id));
}

fn offsets(layout: &LinesLayout, expected: &[(i32, f64)]) {
    for &(line, offset) in expected {
        assert_eq!(
            layout.vertical_offset_for_line_number(line, false),
            offset,
            "offset of line {line}"
        );
    }
}

fn line_at(layout: &LinesLayout, expected: &[(f64, i32)]) {
    for &(offset, line) in expected {
        assert_eq!(
            layout.line_number_at_or_after_vertical_offset(offset),
            line,
            "line at offset {offset}"
        );
    }
}

fn ws_index_at(layout: &LinesLayout, expected: &[(f64, isize)]) {
    for &(offset, index) in expected {
        assert_eq!(
            layout.whitespace_index_at_or_after_vertical_offset(offset),
            index,
            "whitespace index at offset {offset}"
        );
    }
}

/// Checks rows of `(after_line_number, height, accumulated height)`.
fn ws_rows(layout: &LinesLayout, rows: &[(i32, i32, f64)]) {
    assert_eq!(layout.whitespaces_count(), rows.len(), "whitespace count");
    for (index, &(after, height, accumulated)) in rows.iter().enumerate() {
        assert_eq!(
            layout.after_line_number_for_whitespace_index(index),
            after,
            "after of {index}"
        );
        assert_eq!(
            layout.height_for_whitespace_index(index),
            height,
            "height of {index}"
        );
        assert_eq!(
            layout.whitespaces_accumulated_height(index),
            accumulated,
            "accumulated {index}"
        );
    }
}

fn before_lines(layout: &LinesLayout, expected: &[(i32, f64)]) {
    for &(line, height) in expected {
        assert_eq!(
            layout.whitespace_accumulated_height_before_line_number(line),
            height,
            "whitespace before line {line}"
        );
    }
}

#[test]
fn lines_layout_1() {
    let mut layout = LinesLayout::new(10, 10.0, 0.0, 0.0, &[]);
    assert_eq!(layout.lines_total_height(), 100.0);
    offsets(
        &layout,
        &[(1, 0.0), (2, 10.0), (3, 20.0), (5, 40.0), (10, 90.0)],
    );
    line_at(
        &layout,
        &[
            (0.0, 1),
            (1.0, 1),
            (5.0, 1),
            (9.0, 1),
            (10.0, 2),
            (11.0, 2),
            (15.0, 2),
            (19.0, 2),
            (20.0, 3),
            (21.0, 3),
            (29.0, 3),
        ],
    );

    // Whitespace of height 5 after line 2.
    insert(&mut layout, 2, 0, 5, 0);
    assert_eq!(layout.lines_total_height(), 105.0);
    offsets(
        &layout,
        &[(1, 0.0), (2, 10.0), (3, 25.0), (4, 35.0), (5, 45.0)],
    );
    line_at(
        &layout,
        &[
            (0.0, 1),
            (1.0, 1),
            (9.0, 1),
            (10.0, 2),
            (20.0, 3),
            (21.0, 3),
            (24.0, 3),
            (25.0, 3),
            (35.0, 4),
            (45.0, 5),
            (104.0, 10),
            (105.0, 10),
        ],
    );

    // Two more whitespaces of height 5, after lines 3 and 4.
    insert(&mut layout, 3, 0, 5, 0);
    insert(&mut layout, 4, 0, 5, 0);
    assert_eq!(layout.lines_total_height(), 115.0);
    offsets(
        &layout,
        &[
            (1, 0.0),
            (2, 10.0),
            (3, 25.0),
            (4, 40.0),
            (5, 55.0),
            (6, 65.0),
        ],
    );
    line_at(
        &layout,
        &[
            (0.0, 1),
            (1.0, 1),
            (9.0, 1),
            (10.0, 2),
            (19.0, 2),
            (20.0, 3),
            (34.0, 3),
            (35.0, 4),
            (49.0, 4),
            (50.0, 5),
            (64.0, 5),
            (65.0, 6),
        ],
    );
    assert_eq!(layout.vertical_offset_for_whitespace_index(0), 20.0);
    assert_eq!(layout.vertical_offset_for_whitespace_index(1), 35.0);
    assert_eq!(layout.vertical_offset_for_whitespace_index(2), 50.0);
    ws_index_at(
        &layout,
        &[
            (0.0, 0),
            (19.0, 0),
            (20.0, 0),
            (21.0, 0),
            (22.0, 0),
            (23.0, 0),
            (24.0, 0),
            (25.0, 1),
            (26.0, 1),
            (34.0, 1),
            (35.0, 1),
            (36.0, 1),
            (39.0, 1),
            (40.0, 2),
            (41.0, 2),
            (49.0, 2),
            (50.0, 2),
            (51.0, 2),
            (54.0, 2),
            (55.0, -1),
            (1000.0, -1),
        ],
    );
}

#[test]
fn lines_layout_2() {
    let mut layout = LinesLayout::new(10, 1.0, 0.0, 0.0, &[]);
    let a = insert(&mut layout, 2, 0, 5, 0);
    assert_eq!(layout.lines_total_height(), 15.0);
    offsets(
        &layout,
        &[(1, 0.0), (2, 1.0), (3, 7.0), (4, 8.0), (10, 14.0)],
    );

    change_one(&mut layout, &a, 2, 10);
    assert_eq!(layout.lines_total_height(), 20.0);
    offsets(
        &layout,
        &[(1, 0.0), (2, 1.0), (3, 12.0), (4, 13.0), (10, 19.0)],
    );

    change_one(&mut layout, &a, 5, 10);
    assert_eq!(layout.lines_total_height(), 20.0);
    offsets(
        &layout,
        &[
            (1, 0.0),
            (2, 1.0),
            (3, 2.0),
            (4, 3.0),
            (5, 4.0),
            (6, 15.0),
            (10, 19.0),
        ],
    );

    // Lines 5 and 6 deleted: 8 lines left.
    layout.on_lines_deleted(5, 6);
    assert_eq!(layout.lines_total_height(), 18.0);
    offsets(
        &layout,
        &[
            (1, 0.0),
            (2, 1.0),
            (3, 2.0),
            (4, 3.0),
            (5, 14.0),
            (6, 15.0),
            (7, 16.0),
            (8, 17.0),
        ],
    );

    // Two lines inserted at the beginning: 10 lines again.
    layout.on_lines_inserted(1, 2);
    assert_eq!(layout.lines_total_height(), 20.0);
    offsets(
        &layout,
        &[
            (1, 0.0),
            (2, 1.0),
            (3, 2.0),
            (4, 3.0),
            (5, 4.0),
            (6, 5.0),
            (7, 16.0),
            (10, 19.0),
        ],
    );

    remove(&mut layout, &a);
    assert_eq!(layout.lines_total_height(), 10.0);
    offsets(
        &layout,
        &[(1, 0.0), (5, 4.0), (6, 5.0), (7, 6.0), (10, 9.0)],
    );
}

#[test]
fn lines_layout_padding() {
    let mut layout = LinesLayout::new(10, 10.0, 15.0, 20.0, &[]);
    assert_eq!(layout.lines_total_height(), 135.0);
    offsets(&layout, &[(1, 15.0), (2, 25.0), (3, 35.0), (10, 105.0)]);
    line_at(
        &layout,
        &[
            (0.0, 1),
            (10.0, 1),
            (15.0, 1),
            (24.0, 1),
            (25.0, 2),
            (34.0, 2),
            (35.0, 3),
        ],
    );

    insert(&mut layout, 2, 0, 5, 0);
    assert_eq!(layout.lines_total_height(), 140.0);
    offsets(&layout, &[(1, 15.0), (2, 25.0), (3, 40.0), (4, 50.0)]);
    line_at(
        &layout,
        &[
            (0.0, 1),
            (10.0, 1),
            (25.0, 2),
            (34.0, 2),
            (35.0, 3),
            (39.0, 3),
            (40.0, 3),
            (41.0, 3),
            (49.0, 3),
            (50.0, 4),
        ],
    );

    insert(&mut layout, 3, 0, 5, 0);
    insert(&mut layout, 4, 0, 5, 0);
    assert_eq!(layout.lines_total_height(), 150.0);
    offsets(
        &layout,
        &[
            (1, 15.0),
            (2, 25.0),
            (3, 40.0),
            (4, 55.0),
            (5, 70.0),
            (6, 80.0),
        ],
    );
    line_at(
        &layout,
        &[
            (0.0, 1),
            (15.0, 1),
            (24.0, 1),
            (30.0, 2),
            (35.0, 3),
            (39.0, 3),
            (40.0, 3),
            (49.0, 3),
            (50.0, 4),
            (54.0, 4),
            (55.0, 4),
            (64.0, 4),
            (65.0, 5),
            (69.0, 5),
            (70.0, 5),
            (80.0, 6),
        ],
    );
    assert_eq!(layout.vertical_offset_for_whitespace_index(0), 35.0);
    assert_eq!(layout.vertical_offset_for_whitespace_index(1), 50.0);
    assert_eq!(layout.vertical_offset_for_whitespace_index(2), 65.0);
    ws_index_at(
        &layout,
        &[
            (0.0, 0),
            (34.0, 0),
            (35.0, 0),
            (39.0, 0),
            (40.0, 1),
            (49.0, 1),
            (50.0, 1),
            (54.0, 1),
            (55.0, 2),
            (64.0, 2),
            (65.0, 2),
            (70.0, -1),
        ],
    );
}

#[test]
fn lines_layout_get_line_number_at_or_after_vertical_offset() {
    let mut layout = LinesLayout::new(10, 1.0, 0.0, 0.0, &[]);
    insert(&mut layout, 6, 0, 10, 0);
    assert_eq!(layout.lines_total_height(), 20.0);
    offsets(
        &layout,
        &[(1, 0.0), (5, 4.0), (6, 5.0), (7, 16.0), (10, 19.0)],
    );

    // line      [1, 2, 3, 4, 5, 6,  7,  8,  9, 10]
    // vertical: [0, 1, 2, 3, 4, 5, 16, 17, 18, 19]
    line_at(
        &layout,
        &[
            (-100.0, 1),
            (-1.0, 1),
            (0.0, 1),
            (1.0, 2),
            (2.0, 3),
            (3.0, 4),
            (4.0, 5),
            (5.0, 6),
            (6.0, 7),
            (7.0, 7),
            (8.0, 7),
            (9.0, 7),
            (10.0, 7),
            (11.0, 7),
            (12.0, 7),
            (13.0, 7),
            (14.0, 7),
            (15.0, 7),
            (16.0, 7),
            (17.0, 8),
            (18.0, 9),
            (19.0, 10),
            (20.0, 10),
            (21.0, 10),
            (22.0, 10),
            (23.0, 10),
        ],
    );
}

#[test]
fn lines_layout_get_centered_line_in_viewport() {
    let mut layout = LinesLayout::new(10, 1.0, 0.0, 0.0, &[]);
    insert(&mut layout, 6, 0, 10, 0);
    offsets(&layout, &[(1, 0.0), (6, 5.0), (7, 16.0), (10, 19.0)]);

    // Viewport 1: the centre is the line containing (start + end) / 2.
    let centered_first = [
        (0.0, 1.0, 1),
        (0.0, 2.0, 2),
        (0.0, 3.0, 2),
        (0.0, 4.0, 3),
        (0.0, 5.0, 3),
        (0.0, 6.0, 4),
        (0.0, 7.0, 4),
        (0.0, 8.0, 5),
        (0.0, 9.0, 5),
        (0.0, 10.0, 6),
        (0.0, 11.0, 6),
        (0.0, 16.0, 6),
        (0.0, 17.0, 7),
        (0.0, 33.0, 7),
    ];
    for (start, end, centered) in centered_first {
        assert_eq!(
            layout.lines_viewport_data(start, end).centered_line_number,
            centered,
            "viewport {start}..{end}"
        );
    }

    // Viewport 2: a fixed end at 20 with a moving start.
    let centered_second = [
        (0, 7),
        (1, 7),
        (2, 7),
        (3, 7),
        (4, 7),
        (5, 7),
        (6, 7),
        (7, 7),
        (8, 7),
        (9, 7),
        (10, 7),
        (11, 7),
        (12, 7),
        (13, 7),
        (14, 8),
        (15, 8),
        (16, 9),
        (17, 9),
        (18, 10),
        (19, 10),
    ];
    for (start, centered) in centered_second {
        assert_eq!(
            layout
                .lines_viewport_data(f64::from(start), 20.0)
                .centered_line_number,
            centered,
            "start {start}"
        );
    }
    assert_eq!(
        layout.lines_viewport_data(20.0, 23.0).centered_line_number,
        10
    );
    assert_eq!(
        layout.lines_viewport_data(21.0, 23.0).centered_line_number,
        10
    );
    assert_eq!(
        layout.lines_viewport_data(22.0, 23.0).centered_line_number,
        10
    );
}

/// `(v1, v2, start, end, completely visible start, completely visible end, relative offsets)`.
type ViewportCase = (f64, f64, i32, i32, i32, i32, &'static [f64]);

fn check_viewports(layout: &LinesLayout, cases: &[ViewportCase]) {
    for &(v1, v2, start, end, cvs, cve, relative) in cases {
        let data = layout.lines_viewport_data(v1, v2);
        let got = (
            data.start_line_number,
            data.end_line_number,
            data.completely_visible_start_line_number,
            data.completely_visible_end_line_number,
        );
        assert_eq!(got, (start, end, cvs, cve), "viewport {v1}..{v2}");
        assert_eq!(
            data.relative_vertical_offset, relative,
            "relative offsets {v1}..{v2}"
        );
    }
}

#[test]
fn lines_layout_get_lines_viewport_data_1() {
    let mut layout = LinesLayout::new(10, 10.0, 0.0, 0.0, &[]);
    insert(&mut layout, 6, 0, 100, 0);
    assert_eq!(layout.lines_total_height(), 200.0);
    offsets(&layout, &[(1, 0.0), (6, 50.0), (7, 160.0), (10, 190.0)]);

    check_viewports(
        &layout,
        &[
            (0.0, 50.0, 1, 5, 1, 5, &[0.0, 10.0, 20.0, 30.0, 40.0]),
            (1.0, 51.0, 1, 6, 2, 5, &[0.0, 10.0, 20.0, 30.0, 40.0, 50.0]),
            (5.0, 55.0, 1, 6, 2, 5, &[0.0, 10.0, 20.0, 30.0, 40.0, 50.0]),
            (10.0, 60.0, 2, 6, 2, 6, &[10.0, 20.0, 30.0, 40.0, 50.0]),
            (50.0, 100.0, 6, 6, 6, 6, &[50.0]),
            (60.0, 110.0, 7, 7, 7, 7, &[160.0]),
            (65.0, 115.0, 7, 7, 7, 7, &[160.0]),
            (50.0, 159.0, 6, 6, 6, 6, &[50.0]),
            (50.0, 160.0, 6, 6, 6, 6, &[50.0]),
            (51.0, 161.0, 6, 7, 7, 7, &[50.0, 160.0]),
            (150.0, 169.0, 7, 7, 7, 7, &[160.0]),
            (159.0, 169.0, 7, 7, 7, 7, &[160.0]),
            (160.0, 169.0, 7, 7, 7, 7, &[160.0]),
            (160.0, 1000.0, 7, 10, 7, 10, &[160.0, 170.0, 180.0, 190.0]),
        ],
    );
}

#[test]
fn lines_layout_get_lines_viewport_data_2_and_get_whitespace_viewport_data() {
    let mut layout = LinesLayout::new(10, 10.0, 0.0, 0.0, &[]);
    let a = insert(&mut layout, 6, 0, 100, 0);
    let b = insert(&mut layout, 7, 0, 50, 0);
    assert_eq!(layout.lines_total_height(), 250.0);
    offsets(
        &layout,
        &[(1, 0.0), (6, 50.0), (7, 160.0), (8, 220.0), (10, 240.0)],
    );

    check_viewports(&layout, &[(50.0, 160.0, 6, 6, 6, 6, &[50.0])]);
    assert_eq!(
        layout.whitespace_viewport_data(50.0, 160.0),
        vec![WhitespaceViewportData {
            id: a.clone(),
            after_line_number: 6,
            vertical_offset: 60.0,
            height: 100
        }]
    );

    check_viewports(&layout, &[(50.0, 219.0, 6, 7, 6, 7, &[50.0, 160.0])]);
    assert_eq!(
        layout.whitespace_viewport_data(50.0, 219.0),
        vec![
            WhitespaceViewportData {
                id: a,
                after_line_number: 6,
                vertical_offset: 60.0,
                height: 100
            },
            WhitespaceViewportData {
                id: b,
                after_line_number: 7,
                vertical_offset: 170.0,
                height: 50
            },
        ]
    );

    check_viewports(&layout, &[(50.0, 220.0, 6, 7, 6, 7, &[50.0, 160.0])]);
    check_viewports(
        &layout,
        &[(
            50.0,
            250.0,
            6,
            10,
            6,
            10,
            &[50.0, 160.0, 220.0, 230.0, 240.0],
        )],
    );
}

#[test]
fn lines_layout_get_whitespace_at_vertical_offset() {
    let mut layout = LinesLayout::new(10, 10.0, 0.0, 0.0, &[]);
    let a = insert(&mut layout, 6, 0, 100, 0);
    let b = insert(&mut layout, 7, 0, 50, 0);

    let id_at = |layout: &LinesLayout, offset: f64| {
        layout.whitespace_at_vertical_offset(offset).map(|w| w.id)
    };
    assert_eq!(id_at(&layout, 0.0), None);
    assert_eq!(id_at(&layout, 59.0), None);
    assert_eq!(id_at(&layout, 60.0), Some(a.clone()));
    assert_eq!(id_at(&layout, 61.0), Some(a.clone()));
    assert_eq!(id_at(&layout, 159.0), Some(a));
    assert_eq!(id_at(&layout, 160.0), None);
    assert_eq!(id_at(&layout, 161.0), None);
    assert_eq!(id_at(&layout, 169.0), None);
    assert_eq!(id_at(&layout, 170.0), Some(b.clone()));
    assert_eq!(id_at(&layout, 171.0), Some(b.clone()));
    assert_eq!(id_at(&layout, 219.0), Some(b));
    assert_eq!(id_at(&layout, 220.0), None);
}

#[test]
fn lines_layout() {
    let mut layout = LinesLayout::new(100, 20.0, 0.0, 0.0, &[]);

    // a(2, 10)
    let a = insert(&mut layout, 2, 0, 10, 0);
    ws_rows(&layout, &[(2, 10, 10.0)]);
    assert_eq!(layout.whitespaces_total_height(), 10.0);
    before_lines(&layout, &[(1, 0.0), (2, 0.0), (3, 10.0), (4, 10.0)]);

    // a(2, 10), b(2, 20)
    let mut b = insert(&mut layout, 2, 0, 20, 0);
    ws_rows(&layout, &[(2, 10, 10.0), (2, 20, 30.0)]);
    assert_eq!(layout.whitespaces_total_height(), 30.0);
    before_lines(&layout, &[(1, 0.0), (2, 0.0), (3, 30.0), (4, 30.0)]);

    // a(2, 10), b(2, 30)
    change_one(&mut layout, &b, 2, 30);
    ws_rows(&layout, &[(2, 10, 10.0), (2, 30, 40.0)]);
    assert_eq!(layout.whitespaces_total_height(), 40.0);
    before_lines(&layout, &[(1, 0.0), (2, 0.0), (3, 40.0), (4, 40.0)]);

    // a(2, 10)
    remove(&mut layout, &b);
    ws_rows(&layout, &[(2, 10, 10.0)]);
    before_lines(&layout, &[(1, 0.0), (2, 0.0), (3, 10.0), (4, 10.0)]);

    // b(0, 50), a(2, 10)
    b = insert(&mut layout, 0, 0, 50, 0);
    ws_rows(&layout, &[(0, 50, 50.0), (2, 10, 60.0)]);
    assert_eq!(layout.whitespaces_total_height(), 60.0);
    before_lines(&layout, &[(1, 50.0), (2, 50.0), (3, 60.0), (4, 60.0)]);

    // b(0, 50), a(2, 10), c(4, 20)
    let c = insert(&mut layout, 4, 0, 20, 0);
    ws_rows(&layout, &[(0, 50, 50.0), (2, 10, 60.0), (4, 20, 80.0)]);
    before_lines(
        &layout,
        &[(1, 50.0), (2, 50.0), (3, 60.0), (4, 60.0), (5, 80.0)],
    );

    // b(0, 50), a(2, 10), d(3, 30), c(4, 20)
    let d = insert(&mut layout, 3, 0, 30, 0);
    ws_rows(
        &layout,
        &[(0, 50, 50.0), (2, 10, 60.0), (3, 30, 90.0), (4, 20, 110.0)],
    );
    before_lines(
        &layout,
        &[(1, 50.0), (2, 50.0), (3, 60.0), (4, 90.0), (5, 110.0)],
    );

    // b(0, 50), a(2, 100), d(3, 30), c(4, 20)
    change_one(&mut layout, &a, 2, 100);
    ws_rows(
        &layout,
        &[
            (0, 50, 50.0),
            (2, 100, 150.0),
            (3, 30, 180.0),
            (4, 20, 200.0),
        ],
    );
    before_lines(
        &layout,
        &[(1, 50.0), (2, 50.0), (3, 150.0), (4, 180.0), (5, 200.0)],
    );

    // b(0, 50), d(3, 30), c(4, 20)
    remove(&mut layout, &a);
    ws_rows(&layout, &[(0, 50, 50.0), (3, 30, 80.0), (4, 20, 100.0)]);
    before_lines(
        &layout,
        &[(1, 50.0), (2, 50.0), (3, 50.0), (4, 80.0), (5, 100.0)],
    );

    // d(3, 30), c(4, 20)
    remove(&mut layout, &b);
    ws_rows(&layout, &[(3, 30, 30.0), (4, 20, 50.0)]);
    before_lines(
        &layout,
        &[(1, 0.0), (2, 0.0), (3, 0.0), (4, 30.0), (5, 50.0)],
    );

    // Delete line 1: d(2, 30), c(3, 20)
    layout.on_lines_deleted(1, 1);
    ws_rows(&layout, &[(2, 30, 30.0), (3, 20, 50.0)]);
    before_lines(
        &layout,
        &[(1, 0.0), (2, 0.0), (3, 30.0), (4, 50.0), (5, 50.0)],
    );

    // Insert a line before line 1: d(3, 30), c(4, 20)
    layout.on_lines_inserted(1, 1);
    ws_rows(&layout, &[(3, 30, 30.0), (4, 20, 50.0)]);
    before_lines(
        &layout,
        &[(1, 0.0), (2, 0.0), (3, 0.0), (4, 30.0), (5, 50.0)],
    );

    // Delete line 4: d(3, 30), c(3, 20)
    layout.on_lines_deleted(4, 4);
    ws_rows(&layout, &[(3, 30, 30.0), (3, 20, 50.0)]);
    before_lines(
        &layout,
        &[(1, 0.0), (2, 0.0), (3, 0.0), (4, 50.0), (5, 50.0)],
    );
    let _ = (c, d);
}

#[test]
fn lines_layout_find_insertion_index() {
    let make = |after: &[i32], ordinal: i32| -> Vec<EditorWhitespace> {
        after
            .iter()
            .map(|&a| EditorWhitespace::new("", a, ordinal, 0, 0))
            .collect()
    };
    // (sorted after-line numbers, ordinal, [(query line, expected index)])
    type Case<'a> = (&'a [i32], i32, &'a [(i32, usize)]);
    let cases: &[Case<'_>] = &[
        (&[], 0, &[(0, 0), (1, 0), (2, 0)]),
        (&[1], 0, &[(0, 0), (1, 1), (2, 1)]),
        (&[1, 3], 0, &[(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)]),
        (
            &[1, 3, 5],
            0,
            &[(0, 0), (1, 1), (2, 1), (3, 2), (4, 2), (5, 3), (6, 3)],
        ),
        (
            &[1, 3, 5],
            3,
            &[(0, 0), (1, 0), (2, 1), (3, 1), (4, 2), (5, 2), (6, 3)],
        ),
        (
            &[1, 3, 5, 7],
            0,
            &[
                (0, 0),
                (1, 1),
                (2, 1),
                (3, 2),
                (4, 2),
                (5, 3),
                (6, 3),
                (7, 4),
                (8, 4),
            ],
        ),
        (&[1, 3, 5, 7, 9], 0, &[(9, 5), (10, 5)]),
        (&[1, 3, 5, 7, 9, 11, 13, 15], 0, &[(15, 8), (16, 8)]),
    ];
    for (after, ordinal, queries) in cases {
        let arr = make(after, *ordinal);
        for &(line, expected) in *queries {
            assert_eq!(
                LinesLayout::find_insertion_index(&arr, line, 0),
                expected,
                "{after:?} @ {line}"
            );
        }
    }
}

#[test]
fn lines_layout_change_whitespace_after_line_number_and_get_first_whitespace_index_after_line_number()
 {
    let mut layout = LinesLayout::new(100, 20.0, 0.0, 0.0, &[]);
    let a = insert(&mut layout, 0, 0, 1, 0);
    let b = insert(&mut layout, 7, 0, 1, 0);
    let c = insert(&mut layout, 3, 0, 1, 0);

    assert_eq!(layout.id_for_whitespace_index(0), a);
    assert_eq!(layout.id_for_whitespace_index(1), c);
    assert_eq!(layout.id_for_whitespace_index(2), b);
    assert_eq!(layout.after_line_number_for_whitespace_index(1), 3);
    assert_eq!(layout.after_line_number_for_whitespace_index(2), 7);

    let first =
        |layout: &LinesLayout, line: i32| layout.first_whitespace_index_after_line_number(line);
    let expect = |layout: &LinesLayout, rows: &[(i32, isize)]| {
        for &(line, index) in rows {
            assert_eq!(
                first(layout, line),
                index,
                "first whitespace index after line {line}"
            );
        }
    };
    expect(
        &layout,
        &[
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 2),
            (5, 2),
            (6, 2),
            (7, 2),
            (8, -1),
        ],
    );

    // Do not really move a.
    change_one(&mut layout, &a, 1, 1);
    assert_eq!(layout.id_for_whitespace_index(0), a);
    assert_eq!(layout.after_line_number_for_whitespace_index(0), 1);
    expect(
        &layout,
        &[
            (1, 0),
            (2, 1),
            (3, 1),
            (4, 2),
            (5, 2),
            (6, 2),
            (7, 2),
            (8, -1),
        ],
    );

    change_one(&mut layout, &a, 2, 1);
    assert_eq!(layout.after_line_number_for_whitespace_index(0), 2);
    expect(
        &layout,
        &[
            (1, 0),
            (2, 0),
            (3, 1),
            (4, 2),
            (5, 2),
            (6, 2),
            (7, 2),
            (8, -1),
        ],
    );

    // Moving a to line 3 places it after c.
    change_one(&mut layout, &a, 3, 1);
    assert_eq!(layout.id_for_whitespace_index(0), c);
    assert_eq!(layout.id_for_whitespace_index(1), a);
    assert_eq!(layout.after_line_number_for_whitespace_index(1), 3);
    expect(
        &layout,
        &[
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 2),
            (5, 2),
            (6, 2),
            (7, 2),
            (8, -1),
        ],
    );

    // A no-op change.
    change_one(&mut layout, &c, 3, 1);
    assert_eq!(layout.id_for_whitespace_index(0), c);
    assert_eq!(layout.id_for_whitespace_index(1), a);
    expect(
        &layout,
        &[
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 2),
            (5, 2),
            (6, 2),
            (7, 2),
            (8, -1),
        ],
    );

    // Moving c to line 7 places it after b.
    change_one(&mut layout, &c, 7, 1);
    assert_eq!(layout.id_for_whitespace_index(0), a);
    assert_eq!(layout.id_for_whitespace_index(1), b);
    assert_eq!(layout.id_for_whitespace_index(2), c);
    expect(
        &layout,
        &[
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 1),
            (5, 1),
            (6, 1),
            (7, 1),
            (8, -1),
        ],
    );
}

#[test]
fn lines_layout_bug() {
    let mut layout = LinesLayout::new(100, 20.0, 0.0, 0.0, &[]);
    let a = insert(&mut layout, 0, 0, 1, 0);
    let b = insert(&mut layout, 7, 0, 1, 0);
    assert_eq!(layout.id_for_whitespace_index(0), a);
    assert_eq!(layout.id_for_whitespace_index(1), b);

    let c = insert(&mut layout, 3, 0, 1, 0);
    assert_eq!(layout.id_for_whitespace_index(1), c);
    assert_eq!(layout.id_for_whitespace_index(2), b);

    let d = insert(&mut layout, 2, 0, 1, 0);
    assert_eq!(layout.id_for_whitespace_index(1), d);
    assert_eq!(layout.id_for_whitespace_index(2), c);
    assert_eq!(layout.id_for_whitespace_index(3), b);

    let e = insert(&mut layout, 8, 0, 1, 0);
    assert_eq!(layout.id_for_whitespace_index(4), e);

    let f = insert(&mut layout, 11, 0, 1, 0);
    assert_eq!(layout.id_for_whitespace_index(5), f);

    let g = insert(&mut layout, 10, 0, 1, 0);
    assert_eq!(layout.id_for_whitespace_index(5), g);
    assert_eq!(layout.id_for_whitespace_index(6), f);

    let h = insert(&mut layout, 0, 0, 1, 0);
    assert_eq!(layout.id_for_whitespace_index(0), a);
    assert_eq!(layout.id_for_whitespace_index(1), h);
    assert_eq!(layout.id_for_whitespace_index(2), d);
    assert_eq!(layout.id_for_whitespace_index(3), c);
    assert_eq!(layout.id_for_whitespace_index(4), b);
    assert_eq!(layout.id_for_whitespace_index(5), e);
    assert_eq!(layout.id_for_whitespace_index(6), g);
    assert_eq!(layout.id_for_whitespace_index(7), f);
}
