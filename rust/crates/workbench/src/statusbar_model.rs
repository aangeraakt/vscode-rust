//! Status bar view model: entry order, alignment, priority, hidden entries.
//!
//! Ported from:
//! - `src/vs/workbench/browser/parts/statusbar/statusbarModel.ts` (`StatusbarViewModel`)
//! - `src/vs/workbench/services/statusbar/browser/statusbar.ts` (`StatusbarAlignment`,
//!   `IStatusbarEntryLocation`, `IStatusbarEntryPriority`)
//! - `src/vs/workbench/browser/parts/statusbar/statusbarPart.ts` (`updateCompactEntries`, the
//!   grouping part only; see [`StatusbarViewModel::compact_entries`])
//!
//! Tests port `src/vs/workbench/test/browser/parts/statusbar/statusbarModel.test.ts` one for one
//! (same names, snake_cased); the tests after them cover behavior the upstream suite leaves
//! untested.
//!
//! Intentional differences from the TypeScript:
//! - Entries are identified by an [`EntryHandle`] returned from [`StatusbarViewModel::add`]
//!   instead of object identity; `findEntry(container)` becomes [`StatusbarViewModel::find_entry`]
//!   by handle. Ids may repeat, as upstream.
//! - DOM work (showing and hiding containers, `first-visible-item` / `last-visible-item` and
//!   `compact-left` / `compact-right` classes, focusing label containers) is left to the view.
//!   The model answers the same questions: [`StatusbarViewModel::first_last_visible`],
//!   [`StatusbarViewModel::compact_entries`], and the focus methods take the currently focused
//!   entry as an argument and return the entry to focus (`isEntryFocused` is not ported).
//! - Persistence: the storage service is M5 and does not exist yet. The hidden set lives in
//!   memory. [`StatusbarViewModel::with_hidden_state`] takes what storage would return for
//!   [`HIDDEN_ENTRIES_KEY`], [`StatusbarViewModel::set_save_hook`] receives what upstream would
//!   store (`None` means remove the key), and [`StatusbarViewModel::on_did_storage_value_change`]
//!   is the handler for external changes of that key.
//! - `restoreState` uses `new Set(JSON.parse(raw))`. Matched: an array keeps its string items, a
//!   string becomes its characters, anything else gives an empty set. Not matched: non-string
//!   array items are dropped (upstream keeps them in the set, where they never match an id).
//! - Priorities are `f64`. When comparing, equal values (including `0` and `-0`) tie; otherwise
//!   values are ordered with `f64::total_cmp`, so a NaN priority sorts before every number
//!   (upstream's comparator returns NaN, which leaves the order engine-defined).
//! - `lastFocusedEntry` returns `None` once that entry was removed (upstream keeps returning the
//!   removed object).
//!
//! Matched quirk: during sorting, relative entries are grouped by id, so when two entries with
//! the same id are positioned relative to the same existing entry, only the later one is kept
//! (see `relative_entries_with_the_same_id_keep_only_the_last`).
//! Also matched: grouping visits entries in their current order, so an entry placed left of a
//! relative entry it references ends up appended at the end on the next sort (see
//! `compact_entries_group_chained_locations_of_visible_entries`).

use std::cmp::Ordering;
use std::collections::HashSet;

use serde_json::Value;
use vscode_api::{Emitter, Event};

/// Storage key under which upstream persists the hidden entry ids (profile scope, user target).
pub const HIDDEN_ENTRIES_KEY: &str = "workbench.statusbar.hidden";

/// Which side of the status bar an entry sits on (`StatusbarAlignment`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatusbarAlignment {
    /// Left side.
    Left,
    /// Right side.
    Right,
}

/// Positions an entry next to another entry (`IStatusbarEntryLocation`).
#[derive(Clone, Debug, PartialEq)]
pub struct StatusbarEntryLocation {
    /// Id of the entry to position relative to (`location.id`).
    pub id: String,
    /// Priority used when the referenced entry does not exist (`location.priority`).
    pub priority: f64,
    /// Whether to sit left or right of the referenced entry.
    pub alignment: StatusbarAlignment,
    /// Whether to appear as one item with the referenced entry.
    pub compact: bool,
}

/// The main priority of an entry: a number, or a position relative to another entry.
#[derive(Clone, Debug, PartialEq)]
pub enum StatusbarPrimaryPriority {
    /// Higher numbers move towards the left.
    Number(f64),
    /// Next to another entry.
    Location(StatusbarEntryLocation),
}

