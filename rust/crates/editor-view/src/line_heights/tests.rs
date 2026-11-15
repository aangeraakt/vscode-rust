//! Ports of `src/vs/editor/test/common/viewLayout/lineHeights.test.ts`.
//!
//! Test names follow the upstream test titles. Assertions are grouped into tables.

use super::*;

fn heights(manager: &LineHeightsManager, expected: &[(i32, f64)]) {
    for &(line, height) in expected {
        assert_eq!(
            manager.height_for_line_number(line),
            height,
            "height of line {line}"
        );
    }
}

fn accumulated(manager: &LineHeightsManager, expected: &[(i32, f64)]) {
    for &(line, height) in expected {
        assert_eq!(
            manager.accumulated_line_heights_including_line_number(line),
            height,
            "accumulated height through line {line}"
        );
    }
}

#[test]
fn default_line_height_is_used_when_no_custom_heights_exist() {
    let manager = LineHeightsManager::new(10.0, &[]);
    heights(&manager, &[(1, 10.0), (5, 10.0), (100, 10.0)]);
    accumulated(&manager, &[(1, 10.0), (5, 50.0), (10, 100.0)]);
}

#[test]
fn can_change_default_line_height() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.set_default_line_height(20.0);
    heights(&manager, &[(1, 20.0), (5, 20.0)]);
    accumulated(&manager, &[(1, 20.0), (5, 100.0)]);
}

#[test]
fn can_add_single_custom_line_height() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    heights(&manager, &[(1, 10.0), (2, 10.0), (3, 20.0), (4, 10.0)]);
    accumulated(&manager, &[(1, 10.0), (2, 20.0), (3, 40.0), (4, 50.0)]);
}

#[test]
fn can_add_multiple_custom_line_heights() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 2, 2, 15.0);
    manager.insert_or_change_custom_line_height("dec2", 4, 4, 25.0);
    heights(
        &manager,
        &[(1, 10.0), (2, 15.0), (3, 10.0), (4, 25.0), (5, 10.0)],
    );
    accumulated(
        &manager,
        &[(1, 10.0), (2, 25.0), (3, 35.0), (4, 60.0), (5, 70.0)],
    );
}

#[test]
fn can_add_range_of_custom_line_heights() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 2, 4, 15.0);
    heights(
        &manager,
        &[(1, 10.0), (2, 15.0), (3, 15.0), (4, 15.0), (5, 10.0)],
    );
    accumulated(
        &manager,
        &[(1, 10.0), (2, 25.0), (3, 40.0), (4, 55.0), (5, 65.0)],
    );
}

#[test]
fn can_change_existing_custom_line_height() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    assert_eq!(manager.height_for_line_number(3), 20.0);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 30.0);
    heights(&manager, &[(3, 30.0)]);
    accumulated(&manager, &[(3, 50.0), (4, 60.0)]);
}

#[test]
fn can_remove_custom_line_height() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    assert_eq!(manager.height_for_line_number(3), 20.0);
    manager.remove_custom_line_height("dec1");
    heights(&manager, &[(3, 10.0)]);
    accumulated(&manager, &[(3, 30.0), (4, 40.0)]);
}

#[test]
fn handles_overlapping_custom_line_heights_last_one_wins() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 5, 20.0);
    manager.insert_or_change_custom_line_height("dec2", 4, 6, 30.0);
    heights(
        &manager,
        &[
            (2, 10.0),
            (3, 20.0),
            (4, 30.0),
            (5, 30.0),
            (6, 30.0),
            (7, 10.0),
        ],
    );
}

#[test]
fn handles_deleting_lines_before_custom_line_heights() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 10, 12, 20.0);
    manager.on_lines_deleted(5, 7);
    heights(&manager, &[(7, 20.0), (8, 20.0), (9, 20.0), (10, 10.0)]);
}

#[test]
fn handles_deleting_lines_overlapping_with_custom_line_heights() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 5, 10, 20.0);
    manager.on_lines_deleted(7, 12);
    heights(&manager, &[(5, 20.0), (6, 20.0), (7, 10.0)]);
}

#[test]
fn handles_deleting_lines_containing_custom_line_heights_completely() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 5, 7, 20.0);
    manager.on_lines_deleted(4, 8);
    // The decoration collapses to a single line, as the text buffer does.
    heights(&manager, &[(3, 10.0), (4, 20.0), (5, 10.0)]);
}

#[test]
fn handles_deleting_lines_at_the_very_beginning() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("decA", 1, 1, 40.0);
    manager.on_lines_deleted(2, 4);
    heights(&manager, &[(1, 40.0)]);
}

