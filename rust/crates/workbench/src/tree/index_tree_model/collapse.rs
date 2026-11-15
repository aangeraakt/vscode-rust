//! Collapse and expand for [`IndexTreeModel`] (`setCollapsible`, `setCollapsed`, `expandTo`
//! and the render-count updates they trigger in `indexTreeModel.ts`).

use super::{Buffered, IndexTreeModel, NodeInfo};
use crate::tree::{CollapseStateChange, NodeId, TreeError, TreeListSplice};

#[derive(Clone, Copy)]
enum CollapseUpdate {
    Collapsible(bool),
    Collapsed { collapsed: bool, recursive: bool },
}

impl<T, F> IndexTreeModel<T, F> {
    fn fire_collapse_state(&mut self, event: CollapseStateChange) {
        if self.buffer_depth > 0 {
            self.buffered.push(Buffered::CollapseState(event));
        } else {
            self.on_did_change_collapse_state.fire(event);
        }
    }

    /// `EventBufferer.bufferEvents`: buffered events fire, in order, when the outermost call
    /// returns.
    fn buffer_events<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.buffer_depth += 1;
        let result = f(self);
        self.buffer_depth -= 1;
        if self.buffer_depth == 0 {
            for event in std::mem::take(&mut self.buffered) {
                match event {
                    Buffered::CollapseState(e) => self.on_did_change_collapse_state.fire(e),
                    Buffered::RenderNodeCount(n) => self.on_did_change_render_node_count.fire(n),
                }
            }
        }
        result
    }

    // ---- collapse -----------------------------------------------------------------------

    /// Whether the node at `location` is collapsible (`isCollapsible`).
    pub fn is_collapsible(&self, location: &[usize]) -> Result<bool, TreeError> {
        Ok(self.n(self.tree_node(location)?).collapsible)
    }

    /// Sets (or with `None` toggles) collapsibility; returns whether it changed
    /// (`setCollapsible`).
    pub fn set_collapsible(
        &mut self,
        location: &[usize],
        collapsible: Option<bool>,
    ) -> Result<bool, TreeError> {
        let node = self.n(self.tree_node(location)?);
        let collapsible = collapsible.unwrap_or(!node.collapsible);
        self.buffer_events(|m| {
            m.set_collapse_state(location, CollapseUpdate::Collapsible(collapsible))
        })
    }

    /// Whether the node at `location` is collapsed (`isCollapsed`).
    pub fn is_collapsed(&self, location: &[usize]) -> Result<bool, TreeError> {
        Ok(self.n(self.tree_node(location)?).collapsed)
    }

    /// Collapses or expands (with `None`, toggles) the node at `location`, and with `recursive`
    /// all its descendants; returns whether anything changed (`setCollapsed`).
    pub fn set_collapsed(
        &mut self,
        location: &[usize],
        collapsed: Option<bool>,
        recursive: bool,
    ) -> Result<bool, TreeError> {
        let node = self.n(self.tree_node(location)?);
        let collapsed = collapsed.unwrap_or(!node.collapsed);
        let update = CollapseUpdate::Collapsed {
            collapsed,
            recursive,
        };
        self.buffer_events(|m| m.set_collapse_state(location, update))
    }

    fn set_collapse_state(
        &mut self,
        location: &[usize],
        update: CollapseUpdate,
    ) -> Result<bool, TreeError> {
        let info = self.tree_node_with_list_index(location)?;
        let result = self.set_list_node_collapse_state(&info, update);
        let node = self.n(info.node);
        if info.node != self.root
            && self.auto_expand_single_children
            && result
            && matches!(
                update,
                CollapseUpdate::Collapsed {
                    recursive: false,
                    ..
                }
            )
            && node.collapsible
            && !node.collapsed
        {
            let mut visible = node
                .children
                .iter()
                .enumerate()
                .filter(|&(_, &c)| self.n(c).visible)
                .map(|(i, _)| i);
            let only = match (visible.next(), visible.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            };
            if let Some(only) = only {
                let mut child = location.to_vec();
                child.push(only);
                self.set_collapse_state(&child, update)?;
            }
        }
        Ok(result)
    }

    fn set_list_node_collapse_state(&mut self, info: &NodeInfo, update: CollapseUpdate) -> bool {
        let result = self.set_node_collapse_state(info.node, update, false);
        if !info.revealed || !self.n(info.node).visible || !result {
            return result;
        }
        let previous = self.n(info.node).render_node_count;
        let mut to_insert = self.update_node_after_collapse_change(info.node);
        let (start, delete_count) = match info.list_index {
            Some(index) => {
                to_insert.remove(0);
                (index + 1, previous - 1)
            }
            None => (0, previous),
        };
        self.on_did_splice_rendered_nodes.fire(TreeListSplice {
            start,
            delete_count,
            elements: to_insert,
        });
        result
    }

    fn set_node_collapse_state(&mut self, id: NodeId, update: CollapseUpdate, deep: bool) -> bool {
        let mut result = false;
        if id != self.root {
            let node = self.n_mut(id);
            match update {
                CollapseUpdate::Collapsible(collapsible) => {
                    result = node.collapsible != collapsible;
                    node.collapsible = collapsible;
                }
                CollapseUpdate::Collapsed { collapsed, .. } if node.collapsible => {
                    result = node.collapsed != collapsed;
                    node.collapsed = collapsed;
                }
                CollapseUpdate::Collapsed { .. } => {}
            }
            if result {
                self.fire_collapse_state(CollapseStateChange { node: id, deep });
            }
        }
        if let CollapseUpdate::Collapsed {
            recursive: true, ..
        } = update
        {
            for child in self.n(id).children.clone() {
                result = self.set_node_collapse_state(child, update, true) || result;
            }
        }
        result
    }

    /// Expands every collapsed ancestor of the node at `location` (`expandTo`).
    pub fn expand_to(&mut self, location: &[usize]) -> Result<(), TreeError> {
        self.tree_node(location)?;
        self.buffer_events(|m| {
            for len in (0..location.len()).rev() {
                let ancestor = &location[..len];
                if m.n(m.tree_node(ancestor)?).collapsed {
                    let update = CollapseUpdate::Collapsed {
                        collapsed: false,
                        recursive: false,
                    };
                    m.set_collapse_state(ancestor, update)?;
                }
            }
            Ok(())
        })
    }

    fn update_node_after_collapse_change(&mut self, id: NodeId) -> Vec<NodeId> {
        let previous = self.n(id).render_node_count;
        let mut result = Vec::new();
        self.update_node_after_collapse_change_rec(id, &mut result);
        let parent = self.n(id).parent;
        self.update_ancestors_render_node_count(parent, result.len() as isize - previous as isize);
        result
    }

    fn update_node_after_collapse_change_rec(
        &mut self,
        id: NodeId,
        result: &mut Vec<NodeId>,
    ) -> usize {
        if !self.n(id).visible {
            return 0;
        }
        let is_root = id == self.root;
        if !is_root {
            result.push(id);
        }
        let mut count = usize::from(!is_root);
        if !self.n(id).collapsed {
            for child in self.n(id).children.clone() {
                count += self.update_node_after_collapse_change_rec(child, result);
            }
        }
        self.n_mut(id).render_node_count = count;
        self.fire_render_node_count(id);
        count
    }
}
