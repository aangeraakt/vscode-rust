//! A tree model addressed by index paths (`[0, 2]` = third child of the first root child).
//!
//! Ported from `src/vs/base/browser/ui/tree/indexTreeModel.ts` (`IndexTreeModel`).
//!
//! The model flattens the expanded, visible nodes into the rows a list view shows, and reports
//! every change to those rows as a [`TreeListSplice`] on
//! [`IndexTreeModel::on_did_splice_rendered_nodes`].
//!
//! Intentional differences from the TypeScript:
//! - Nodes live in an arena and are referred to by [`NodeId`]; read them with
//!   [`IndexTreeModel::node`] after the call that changed them returns. Listeners cannot read
//!   the model while it is being mutated.
//! - Invalid locations return [`TreeError`] instead of throwing. An index equal to the child
//!   count is accepted only as the last component of a `splice` location; upstream accepts it
//!   everywhere and then fails on the missing node (`has` returns `false` here).
//! - `getListIndex` returns `None` where upstream returns `-1`; the root's
//!   `visibleChildIndex` of `-1` is `None`.
//! - Refiltering after a splice under a `Recurse` ancestor is delayed to a microtask upstream.
//!   Here the model only records it ([`IndexTreeModel::has_pending_refilter`]); the owner calls
//!   [`IndexTreeModel::run_pending_refilter`] when its event-loop turn ends.
//! - `refilter` passes the root's visibility (`Visible`) as the parent visibility of top-level
//!   nodes; upstream passes `undefined`.
//! - A recursive collapse of the root does not count the root as a rendered row (upstream
//!   leaves the root's render count one too high, which a JS list hides by clamping).
//! - The `diffIdentityProvider` smart splice is not ported yet; every splice replaces nodes.
//!
//! Upstream behavior kept as is: the `visibleChildIndex` given to inserted nodes starts from the
//! visible child at or before the splice index, so it is only exact when that child is visible
//! (where upstream would then go below zero on a later delete, it stops at 0 here).

use super::{
    CollapseStateChange, NodeId, TreeElement, TreeError, TreeFilter, TreeFilterResult,
    TreeListSplice, TreeModelSplice, TreeVisibility,
};
use vscode_api::{Emitter, Event};

mod collapse;
mod filter;

/// A node of an [`IndexTreeModel`] (`IIndexTreeNode`).
#[derive(Debug, Clone)]
pub struct TreeNode<T, F> {
    element: T,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    depth: usize,
    visible_children_count: usize,
    visible_child_index: Option<usize>,
    collapsible: bool,
    collapsed: bool,
    render_node_count: usize,
    visibility: TreeVisibility,
    visible: bool,
    filter_data: Option<F>,
}

impl<T, F> TreeNode<T, F> {
    /// The element.
    pub fn element(&self) -> &T {
        &self.element
    }
    /// The parent node; `None` for the root.
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }
    /// The child nodes, in order.
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
    /// Depth; the root is 0 and its children are 1.
    pub fn depth(&self) -> usize {
        self.depth
    }
    /// Number of visible children.
    pub fn visible_children_count(&self) -> usize {
        self.visible_children_count
    }
    /// Position among the visible siblings; `None` for the root and nodes never visible.
    pub fn visible_child_index(&self) -> Option<usize> {
        self.visible_child_index
    }
    /// Whether the node can be collapsed.
    pub fn collapsible(&self) -> bool {
        self.collapsible
    }
    /// Whether the node is collapsed.
    pub fn collapsed(&self) -> bool {
        self.collapsed
    }
    /// Rows this node and its revealed descendants take in the list (0 when hidden).
    pub fn render_node_count(&self) -> usize {
        self.render_node_count
    }
    /// The filter's last verdict for this node.
    pub fn visibility(&self) -> TreeVisibility {
        self.visibility
    }
    /// Whether the node passes the filter.
    pub fn visible(&self) -> bool {
        self.visible
    }
    /// Data from the last [`TreeFilterResult::Data`] result.
    pub fn filter_data(&self) -> Option<&F> {
        self.filter_data.as_ref()
    }
}

