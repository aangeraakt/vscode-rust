//! The `diffIdentityProvider` smart splice of [`IndexTreeModel`] (`spliceSmart` in
//! `indexTreeModel.ts`): children whose identity did not change keep their nodes.

use super::{IndexTreeModel, SpliceOptions};
use crate::tree::{NodeId, TreeElement, TreeError, lcs};

/// State shared by one smart splice and its recursion over unchanged nodes.
struct SmartSplice<'s, 'e, T> {
    identity: &'s dyn Fn(&T) -> String,
    prefix: &'s [usize],
    levels: usize,
    to_insert: &'e mut Vec<Option<TreeElement<T>>>,
}

impl<T> SmartSplice<'_, '_, T> {
    /// Takes the element at `index` of the inserted elements, if it is in range.
    fn take(&mut self, index: isize) -> Option<TreeElement<T>> {
        let index = usize::try_from(index).ok()?;
        self.to_insert.get_mut(index)?.take()
    }

    /// The location of the child at `index` of the spliced parent.
    fn at(&self, index: usize) -> Vec<usize> {
        self.prefix.iter().copied().chain([index]).collect()
    }
}

impl<T, F> IndexTreeModel<T, F> {
    pub(super) fn splice_smart(
        &mut self,
        identity: &dyn Fn(&T) -> String,
        location: &[usize],
        delete_count: usize,
        to_insert: Vec<TreeElement<T>>,
        options: &mut SpliceOptions<'_, T, F>,
        recurse_levels: usize,
    ) -> Result<(), TreeError> {
        let parent = self.parent_node_with_list_index(location)?.parent;
        let Some(original) = self.n(parent).last_diff_ids.clone() else {
            return self.splice_simple(location, delete_count, to_insert, options);
        };
        let index = location[location.len() - 1];
        let children = &self.n(parent).children;
        let end = index.saturating_add(delete_count).min(children.len());
        let id_of = |id: &NodeId| identity(&self.n(*id).element);
        let modified: Vec<String> = children[..index]
            .iter()
            .map(id_of)
            .chain(to_insert.iter().map(|e| identity(&e.element)))
            .chain(children[end..].iter().map(id_of))
            .collect();
        let Some(changes) = lcs::diff(&original, &modified) else {
            // `LcsDiff` gave a best-effort diff: use the plain splice.
            self.n_mut(parent).last_diff_ids = None;
            return self.splice_simple(location, delete_count, to_insert, options);
        };

        let prefix = &location[..location.len() - 1];
        let mut to_insert: Vec<Option<TreeElement<T>>> = to_insert.into_iter().map(Some).collect();
        let mut smart = SmartSplice {
            identity,
            prefix,
            levels: recurse_levels,
            to_insert: &mut to_insert,
        };
        let mut last_o = end;
        let mut last_m = smart.to_insert.len() as isize;
        for change in changes.iter().rev() {
            let count = last_o.saturating_sub(change.original_start + change.original_length);
            self.smart_recurse(&mut smart, options, last_o, last_m, count)?;
            last_o = change.original_start;
            last_m = change.modified_start as isize - index as isize;
            let inserted: Vec<_> = (0..change.modified_length as isize)
                .filter_map(|i| smart.take(last_m + i))
                .collect();
            let location = smart.at(last_o);
            self.splice_simple(&location, change.original_length, inserted, options)?;
        }
        // Whatever precedes the first change is unchanged.
        self.smart_recurse(&mut smart, options, last_o, last_m, last_o)
    }

    /// Re-splices the children of the `count` unchanged nodes that end at the given original
    /// and modified positions (`recurseSplice`).
    fn smart_recurse(
        &mut self,
        smart: &mut SmartSplice<'_, '_, T>,
        options: &mut SpliceOptions<'_, T, F>,
        mut from_original: usize,
        mut from_modified: isize,
        count: usize,
    ) -> Result<(), TreeError> {
        if smart.levels == 0 {
            return Ok(());
        }
        for _ in 0..count {
            from_original -= 1;
            from_modified -= 1;
            // Unchanged nodes before the splice index have no element in `to_insert`.
            let Some(element) = smart.take(from_modified) else {
                continue;
            };
            let mut location = smart.at(from_original);
            location.push(0);
            self.splice_smart(
                smart.identity,
                &location,
                usize::MAX,
                element.children,
                options,
                smart.levels - 1,
            )?;
        }
        Ok(())
    }
}