#[test]
fn handles_inserting_lines_before_custom_line_heights() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 5, 7, 20.0);
    manager.on_lines_inserted(3, 4);
    heights(
        &manager,
        &[(5, 10.0), (6, 10.0), (7, 20.0), (8, 20.0), (9, 20.0)],
    );
}

#[test]
fn handles_inserting_lines_inside_custom_line_heights_range() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 5, 7, 20.0);
    manager.on_lines_inserted(6, 7);
    heights(
        &manager,
        &[(5, 20.0), (6, 20.0), (7, 20.0), (8, 20.0), (9, 20.0)],
    );
}

#[test]
fn changing_decoration_id_maintains_custom_line_height() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 5, 7, 20.0);
    manager.remove_custom_line_height("dec1");
    manager.insert_or_change_custom_line_height("dec2", 5, 7, 20.0);
    heights(&manager, &[(5, 20.0), (6, 20.0), (7, 20.0)]);
}

#[test]
fn accumulates_heights_correctly_with_complex_setup() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 15.0);
    manager.insert_or_change_custom_line_height("dec2", 5, 7, 20.0);
    manager.insert_or_change_custom_line_height("dec3", 10, 10, 30.0);
    accumulated(
        &manager,
        &[
            (1, 10.0),
            (2, 20.0),
            (3, 35.0),
            (4, 45.0),
            (5, 65.0),
            (7, 105.0),
            (9, 125.0),
            (10, 155.0),
        ],
    );
}

#[test]
fn partial_deletion_with_multiple_lines_for_the_same_decoration_id() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("decSame", 5, 5, 20.0);
    manager.insert_or_change_custom_line_height("decSame", 6, 6, 25.0);
    manager.on_lines_deleted(6, 6);
    heights(&manager, &[(5, 10.0), (6, 25.0)]);
}

#[test]
fn overlapping_decorations_use_maximum_line_height() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("decA", 3, 5, 40.0);
    manager.insert_or_change_custom_line_height("decB", 4, 6, 30.0);
    heights(&manager, &[(3, 40.0), (4, 40.0), (5, 40.0), (6, 30.0)]);
}

#[test]
fn on_lines_inserted_with_same_decoration_id_extending_to_inserted_line() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("decA", 1, 1, 30.0);
    heights(&manager, &[(1, 30.0), (2, 10.0)]);

    manager.on_lines_inserted(2, 2);
    manager.insert_or_change_custom_line_height("decA", 2, 2, 30.0);

    // insertOrChange removes the old decoration first, so line 1 loses its height.
    heights(&manager, &[(1, 10.0), (2, 30.0), (3, 10.0)]);
}

#[test]
fn constructor_with_initial_data_works_without_explicit_commit() {
    let data = [
        CustomLineHeightData::new("dec1", 2, 4, 20.0),
        CustomLineHeightData::new("dec2", 6, 6, 30.0),
    ];
    let manager = LineHeightsManager::new(10.0, &data);
    heights(
        &manager,
        &[
            (1, 10.0),
            (2, 20.0),
            (3, 20.0),
            (4, 20.0),
            (5, 10.0),
            (6, 30.0),
        ],
    );
    accumulated(&manager, &[(6, 110.0)]);
}

// Ported from the upstream suite "LineHeightsManager (auto-commit on read)". Changes are applied
// immediately in this port, so every read sees all earlier changes.

#[test]
fn read_after_single_insert_without_commit() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    heights(&manager, &[(1, 10.0), (3, 20.0), (4, 10.0)]);
    accumulated(&manager, &[(3, 40.0), (4, 50.0)]);
}

#[test]
fn read_after_multiple_inserts_without_commit() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 2, 2, 15.0);
    manager.insert_or_change_custom_line_height("dec2", 4, 4, 25.0);
    heights(&manager, &[(2, 15.0), (3, 10.0), (4, 25.0)]);
    accumulated(&manager, &[(4, 60.0), (5, 70.0)]);
}

#[test]
fn read_after_remove_without_commit() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.remove_custom_line_height("dec1");
    heights(&manager, &[(3, 10.0)]);
    accumulated(&manager, &[(3, 30.0)]);
}

#[test]
fn insert_then_remove_same_decoration_without_commit() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.remove_custom_line_height("dec1");
    heights(&manager, &[(3, 10.0)]);
    accumulated(&manager, &[(3, 30.0)]);
}

#[test]
fn insert_same_decoration_id_twice_without_commit_replaces_first() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.insert_or_change_custom_line_height("dec1", 5, 5, 30.0);
    heights(&manager, &[(3, 10.0), (5, 30.0)]);
    accumulated(&manager, &[(5, 70.0)]);
}