impl StatusbarPrimaryPriority {
    fn location(&self) -> Option<&StatusbarEntryLocation> {
        match self {
            Self::Number(_) => None,
            Self::Location(location) => Some(location),
        }
    }

    fn as_number(&self) -> f64 {
        match self {
            Self::Number(n) => *n,
            Self::Location(location) => location.priority,
        }
    }
}

/// `IStatusbarEntryPriority`.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusbarEntryPriority {
    /// Order of appearance; may repeat across entries.
    pub primary: StatusbarPrimaryPriority,
    /// Tie breaker when primary priorities are equal; higher moves towards the left.
    pub secondary: f64,
}

/// What a caller adds to the model (`IStatusbarViewModelEntry` without the DOM containers).
#[derive(Clone, Debug, PartialEq)]
pub struct StatusbarEntry {
    /// Entry id; not unique.
    pub id: String,
    /// Contributing extension, if any.
    pub extension_id: Option<String>,
    /// User-facing name.
    pub name: String,
    /// Whether clicking the entry runs a command.
    pub has_command: bool,
    /// Side of the status bar.
    pub alignment: StatusbarAlignment,
    /// Sort priority.
    pub priority: StatusbarEntryPriority,
}

/// Identifies one added entry; stands in for upstream's object identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryHandle(u64);

/// An entry held by the model.
#[derive(Clone, Debug)]
pub struct StatusbarViewModelEntry {
    handle: EntryHandle,
    /// The entry as added.
    pub entry: StatusbarEntry,
}

impl StatusbarViewModelEntry {
    /// The handle [`StatusbarViewModel::add`] returned for this entry.
    pub fn handle(&self) -> EntryHandle {
        self.handle
    }

    /// The entry id.
    pub fn id(&self) -> &str {
        &self.entry.id
    }
}

/// Payload of `onDidChangeEntryVisibility`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryVisibilityChange {
    /// Entry id.
    pub id: String,
    /// Whether the entry is now visible.
    pub visible: bool,
}

/// Entries that belong together because of `compact` locations (from `updateCompactEntries`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompactEntries {
    /// Each group of entries that share hover feedback, in upstream's map order.
    pub groups: Vec<Vec<EntryHandle>>,
    /// Entries that get the `compact-left` class.
    pub compact_left: HashSet<EntryHandle>,
    /// Entries that get the `compact-right` class.
    pub compact_right: HashSet<EntryHandle>,
}

type SaveHook = Box<dyn FnMut(Option<&str>) + Send>;

/// `StatusbarViewModel`: the sorted entries of both sides plus the hidden set.
pub struct StatusbarViewModel {
    entries: Vec<StatusbarViewModelEntry>,
    hidden: Vec<String>,
    last_focused: Option<EntryHandle>,
    next_handle: u64,
    save_hook: Option<SaveHook>,
    on_did_change_entry_visibility: Emitter<EntryVisibilityChange>,
}

impl Default for StatusbarViewModel {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusbarViewModel {
    /// A model with no entries and nothing hidden.
    pub fn new() -> Self {
        Self::with_hidden_state(None)
    }

    /// A model whose hidden set is restored from the stored value of [`HIDDEN_ENTRIES_KEY`].
    pub fn with_hidden_state(raw: Option<&str>) -> Self {
        Self {
            entries: Vec::new(),
            hidden: parse_hidden(raw),
            last_focused: None,
            next_handle: 0,
            save_hook: None,
            on_did_change_entry_visibility: Emitter::new(),
        }
    }

    /// Called with the new stored value whenever the hidden set changes (`saveState`).
    pub fn set_save_hook(&mut self, hook: impl FnMut(Option<&str>) + Send + 'static) {
        self.save_hook = Some(Box::new(hook));
    }