/// Construction options (`IIndexTreeModelOptions`).
pub struct IndexTreeModelOptions<T, F> {
    /// Default `collapsed` for inserted elements that do not set it.
    pub collapse_by_default: bool,
    /// Keep `collapsible = false` on nodes with children.
    pub allow_non_collapsible_parents: bool,
    /// Visibility filter; every node is visible without one.
    pub filter: Option<TreeFilter<T, F>>,
    /// Expanding a node with a single visible child expands that child too.
    pub auto_expand_single_children: bool,
}

impl<T, F> Default for IndexTreeModelOptions<T, F> {
    fn default() -> Self {
        Self {
            collapse_by_default: false,
            allow_non_collapsible_parents: false,
            filter: None,
            auto_expand_single_children: false,
        }
    }
}

/// Callback receiving a created or deleted node.
pub type NodeCallback<'a, T, F> = &'a mut dyn FnMut(NodeId, &TreeNode<T, F>);

/// Options for [`IndexTreeModel::splice`] (`IIndexTreeModelSpliceOptions`).
pub struct SpliceOptions<'a, T, F> {
    /// Called for every created node, children first.
    pub on_did_create_node: Option<NodeCallback<'a, T, F>>,
    /// Called for every deleted node, parents first.
    pub on_did_delete_node: Option<NodeCallback<'a, T, F>>,
}

impl<T, F> Default for SpliceOptions<'_, T, F> {
    fn default() -> Self {
        Self {
            on_did_create_node: None,
            on_did_delete_node: None,
        }
    }
}

enum Buffered {
    CollapseState(CollapseStateChange),
    RenderNodeCount(NodeId),
}

struct Slot<T, F> {
    generation: u32,
    node: Option<TreeNode<T, F>>,
}

struct ParentInfo {
    parent: NodeId,
    list_index: usize,
    revealed: bool,
    visible: bool,
}

struct NodeInfo {
    node: NodeId,
    /// `None` for the root, which has no row.
    list_index: Option<usize>,
    revealed: bool,
    visible: bool,
}

/// A tree addressed by index paths (`IndexTreeModel`).
pub struct IndexTreeModel<T, F = ()> {
    user: String,
    slots: Vec<Slot<T, F>>,
    free: Vec<u32>,
    root: NodeId,
    collapse_by_default: bool,
    allow_non_collapsible_parents: bool,
    auto_expand_single_children: bool,
    filter: Option<TreeFilter<T, F>>,
    buffer_depth: usize,
    buffered: Vec<Buffered>,
    refilter_pending: bool,
    on_did_splice_model: Emitter<TreeModelSplice>,
    on_did_splice_rendered_nodes: Emitter<TreeListSplice>,
    on_did_change_collapse_state: Emitter<CollapseStateChange>,
    on_did_change_render_node_count: Emitter<NodeId>,
}

impl<T, F> IndexTreeModel<T, F> {
    /// Creates a model whose (unrendered) root holds `root_element`. `user` labels errors.
    pub fn new(user: &str, root_element: T, options: IndexTreeModelOptions<T, F>) -> Self {
        let mut model = Self {
            user: user.to_owned(),
            slots: Vec::new(),
            free: Vec::new(),
            root: NodeId {
                index: 0,
                generation: 0,
            },
            collapse_by_default: options.collapse_by_default,
            allow_non_collapsible_parents: options.allow_non_collapsible_parents,
            auto_expand_single_children: options.auto_expand_single_children,
            filter: options.filter,
            buffer_depth: 0,
            buffered: Vec::new(),
            refilter_pending: false,
            on_did_splice_model: Emitter::new(),
            on_did_splice_rendered_nodes: Emitter::new(),
            on_did_change_collapse_state: Emitter::new(),
            on_did_change_render_node_count: Emitter::new(),
        };
        model.root = model.alloc(TreeNode {
            element: root_element,
            parent: None,
            children: Vec::new(),
            depth: 0,
            visible_children_count: 0,
            visible_child_index: None,
            collapsible: false,
            collapsed: false,
            render_node_count: 0,
            visibility: TreeVisibility::Visible,
            visible: true,
            filter_data: None,
        });
        model
    }