#[test]
fn interleaved_callers_remove_must_cancel_queued_inserts_before_first_flush() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("decA", 3, 3, 20.0);
    manager.insert_or_change_custom_line_height("decB", 4, 4, 30.0);
    manager.remove_custom_line_height("decA");
    manager.on_lines_inserted(1, 1);
    heights(&manager, &[(4, 10.0), (5, 30.0)]);
    accumulated(&manager, &[(5, 70.0)]);
}

#[test]
fn interleaved_callers_remove_must_cancel_queued_inserts_before_delete_flush() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("decA", 3, 3, 20.0);
    manager.insert_or_change_custom_line_height("decB", 5, 5, 30.0);
    manager.remove_custom_line_height("decA");
    manager.on_lines_deleted(1, 1);
    heights(&manager, &[(2, 10.0), (3, 10.0), (4, 30.0)]);
    accumulated(&manager, &[(4, 60.0)]);
}

#[test]
fn interleaved_insert_insert_inserted_deleted_remove_remove_insert_insert_read() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 2, 2, 20.0);
    manager.insert_or_change_custom_line_height("dec2", 5, 5, 30.0);
    manager.on_lines_inserted(3, 4);
    manager.on_lines_deleted(1, 1);
    manager.remove_custom_line_height("dec1");
    manager.remove_custom_line_height("dec2");
    manager.insert_or_change_custom_line_height("dec3", 3, 3, 40.0);
    manager.insert_or_change_custom_line_height("dec4", 5, 5, 50.0);
    heights(
        &manager,
        &[(1, 10.0), (3, 40.0), (4, 10.0), (5, 50.0), (6, 10.0)],
    );
}

#[test]
fn interleaved_insert_on_lines_inserted_remove_read() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.on_lines_inserted(1, 1);
    manager.remove_custom_line_height("dec1");
    heights(&manager, &[(3, 10.0), (4, 10.0)]);
}

#[test]
fn interleaved_on_lines_deleted_insert_read() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 5, 5, 20.0);
    manager.on_lines_deleted(1, 2);
    manager.insert_or_change_custom_line_height("dec2", 1, 1, 30.0);
    heights(&manager, &[(1, 30.0), (2, 10.0), (3, 20.0)]);
}

#[test]
fn interleaved_insert_on_lines_deleted_insert_read() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.on_lines_deleted(1, 1);
    manager.insert_or_change_custom_line_height("dec2", 5, 5, 30.0);
    heights(&manager, &[(1, 10.0), (2, 20.0), (5, 30.0)]);
    accumulated(&manager, &[(5, 80.0)]);
}

#[test]
fn on_lines_inserted_then_on_lines_deleted_without_reads_between() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.on_lines_inserted(1, 2);
    manager.on_lines_deleted(1, 1);
    heights(&manager, &[(3, 10.0), (4, 20.0)]);
    accumulated(&manager, &[(4, 50.0)]);
}

#[test]
fn multiple_on_lines_inserted_without_reads_between() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.on_lines_inserted(1, 1);
    manager.on_lines_inserted(1, 1);
    heights(&manager, &[(3, 10.0), (4, 10.0), (5, 20.0)]);
}

#[test]
fn multiple_on_lines_deleted_without_reads_between() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 10, 10, 20.0);
    manager.on_lines_deleted(1, 2);
    manager.on_lines_deleted(1, 2);
    heights(&manager, &[(6, 20.0), (7, 10.0)]);
}

#[test]
fn pending_insert_then_on_lines_deleted_affecting_that_line() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.on_lines_deleted(3, 3);
    // The decoration collapses to line 3 (fromLineNumber), as onLinesDeleted does.
    heights(&manager, &[(3, 20.0)]);
}

#[test]
fn pending_insert_then_on_lines_inserted_shifting_that_line() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 3, 3, 20.0);
    manager.on_lines_inserted(1, 2);
    heights(&manager, &[(3, 10.0), (5, 20.0)]);
}

#[test]
fn accumulated_heights_correct_after_interleaved_ops_without_commit() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 2, 2, 15.0);
    manager.insert_or_change_custom_line_height("dec2", 4, 4, 25.0);
    accumulated(
        &manager,
        &[(1, 10.0), (2, 25.0), (3, 35.0), (4, 60.0), (5, 70.0)],
    );
}

#[test]
fn deleting_line_2_then_re_adding_at_line_1_moves_special_line_to_line_1() {
    let mut manager = LineHeightsManager::new(10.0, &[]);
    manager.insert_or_change_custom_line_height("dec1", 2, 2, 20.0);
    assert_eq!(manager.height_for_line_number(2), 20.0);
    manager.on_lines_deleted(2, 2);
    manager.insert_or_change_custom_line_height("dec1", 1, 1, 20.0);
    heights(&manager, &[(1, 20.0)]);
}