    /// The value upstream would store for [`HIDDEN_ENTRIES_KEY`]; `None` means no key.
    pub fn hidden_state(&self) -> Option<String> {
        if self.hidden.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&self.hidden).unwrap_or_default())
        }
    }

    /// Fires when `hide`, `show` or a storage change flips an entry's visibility.
    pub fn on_did_change_entry_visibility(&self) -> Event<EntryVisibilityChange> {
        self.on_did_change_entry_visibility.event()
    }

    /// All entries, sorted: left side first, then right side.
    pub fn entries(&self) -> &[StatusbarViewModelEntry] {
        &self.entries
    }

    /// The entry last focused through the focus methods, unless it is hidden or removed.
    pub fn last_focused_entry(&self) -> Option<&StatusbarViewModelEntry> {
        let entry = self.find_entry(self.last_focused?)?;
        (!self.is_hidden(entry.id())).then_some(entry)
    }

    /// Adds an entry and re-sorts.
    pub fn add(&mut self, entry: StatusbarEntry) -> EntryHandle {
        let handle = EntryHandle(self.next_handle);
        self.next_handle += 1;
        self.entries.push(StatusbarViewModelEntry { handle, entry });
        self.sort();
        handle
    }

    /// Removes an entry; re-sorts when another entry was positioned relative to its id.
    pub fn remove(&mut self, handle: EntryHandle) -> bool {
        let Some(index) = self.index_of(handle) else {
            return false;
        };
        let removed = self.entries.remove(index);
        let referenced = self.entries.iter().any(|other| {
            other
                .entry
                .priority
                .primary
                .location()
                .is_some_and(|location| location.id == removed.entry.id)
        });
        if referenced {
            self.sort();
        }
        true
    }

    /// Whether entries with this id are hidden.
    pub fn is_hidden(&self, id: &str) -> bool {
        self.hidden.iter().any(|h| h == id)
    }

    /// Hides every entry with this id, now and when added later.
    pub fn hide(&mut self, id: &str) {
        if !self.is_hidden(id) {
            self.hidden.push(id.to_owned());
            self.fire_visibility(id);
            self.save_state();
        }
    }

    /// Shows entries with this id again.
    pub fn show(&mut self, id: &str) {
        if let Some(pos) = self.hidden.iter().position(|h| h == id) {
            self.hidden.remove(pos);
            self.fire_visibility(id);
            self.save_state();
        }
    }

    /// The entry with this handle (`findEntry`).
    pub fn find_entry(&self, handle: EntryHandle) -> Option<&StatusbarViewModelEntry> {
        self.entries.iter().find(|e| e.handle == handle)
    }

    /// The sorted entries of one side.
    pub fn get_entries(&self, alignment: StatusbarAlignment) -> Vec<&StatusbarViewModelEntry> {
        self.entries
            .iter()
            .filter(|e| e.entry.alignment == alignment)
            .collect()
    }

    /// The next visible entry after `focused`, wrapping to the first (`focusNextEntry`).
    pub fn focus_next_entry(&mut self, focused: Option<EntryHandle>) -> Option<EntryHandle> {
        self.focus_entry(focused, 1, 0)
    }

    /// The previous visible entry before `focused`, wrapping to the last
    /// (`focusPreviousEntry`).
    pub fn focus_previous_entry(&mut self, focused: Option<EntryHandle>) -> Option<EntryHandle> {
        let last = self.entries.len() as isize - 1;
        self.focus_entry(focused, -1, last)
    }

    /// First and last visible entry of one side (`markFirstLastVisibleEntry`).
    pub fn first_last_visible(
        &self,
        alignment: StatusbarAlignment,
    ) -> (Option<EntryHandle>, Option<EntryHandle>) {
        let mut visible = self
            .entries
            .iter()
            .filter(|e| e.entry.alignment == alignment && !self.is_hidden(e.id()))
            .map(|e| e.handle);
        let first = visible.next();
        (first, visible.next_back().or(first))
    }

    /// Reloads the hidden set from a changed stored value and fires one visibility event per
    /// entry whose id changed state (`onDidStorageValueChange`).
    pub fn on_did_storage_value_change(&mut self, raw: Option<&str>) {
        let previously = std::mem::replace(&mut self.hidden, parse_hidden(raw));
        let mut changed: Vec<String> = previously
            .iter()
            .filter(|id| !self.is_hidden(id))
            .cloned()
            .collect();
        for id in &self.hidden {
            if !previously.contains(id) && !changed.contains(id) {
                changed.push(id.clone());
            }
        }
        let ids: Vec<String> = self.entries.iter().map(|e| e.entry.id.clone()).collect();
        for id in ids {
            if let Some(pos) = changed.iter().position(|c| *c == id) {
                self.fire_visibility(&id);
                changed.remove(pos);
            }
        }
    }

    /// Groups of visible entries joined by `compact` locations and the side classes each gets.
    pub fn compact_entries(&self) -> CompactEntries {
        // `mapIdToVisibleEntry`: first insertion position, last entry wins.
        let mut visible: Vec<(&str, usize)> = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            if self.is_hidden(entry.id()) {
                continue;
            }
            match visible.iter_mut().find(|(id, _)| *id == entry.id()) {
                Some(slot) => slot.1 = index,
                None => visible.push((entry.id(), index)),
            }
        }
        let lookup = |id: &str| visible.iter().find(|(v, _)| *v == id).map(|(_, i)| *i);

        let mut groups: Vec<(&str, Vec<(&str, usize)>)> = Vec::new();
        let mut result = CompactEntries::default();
        for &(_, index) in &visible {
            let entry = &self.entries[index];
            let Some(location) = entry.entry.priority.primary.location() else {
                continue;
            };
            if !location.compact {
                continue;
            }
            let Some(location_index) = lookup(&location.id) else {
                continue;
            };
            let target = &self.entries[location_index];
            let group = group_for(&mut groups, &location.id);
            insert_keyed(group, entry.id(), index);
            insert_keyed(group, target.id(), location_index);
            let (left, right) = match location.alignment {
                StatusbarAlignment::Left => (target.handle, entry.handle),
                StatusbarAlignment::Right => (entry.handle, target.handle),
            };
            result.compact_left.insert(left);
            result.compact_right.insert(right);
        }
        result.groups = groups
            .into_iter()
            .map(|(_, members)| {
                members
                    .into_iter()
                    .map(|(_, i)| self.entries[i].handle)
                    .collect()
            })
            .collect();
        result
    }

    fn index_of(&self, handle: EntryHandle) -> Option<usize> {
        self.entries.iter().position(|e| e.handle == handle)
    }

    fn fire_visibility(&self, id: &str) {
        let visible = !self.is_hidden(id);
        for entry in &self.entries {
            if entry.id() == id {
                self.on_did_change_entry_visibility
                    .fire(EntryVisibilityChange {
                        id: id.to_owned(),
                        visible,
                    });
            }
        }
    }

    fn save_state(&mut self) {
        let state = self.hidden_state();
        if let Some(hook) = self.save_hook.as_mut() {
            hook(state.as_deref());
        }
    }

    fn focus_entry(
        &mut self,
        focused: Option<EntryHandle>,
        delta: isize,
        restart: isize,
    ) -> Option<EntryHandle> {
        let visible_from = |start: isize| {
            let mut index = start;
            while index >= 0 && (index as usize) < self.entries.len() {
                let entry = &self.entries[index as usize];
                if !self.is_hidden(entry.id()) {
                    return Some(entry.handle);
                }
                index += delta;
            }
            None
        };
        let next = focused
            .and_then(|h| self.index_of(h))
            .and_then(|index| visible_from(index as isize + delta))
            .or_else(|| visible_from(restart));
        if next.is_some() {
            self.last_focused = next;
        }
        next
    }

    fn sort(&mut self) {
        let entries = std::mem::take(&mut self.entries);
        let all_ids: HashSet<&str> = entries.iter().map(|e| e.id()).collect();

        // Bucket 1: numbered priority, or a location whose entry does not exist.
        // Bucket 2: relative to an existing entry, grouped by the referenced id.
        let mut numbered: Vec<usize> = Vec::new();
        let mut relative: Vec<(&str, Vec<(&str, usize)>)> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            match entry.entry.priority.primary.location() {
                Some(location) if all_ids.contains(location.id.as_str()) => {
                    let group = group_for(&mut relative, &location.id);
                    insert_keyed(group, entry.id(), index);
                }
                _ => numbered.push(index),
            }
        }

        let priority = |i: usize| &entries[i].entry.priority;
        numbered.sort_by(|&a, &b| {
            let (alignment_a, alignment_b) =
                (entries[a].entry.alignment, entries[b].entry.alignment);
            if alignment_a == alignment_b {
                descending(
                    priority(a).primary.as_number(),
                    priority(b).primary.as_number(),
                )
                .then_with(|| descending(priority(a).secondary, priority(b).secondary))
                .then(a.cmp(&b))
            } else if alignment_a == StatusbarAlignment::Left {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        });

        let by_secondary = |members: &[(&str, usize)], side: Option<StatusbarAlignment>| {
            let mut picked: Vec<usize> = members
                .iter()
                .map(|(_, i)| *i)
                .filter(|&i| {
                    side.is_none_or(|side| {
                        priority(i)
                            .primary
                            .location()
                            .is_some_and(|l| l.alignment == side)
                    })
                })
                .collect();
            picked.sort_by(|&a, &b| descending(priority(a).secondary, priority(b).secondary));
            picked
        };

        let order: Vec<usize> = if relative.is_empty() {
            numbered
        } else {
            let mut order = Vec::with_capacity(entries.len());
            for &index in &numbered {
                let pos = relative.iter().position(|(r, _)| *r == entries[index].id());
                let group = pos.map(|p| relative.remove(p).1);
                if let Some(members) = &group {
                    order.extend(by_secondary(members, Some(StatusbarAlignment::Left)));
                }
                order.push(index);
                if let Some(members) = &group {
                    order.extend(by_secondary(members, Some(StatusbarAlignment::Right)));
                }
            }
            for (_, members) in &relative {
                order.extend(by_secondary(members, None));
            }
            order
        };

        let mut slots: Vec<Option<StatusbarViewModelEntry>> =
            entries.iter().cloned().map(Some).collect();
        self.entries = order.into_iter().filter_map(|i| slots[i].take()).collect();
    }
}