    // ---- events -------------------------------------------------------------------------

    /// Fires after each change to one parent's children (`onDidSpliceModel`).
    pub fn on_did_splice_model(&self) -> Event<TreeModelSplice> {
        self.on_did_splice_model.event()
    }

    /// Fires for each change to the rendered rows (`onDidSpliceRenderedNodes`).
    pub fn on_did_splice_rendered_nodes(&self) -> Event<TreeListSplice> {
        self.on_did_splice_rendered_nodes.event()
    }

    /// Fires when a node's collapsible or collapsed state changes (`onDidChangeCollapseState`).
    /// During `set_collapsed`, `set_collapsible` and `expand_to`, delivery waits until the
    /// call finishes.
    pub fn on_did_change_collapse_state(&self) -> Event<CollapseStateChange> {
        self.on_did_change_collapse_state.event()
    }

    /// Fires when a node's render count is recomputed (`onDidChangeRenderNodeCount`); buffered
    /// like [`IndexTreeModel::on_did_change_collapse_state`].
    pub fn on_did_change_render_node_count(&self) -> Event<NodeId> {
        self.on_did_change_render_node_count.event()
    }

    fn fire_render_node_count(&mut self, node: NodeId) {
        if self.buffer_depth > 0 {
            self.buffered.push(Buffered::RenderNodeCount(node));
        } else {
            self.on_did_change_render_node_count.fire(node);
        }
    }

    // ---- arena --------------------------------------------------------------------------

