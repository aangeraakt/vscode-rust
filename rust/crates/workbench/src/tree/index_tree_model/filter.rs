//! Refiltering for [`IndexTreeModel`] (`refilter` and `updateNodeAfterFilterChange` in
//! `indexTreeModel.ts`).

use super::IndexTreeModel;
use crate::tree::{NodeId, TreeListSplice, TreeVisibility};

impl<T, F> IndexTreeModel<T, F> {
    // ---- filter -------------------------------------------------------------------------

    /// Re-runs the filter on every node and re-emits all rows (`refilter`).
    pub fn refilter(&mut self) {
        let previous = self.n(self.root).render_node_count;
        let to_insert = self.update_node_after_filter_change(self.root);
        self.on_did_splice_rendered_nodes.fire(TreeListSplice {
            start: 0,
            delete_count: previous,
            elements: to_insert,
        });
        self.refilter_pending = false;
    }

    /// Whether a splice under a `Recurse` node asked for a refilter that has not run yet.
    pub fn has_pending_refilter(&self) -> bool {
        self.refilter_pending
    }

    /// Runs the refilter a splice asked for, if any (the upstream microtask).
    pub fn run_pending_refilter(&mut self) {
        if self.refilter_pending {
            self.refilter();
        }
    }

    fn update_node_after_filter_change(&mut self, id: NodeId) -> Vec<NodeId> {
        let previous = self.n(id).render_node_count;
        let mut result = Vec::new();
        let parent_visibility = TreeVisibility::from(self.n(id).visible);
        self.update_node_after_filter_change_rec(id, parent_visibility, &mut result, true);
        let parent = self.n(id).parent;
        self.update_ancestors_render_node_count(parent, result.len() as isize - previous as isize);
        result
    }

    fn update_node_after_filter_change_rec(
        &mut self,
        id: NodeId,
        parent_visibility: TreeVisibility,
        result: &mut Vec<NodeId>,
        revealed: bool,
    ) -> bool {
        let is_root = id == self.root;
        let visibility = if is_root {
            self.n(id).visibility
        } else {
            let visibility = self.filter_node(id, parent_visibility);
            if visibility == TreeVisibility::Hidden {
                let node = self.n_mut(id);
                node.visible = false;
                node.render_node_count = 0;
                return false;
            }
            if revealed {
                result.push(id);
            }
            visibility
        };

        let start = result.len();
        let collapsed = self.n(id).collapsed;
        let mut has_visible_descendants = false;
        let mut visible_child_index = 0;
        for child in self.n(id).children.clone() {
            has_visible_descendants = self.update_node_after_filter_change_rec(
                child,
                visibility,
                result,
                revealed && !collapsed,
            ) || has_visible_descendants;
            let child = self.n_mut(child);
            if child.visible {
                child.visible_child_index = Some(visible_child_index);
                visible_child_index += 1;
            }
        }

        let node = self.n_mut(id);
        node.visible_children_count = visible_child_index;
        if !is_root {
            node.visible = if visibility == TreeVisibility::Recurse {
                has_visible_descendants
            } else {
                visibility == TreeVisibility::Visible
            };
            node.visibility = visibility;
        }
        node.render_node_count = usize::from(!is_root);
        if !node.visible {
            node.render_node_count = 0;
            if revealed {
                result.pop();
            }
        } else if !node.collapsed {
            node.render_node_count += result.len() - start;
        }
        let visible = node.visible;
        self.fire_render_node_count(id);
        visible
    }
}