/// The group keyed by `reference`, else the first group that contains `reference` as a member
/// (chained references), else a new group.
fn group_for<'a, 'b>(
    groups: &'b mut Vec<(&'a str, Vec<(&'a str, usize)>)>,
    reference: &'a str,
) -> &'b mut Vec<(&'a str, usize)> {
    let pos = groups
        .iter()
        .position(|(key, _)| *key == reference)
        .or_else(|| {
            groups
                .iter()
                .position(|(_, members)| members.iter().any(|(id, _)| *id == reference))
        })
        .unwrap_or_else(|| {
            groups.push((reference, Vec::new()));
            groups.len() - 1
        });
    &mut groups[pos].1
}

/// `Map.set`: replaces the value of an existing key in place, else appends.
fn insert_keyed<'a>(members: &mut Vec<(&'a str, usize)>, id: &'a str, index: usize) {
    match members.iter_mut().find(|(m, _)| *m == id) {
        Some(slot) => slot.1 = index,
        None => members.push((id, index)),
    }
}

/// Higher values first; equal values (`==`) tie.
fn descending(a: f64, b: f64) -> Ordering {
    if a == b {
        Ordering::Equal
    } else {
        b.total_cmp(&a)
    }
}

/// `new Set(JSON.parse(raw))`, with parse and iteration errors giving an empty set.
fn parse_hidden(raw: Option<&str>) -> Vec<String> {
    let mut hidden: Vec<String> = Vec::new();
    let items: Vec<String> = match raw.filter(|r| !r.is_empty()).map(serde_json::from_str) {
        Some(Ok(Value::Array(items))) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Some(Ok(Value::String(s))) => s.chars().map(String::from).collect(),
        _ => Vec::new(),
    };
    for item in items {
        if !hidden.contains(&item) {
            hidden.push(item);
        }
    }
    hidden
}