    fn alloc(&mut self, node: TreeNode<T, F>) -> NodeId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.node = Some(node);
            NodeId {
                index,
                generation: slot.generation,
            }
        } else {
            let index = u32::try_from(self.slots.len()).expect("tree node arena full");
            self.slots.push(Slot {
                generation: 0,
                node: Some(node),
            });
            NodeId {
                index,
                generation: 0,
            }
        }
    }

    fn release(&mut self, id: NodeId) {
        let slot = &mut self.slots[id.index as usize];
        if let Some(node) = slot.node.take() {
            slot.generation = slot.generation.wrapping_add(1);
            self.free.push(id.index);
            for child in node.children {
                self.release(child);
            }
        }
    }

    fn n(&self, id: NodeId) -> &TreeNode<T, F> {
        self.node(id).expect("stale tree node id")
    }

    fn n_mut(&mut self, id: NodeId) -> &mut TreeNode<T, F> {
        let slot = &mut self.slots[id.index as usize];
        assert_eq!(slot.generation, id.generation, "stale tree node id");
        slot.node.as_mut().expect("stale tree node id")
    }

    /// The node for `id`, or `None` if it was deleted.
    pub fn node(&self, id: NodeId) -> Option<&TreeNode<T, F>> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.node.as_ref()
    }

    /// The root node's id.
    pub fn root(&self) -> NodeId {
        self.root
    }

    fn invalid(&self) -> TreeError {
        TreeError::new(&self.user, "Invalid tree location")
    }

    // ---- splice -------------------------------------------------------------------------

    /// Replaces `delete_count` children at `location` with `to_insert` (`splice`).
    ///
    /// `location` addresses the first replaced child, so it must not be empty; `delete_count`
    /// is clamped to the available children.
    pub fn splice(
        &mut self,
        location: &[usize],
        delete_count: usize,
        to_insert: impl IntoIterator<Item = TreeElement<T>>,
        mut options: SpliceOptions<'_, T, F>,
    ) -> Result<(), TreeError> {
        if location.is_empty() {
            return Err(self.invalid());
        }
        let to_insert = to_insert.into_iter().collect();
        self.splice_simple(location, delete_count, to_insert, &mut options)
    }

    fn splice_simple(
        &mut self,
        location: &[usize],
        delete_count: usize,
        to_insert: Vec<TreeElement<T>>,
        options: &mut SpliceOptions<'_, T, F>,
    ) -> Result<(), TreeError> {
        let ParentInfo {
            parent,
            list_index,
            revealed,
            visible,
        } = self.parent_node_with_list_index(location)?;
        let parent_visibility = TreeVisibility::from(self.n(parent).visible);
        let last_index = location[location.len() - 1];

        // The visible child index right before the splice point.
        let mut visible_child_start_index = 0;
        let children = &self.n(parent).children;
        if last_index < children.len() {
            for &child in children[..=last_index].iter().rev() {
                let child = self.n(child);
                if child.visible {
                    visible_child_start_index = child.visible_child_index.unwrap_or(0);
                    break;
                }
            }
        }

        let mut tree_list = Vec::new();
        let mut nodes_to_insert = Vec::with_capacity(to_insert.len());
        let mut inserted_visible = 0;
        let mut render_node_count = 0;
        for element in to_insert {
            let child = self.create_tree_node(
                element,
                parent,
                parent_visibility,
                revealed,
                &mut tree_list,
                options,
            );
            nodes_to_insert.push(child);
            let child = self.n_mut(child);
            render_node_count += child.render_node_count;
            if child.visible {
                child.visible_child_index = Some(visible_child_start_index + inserted_visible);
                inserted_visible += 1;
            }
        }

        let parent_node = self.n_mut(parent);
        let end = last_index
            .saturating_add(delete_count)
            .min(parent_node.children.len());
        let deleted: Vec<NodeId> = parent_node
            .children
            .splice(last_index..end, nodes_to_insert.iter().copied())
            .collect();
        let deleted_visible = deleted.iter().filter(|&&n| self.n(n).visible).count();
        if deleted_visible != 0 {
            let after = self.n(parent).children[last_index + nodes_to_insert.len()..].to_vec();
            for child in after {
                let child = self.n_mut(child);
                if child.visible {
                    child.visible_child_index = child
                        .visible_child_index
                        .map(|i| i.saturating_sub(deleted_visible));
                }
            }
        }
        let parent_node = self.n_mut(parent);
        parent_node.visible_children_count =
            parent_node.visible_children_count + inserted_visible - deleted_visible;

        if let Some(on_delete) = options.on_did_delete_node.as_deref_mut() {
            let mut stack: Vec<NodeId> = deleted.iter().rev().copied().collect();
            while let Some(id) = stack.pop() {
                let node = self.n(id);
                on_delete(id, node);
                stack.extend(node.children.iter().rev());
            }
        }

        if revealed && visible {
            let visible_delete_count: usize = deleted
                .iter()
                .map(|&n| self.n(n))
                .filter(|n| n.visible)
                .map(|n| n.render_node_count)
                .sum();
            self.update_ancestors_render_node_count(
                Some(parent),
                render_node_count as isize - visible_delete_count as isize,
            );
            self.on_did_splice_rendered_nodes.fire(TreeListSplice {
                start: list_index,
                delete_count: visible_delete_count,
                elements: tree_list,
            });
        }

        for &id in &deleted {
            self.release(id);
        }
        self.on_did_splice_model.fire(TreeModelSplice {
            inserted_nodes: nodes_to_insert,
            deleted_nodes: deleted,
        });

        // Delayed upstream to avoid excessive refiltering (#135941).
        let mut node = Some(parent);
        while let Some(id) = node {
            let n = self.n(id);
            if n.visibility == TreeVisibility::Recurse {
                self.refilter_pending = true;
                break;
            }
            node = n.parent;
        }
        Ok(())
    }

    fn create_tree_node(
        &mut self,
        element: TreeElement<T>,
        parent: NodeId,
        parent_visibility: TreeVisibility,
        revealed: bool,
        tree_list: &mut Vec<NodeId>,
        options: &mut SpliceOptions<'_, T, F>,
    ) -> NodeId {
        let TreeElement {
            element,
            children,
            collapsible,
            collapsed,
        } = element;
        let id = self.alloc(TreeNode {
            element,
            parent: Some(parent),
            children: Vec::with_capacity(children.len()),
            depth: self.n(parent).depth + 1,
            visible_children_count: 0,
            visible_child_index: None,
            collapsible: collapsible.unwrap_or(collapsed.is_some()),
            collapsed: collapsed.unwrap_or(self.collapse_by_default),
            render_node_count: 1,
            visibility: TreeVisibility::Visible,
            visible: true,
            filter_data: None,
        });
        let visibility = self.filter_node(id, parent_visibility);
        self.n_mut(id).visibility = visibility;
        if revealed {
            tree_list.push(id);
        }
        let child_revealed =
            revealed && visibility != TreeVisibility::Hidden && !self.n(id).collapsed;

        let mut visible_children_count = 0;
        let mut render_node_count = 1;
        for element in children {
            let child =
                self.create_tree_node(element, id, visibility, child_revealed, tree_list, options);
            self.n_mut(id).children.push(child);
            let child = self.n_mut(child);
            render_node_count += child.render_node_count;
            if child.visible {
                child.visible_child_index = Some(visible_children_count);
                visible_children_count += 1;
            }
        }

        let allow_non_collapsible_parents = self.allow_non_collapsible_parents;
        let node = self.n_mut(id);
        if !allow_non_collapsible_parents {
            node.collapsible = node.collapsible || !node.children.is_empty();
        }
        node.visible_children_count = visible_children_count;
        node.visible = if visibility == TreeVisibility::Recurse {
            visible_children_count > 0
        } else {
            visibility == TreeVisibility::Visible
        };
        if !node.visible {
            node.render_node_count = 0;
            if revealed {
                tree_list.pop();
            }
        } else if !node.collapsed {
            node.render_node_count = render_node_count;
        }
        if let Some(on_create) = options.on_did_create_node.as_deref_mut() {
            on_create(id, self.n(id));
        }
        id
    }

    /// Re-emits the row of the node at `location` (`rerender`).
    pub fn rerender(&mut self, location: &[usize]) -> Result<(), TreeError> {
        if location.is_empty() {
            return Err(self.invalid());
        }
        let info = self.tree_node_with_list_index(location)?;
        if let (true, true, Some(start)) =
            (self.n(info.node).visible, info.revealed, info.list_index)
        {
            self.on_did_splice_rendered_nodes.fire(TreeListSplice {
                start,
                delete_count: 1,
                elements: vec![info.node],
            });
        }
        Ok(())
    }

    // ---- queries ------------------------------------------------------------------------

    /// Whether `location` addresses a node (`has`).
    pub fn has(&self, location: &[usize]) -> bool {
        self.tree_node(location).is_ok()
    }

    /// The row of the node at `location`, or `None` if it is hidden or inside a collapsed
    /// ancestor (`getListIndex`).
    pub fn get_list_index(&self, location: &[usize]) -> Result<Option<usize>, TreeError> {
        let info = self.tree_node_with_list_index(location)?;
        Ok(if info.visible && info.revealed {
            info.list_index
        } else {
            None
        })
    }

    /// Rows the node at `location` takes (`getListRenderCount`).
    pub fn get_list_render_count(&self, location: &[usize]) -> Result<usize, TreeError> {
        Ok(self.n(self.tree_node(location)?).render_node_count)
    }

    /// The node at `location`; the root for `[]` (`getNode`).
    pub fn get_node(&self, location: &[usize]) -> Result<&TreeNode<T, F>, TreeError> {
        Ok(self.n(self.tree_node(location)?))
    }

    /// The id of the node at `location`.
    pub fn get_node_id(&self, location: &[usize]) -> Result<NodeId, TreeError> {
        self.tree_node(location)
    }

    /// The index path of `node` (`getNodeLocation`). Panics on a stale id.
    pub fn get_node_location(&self, node: NodeId) -> Vec<usize> {
        let mut location = Vec::new();
        let mut current = node;
        while let Some(parent) = self.n(current).parent {
            let index = self.n(parent).children.iter().position(|&c| c == current);
            location.push(index.expect("node is a child of its parent"));
            current = parent;
        }
        location.reverse();
        location
    }

    /// The parent's location; `None` for the root (`getParentNodeLocation`).
    pub fn get_parent_node_location(&self, location: &[usize]) -> Option<Vec<usize>> {
        location.split_last().map(|(_, parent)| parent.to_vec())
    }

    /// The first child's element (`getFirstElementChild`).
    pub fn get_first_element_child(&self, location: &[usize]) -> Result<Option<&T>, TreeError> {
        let node = self.n(self.tree_node(location)?);
        Ok(node.children.first().map(|&c| &self.n(c).element))
    }

    /// The element of the last descendant along last children (`getLastElementAncestor`);
    /// `None` without children.
    pub fn get_last_element_ancestor(&self, location: &[usize]) -> Result<Option<&T>, TreeError> {
        let mut node = self.n(self.tree_node(location)?);
        if node.children.is_empty() {
            return Ok(None);
        }
        while let Some(&last) = node.children.last() {
            node = self.n(last);
        }
        Ok(Some(&node.element))
    }

    fn filter_node(&mut self, id: NodeId, parent_visibility: TreeVisibility) -> TreeVisibility {
        let result = match &self.filter {
            Some(filter) => filter(&self.n(id).element, parent_visibility),
            None => TreeFilterResult::Visibility(TreeVisibility::Visible),
        };
        let (visibility, data) = match result {
            TreeFilterResult::Visibility(v) => (v, None),
            TreeFilterResult::Data { visibility, data } => (visibility, Some(data)),
        };
        self.n_mut(id).filter_data = data;
        visibility
    }

    fn update_ancestors_render_node_count(&mut self, mut node: Option<NodeId>, diff: isize) {
        if diff == 0 {
            return;
        }
        while let Some(id) = node {
            let n = self.n_mut(id);
            n.render_node_count = n
                .render_node_count
                .checked_add_signed(diff)
                .expect("render node count underflow");
            node = n.parent;
            self.fire_render_node_count(id);
        }
    }

    // ---- locations ----------------------------------------------------------------------

    fn tree_node(&self, location: &[usize]) -> Result<NodeId, TreeError> {
        let mut node = self.root;
        for &index in location {
            node = *self
                .n(node)
                .children
                .get(index)
                .ok_or_else(|| self.invalid())?;
        }
        Ok(node)
    }

    fn tree_node_with_list_index(&self, location: &[usize]) -> Result<NodeInfo, TreeError> {
        if location.is_empty() {
            return Ok(NodeInfo {
                node: self.root,
                list_index: None,
                revealed: true,
                visible: false,
            });
        }
        let info = self.parent_node_with_list_index(location)?;
        let index = location[location.len() - 1];
        let node = *self
            .n(info.parent)
            .children
            .get(index)
            .ok_or_else(|| self.invalid())?;
        Ok(NodeInfo {
            node,
            list_index: Some(info.list_index),
            revealed: info.revealed,
            visible: info.visible && self.n(node).visible,
        })
    }

    fn parent_node_with_list_index(&self, location: &[usize]) -> Result<ParentInfo, TreeError> {
        let Some((&last, path)) = location.split_last() else {
            return Err(self.invalid());
        };
        let mut node = self.root;
        let mut list_index = 0;
        let mut revealed = true;
        let mut visible = true;
        for (i, &index) in path.iter().chain([&last]).enumerate() {
            let n = self.n(node);
            let is_last = i == path.len();
            if index > n.children.len() || (!is_last && index == n.children.len()) {
                return Err(self.invalid());
            }
            list_index += n.children[..index]
                .iter()
                .map(|&c| self.n(c).render_node_count)
                .sum::<usize>();
            revealed = revealed && !n.collapsed;
            visible = visible && n.visible;
            if !is_last {
                node = n.children[index];
                list_index += 1;
            }
        }
        Ok(ParentInfo {
            parent: node,
            list_index,
            revealed,
            visible,
        })
    }
}

#[cfg(test)]
#[path = "index_tree_model_tests.rs"]
mod tests;