#[cfg(test)]
mod tests {
    use super::StatusbarAlignment::{Left, Right};
    use super::*;
    use std::sync::{Arc, Mutex};

    fn entry(
        id: &str,
        alignment: StatusbarAlignment,
        primary: f64,
        secondary: f64,
    ) -> StatusbarEntry {
        StatusbarEntry {
            id: id.to_owned(),
            extension_id: None,
            name: id.to_owned(),
            has_command: false,
            alignment,
            priority: StatusbarEntryPriority {
                primary: StatusbarPrimaryPriority::Number(primary),
                secondary,
            },
        }
    }

    fn relative(
        id: &str,
        alignment: StatusbarAlignment,
        (location, location_priority): (&str, f64),
        location_alignment: StatusbarAlignment,
        secondary: f64,
    ) -> StatusbarEntry {
        let mut e = entry(id, alignment, 0.0, secondary);
        e.priority.primary = StatusbarPrimaryPriority::Location(StatusbarEntryLocation {
            id: location.to_owned(),
            priority: location_priority,
            alignment: location_alignment,
            compact: false,
        });
        e
    }

    fn compact(mut e: StatusbarEntry) -> StatusbarEntry {
        if let StatusbarPrimaryPriority::Location(l) = &mut e.priority.primary {
            l.compact = true;
        }
        e
    }

    fn ids(model: &StatusbarViewModel) -> Vec<&str> {
        model.entries().iter().map(|e| e.id()).collect()
    }

    fn record(
        model: &StatusbarViewModel,
    ) -> (
        Arc<Mutex<Vec<EntryVisibilityChange>>>,
        vscode_api::Disposable,
    ) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s = seen.clone();
        let d = model
            .on_did_change_entry_visibility()
            .subscribe(move |e| s.lock().unwrap().push(e.clone()));
        (seen, d)
    }

    #[test]
    fn basics() {
        let mut model = StatusbarViewModel::new();
        assert_eq!(model.entries().len(), 0);

        let entry1 = model.add(entry("3", Left, 3.0, 1.0));
        let entry2 = model.add(entry("2", Left, 2.0, 1.0));
        let entry3 = model.add(entry("1", Left, 1.0, 1.0));
        let entry4 = model.add(entry("1-right", Right, 1.0, 1.0));
        assert_eq!(model.entries().len(), 4);

        let left: Vec<&str> = model.get_entries(Left).iter().map(|e| e.id()).collect();
        assert_eq!(left, ["3", "2", "1"]);
        assert_eq!(model.get_entries(Right).len(), 1);
        assert_eq!(ids(&model), ["3", "2", "1", "1-right"]);
        assert!(model.find_entry(entry1).is_some());

        let (seen, _d) = record(&model);
        assert!(!model.is_hidden("1"));
        model.hide("1");
        assert_eq!(
            seen.lock().unwrap().last(),
            Some(&EntryVisibilityChange {
                id: "1".into(),
                visible: false
            })
        );
        assert!(model.is_hidden("1"));

        model.show("1");
        assert_eq!(
            seen.lock().unwrap().last(),
            Some(&EntryVisibilityChange {
                id: "1".into(),
                visible: true
            })
        );
        assert!(!model.is_hidden("1"));

        model.remove(entry1);
        model.remove(entry4);
        assert_eq!(model.entries().len(), 2);
        model.remove(entry2);
        model.remove(entry3);
        assert_eq!(model.entries().len(), 0);
    }

    #[test]
    fn sorting_with_infinity_and_max_number() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("3", Left, f64::MAX, 1.0));
        // `Number.MIN_VALUE` is the smallest positive number.
        model.add(entry("2", Left, f64::from_bits(1), 1.0));
        model.add(entry("1", Left, f64::INFINITY, 1.0));
        model.add(entry("0", Left, f64::NEG_INFINITY, 1.0));
        model.add(entry("4", Left, 100.0, 1.0));
        assert_eq!(ids(&model), ["1", "3", "4", "2", "0"]);
    }

    #[test]
    fn secondary_priority_used_when_primary_is_same() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("1", Left, 1.0, 1.0));
        model.add(entry("2", Left, 1.0, 2.0));
        model.add(entry("3", Left, 1.0, 3.0));
        assert_eq!(ids(&model), ["3", "2", "1"]);
    }

    #[test]
    fn insertion_order_preserved_when_priorities_are_the_same() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("1", Left, 1.0, 1.0));
        model.add(entry("2", Left, 1.0, 1.0));
        model.add(entry("3", Left, 1.0, 1.0));
        assert_eq!(ids(&model), ["1", "2", "3"]);
    }

    #[test]
    fn entry_with_reference_to_other_entry_existing() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("a", Left, 2.0, 1.0));
        model.add(entry("b", Left, 1.0, 1.0));
        let c = model.add(relative("c", Left, ("a", 2.0), Left, 1.0));
        assert_eq!(ids(&model), ["c", "a", "b"]);
        model.remove(c);

        model.add(relative("c", Right, ("a", 2.0), Right, 1.0));
        assert_eq!(ids(&model), ["a", "c", "b"]);
    }

    #[test]
    fn entry_with_reference_to_other_entry_nonexistent() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("a", Left, 2.0, 1.0));
        model.add(entry("b", Left, 1.0, 1.0));
        let c = model.add(relative("c", Left, ("not-existing", 0.0), Left, 1.0));
        assert_eq!(ids(&model), ["a", "b", "c"]);
        model.remove(c);

        let c = model.add(relative("c", Left, ("not-existing", 3.0), Left, 1.0));
        assert_eq!(ids(&model), ["c", "a", "b"]);
        model.remove(c);

        model.add(relative("c", Right, ("not-existing", 3.0), Right, 1.0));
        assert_eq!(ids(&model), ["a", "b", "c"]);
    }

    #[test]
    fn entry_with_reference_to_other_entry_resorts_based_on_other_entry_being_there_or_not() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("a", Left, 2.0, 1.0));
        model.add(entry("b", Left, 1.0, 1.0));
        model.add(relative("c", Left, ("not-existing", 0.0), Left, 1.0));
        assert_eq!(ids(&model), ["a", "b", "c"]);

        let target = model.add(entry("not-existing", Left, 3.0, 1.0));
        assert_eq!(ids(&model), ["c", "not-existing", "a", "b"]);

        model.remove(target);
        assert_eq!(ids(&model), ["a", "b", "c"]);
    }

    #[test]
    fn entry_with_reference_to_other_entry_but_different_alignment_does_not_explode() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("1-left", Left, 2.0, 1.0));
        model.add(entry("2-left", Left, 1.0, 1.0));
        model.add(entry("1-right", Right, 2.0, 1.0));
        model.add(entry("2-right", Right, 1.0, 1.0));
        assert_eq!(model.get_entries(Left).len(), 2);
        assert_eq!(model.get_entries(Right).len(), 2);

        let left = model.add(relative("relative", Left, ("1-right", 2.0), Left, 1.0));
        assert_eq!(model.get_entries(Left).len(), 3);
        assert_eq!(model.get_entries(Left)[2].handle(), left);
        assert_eq!(model.get_entries(Right).len(), 2);
        model.remove(left);

        model.add(relative("relative", Right, ("1-right", 2.0), Left, 1.0));
        assert_eq!(model.get_entries(Left).len(), 2);
        assert_eq!(model.get_entries(Right).len(), 3);
    }

    #[test]
    fn entry_with_reference_to_other_entry_respects_secondary_sorting_existent() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("ref", Left, 0.0, 0.0));
        model.add(relative("entry2", Right, ("ref", 0.0), Right, 2.0));
        model.add(relative("entry1", Right, ("ref", 0.0), Right, 1.0));
        model.add(relative("entry3", Right, ("ref", 0.0), Right, 3.0));
        assert_eq!(ids(&model), ["ref", "entry3", "entry2", "entry1"]);
    }

    #[test]
    fn entry_with_reference_to_other_entry_respects_secondary_sorting_nonexistent() {
        let mut model = StatusbarViewModel::new();
        model.add(relative("entry2", Right, ("ref", 1.0), Right, 2.0));
        model.add(relative("entry1", Right, ("ref", 1.0), Right, 1.0));
        model.add(relative("entry3", Right, ("ref", 1.0), Right, 3.0));
        assert_eq!(ids(&model), ["entry3", "entry2", "entry1"]);
    }

    // Tests below are not in the upstream suite.

    #[test]
    fn chained_reference_joins_the_group_of_the_root_entry() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("root", Left, 1.0, 0.0));
        model.add(relative("child", Left, ("root", 0.0), Right, 2.0));
        model.add(relative("grandchild", Left, ("child", 0.0), Right, 1.0));
        model.add(entry("other", Left, 0.0, 0.0));
        assert_eq!(ids(&model), ["root", "child", "grandchild", "other"]);
    }

    #[test]
    fn relative_entries_with_the_same_id_keep_only_the_last() {
        let mut model = StatusbarViewModel::new();
        model.add(entry("a", Left, 1.0, 0.0));
        let first = model.add(relative("dup", Left, ("a", 0.0), Right, 0.0));
        let second = model.add(relative("dup", Left, ("a", 0.0), Right, 0.0));
        assert_eq!(ids(&model), ["a", "dup"]);
        assert!(model.find_entry(first).is_none());
        assert!(model.find_entry(second).is_some());
    }

    #[test]
    fn hide_fires_once_per_entry_with_the_id_and_saves_state() {
        let saved = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
        let mut model = StatusbarViewModel::with_hidden_state(Some(r#"["x"]"#));
        let s = saved.clone();
        model.set_save_hook(move |v| s.lock().unwrap().push(v.map(str::to_owned)));
        assert!(model.is_hidden("x"));
        model.add(entry("a", Left, 1.0, 0.0));
        model.add(entry("a", Right, 1.0, 0.0));
        let (seen, _d) = record(&model);

        model.hide("a");
        model.hide("a");
        assert_eq!(seen.lock().unwrap().len(), 2);
        model.show("x");
        model.show("a");
        model.show("never-hidden");
        assert_eq!(seen.lock().unwrap().len(), 4);
        assert_eq!(
            *saved.lock().unwrap(),
            [
                Some(r#"["x","a"]"#.to_owned()),
                Some(r#"["a"]"#.to_owned()),
                None
            ]
        );
        assert_eq!(model.hidden_state(), None);
    }

    #[test]
    fn restore_state_follows_new_set_of_json_parse() {
        let hidden = |raw| StatusbarViewModel::with_hidden_state(raw).hidden;
        assert_eq!(hidden(Some(r#"["a","b","a",1]"#)), ["a", "b"]);
        assert_eq!(hidden(Some(r#""ab""#)), ["a", "b"]);
        assert!(hidden(Some("{}")).is_empty());
        assert!(hidden(Some("not json")).is_empty());
        assert!(hidden(Some("null")).is_empty());
        assert!(hidden(None).is_empty());
    }

    #[test]
    fn storage_change_fires_for_entries_whose_visibility_changed() {
        let mut model = StatusbarViewModel::with_hidden_state(Some(r#"["a","b"]"#));
        model.add(entry("a", Left, 3.0, 0.0));
        model.add(entry("b", Left, 2.0, 0.0));
        model.add(entry("c", Left, 1.0, 0.0));
        let (seen, _d) = record(&model);
        model.on_did_storage_value_change(Some(r#"["b","c","gone"]"#));
        let got: Vec<(String, bool)> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|e| (e.id.clone(), e.visible))
            .collect();
        assert_eq!(got, [("a".into(), true), ("c".into(), false)]);
        assert!(model.is_hidden("gone"));
    }

    #[test]
    fn focus_moves_over_visible_entries_and_wraps() {
        let mut model = StatusbarViewModel::new();
        let a = model.add(entry("a", Left, 3.0, 0.0));
        let b = model.add(entry("b", Left, 2.0, 0.0));
        let c = model.add(entry("c", Right, 1.0, 0.0));
        model.hide("b");
        assert_eq!(model.focus_next_entry(None), Some(a));
        assert_eq!(model.focus_next_entry(Some(a)), Some(c));
        assert_eq!(model.focus_next_entry(Some(c)), Some(a));
        assert_eq!(model.focus_previous_entry(Some(a)), Some(c));
        assert_eq!(model.focus_previous_entry(None), Some(c));
        assert_eq!(model.last_focused_entry().map(|e| e.handle()), Some(c));
        model.hide("c");
        assert!(model.last_focused_entry().is_none());
        model.show("b");
        assert_eq!(model.focus_previous_entry(Some(a)), Some(b));
    }

    #[test]
    fn first_and_last_visible_entry_per_side() {
        let mut model = StatusbarViewModel::new();
        let a = model.add(entry("a", Left, 3.0, 0.0));
        model.add(entry("b", Left, 2.0, 0.0));
        let c = model.add(entry("c", Left, 1.0, 0.0));
        let r = model.add(entry("r", Right, 1.0, 0.0));
        assert_eq!(model.first_last_visible(Left), (Some(a), Some(c)));
        assert_eq!(model.first_last_visible(Right), (Some(r), Some(r)));
        model.hide("r");
        assert_eq!(model.first_last_visible(Right), (None, None));
    }

    #[test]
    fn compact_entries_group_chained_locations_of_visible_entries() {
        let mut model = StatusbarViewModel::new();
        let root = model.add(entry("root", Right, 5.0, 0.0));
        let after = model.add(compact(relative("after", Right, ("root", 0.0), Right, 0.0)));
        let chained = model.add(compact(relative(
            "chained",
            Right,
            ("after", 0.0),
            Left,
            0.0,
        )));
        model.add(relative("loose", Right, ("root", 0.0), Left, 0.0));
        let solo = model.add(entry("solo", Right, 1.0, 0.0));
        let before = model.add(compact(relative("before", Right, ("solo", 0.0), Left, 0.0)));

        // Upstream quirk, matched: `chained` first sorts before `after` (left of it), so the
        // next sort visits `chained` before `after` joined the `root` group; `chained` then
        // starts a leftover group and moves to the end.
        assert_eq!(
            ids(&model),
            ["loose", "root", "after", "before", "solo", "chained"]
        );
        let compact = model.compact_entries();
        assert_eq!(
            compact.groups,
            [vec![after, root, chained], vec![before, solo]]
        );
        assert_eq!(compact.compact_left, HashSet::from([after, solo]));
        assert_eq!(
            compact.compact_right,
            HashSet::from([root, before, chained])
        );

        model.hide("solo");
        assert_eq!(model.compact_entries().groups, [vec![after, root, chained]]);
    }
}
