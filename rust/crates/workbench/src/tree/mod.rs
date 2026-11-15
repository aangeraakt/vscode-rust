//! Tree widget models, ported from `src/vs/base/browser/ui/tree/`.
//!
//! This module holds the shared types of `tree.ts`. The models keep their nodes in an arena and
//! hand out [`NodeId`] handles where TypeScript hands out node object references; read a node
//! with the model's `node(id)` once the mutating call has returned.
//!
//! Not ported from `tree.ts`: the renderer, drag-and-drop, mouse/keyboard event and
//! `WeakMapper` types (view layer), and the `ITreeModel` interface (each model has the same
//! methods with its own location type instead).

pub mod index_tree_model;

use std::fmt;

pub use index_tree_model::{IndexTreeModel, IndexTreeModelOptions, SpliceOptions, TreeNode};

/// Whether a node is shown (`TreeVisibility`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeVisibility {
    /// The node is hidden.
    Hidden,
    /// The node is visible.
    Visible,
    /// The node is visible if any of its descendants is visible.
    Recurse,
}

impl From<bool> for TreeVisibility {
    /// `true` is [`TreeVisibility::Visible`], `false` is [`TreeVisibility::Hidden`]
    /// (`getVisibleState`).
    fn from(visible: bool) -> Self {
        if visible { Self::Visible } else { Self::Hidden }
    }
}

/// What a filter returns for an element (`TreeFilterResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeFilterResult<F> {
    /// Visibility only; the node's filter data is cleared.
    Visibility(TreeVisibility),
    /// Visibility plus data forwarded to the renderer (`ITreeFilterDataResult`).
    Data {
        /// Whether the node is shown.
        visibility: TreeVisibility,
        /// Metadata stored on the node as its filter data.
        data: F,
    },
}

impl<F> From<bool> for TreeFilterResult<F> {
    fn from(visible: bool) -> Self {
        Self::Visibility(visible.into())
    }
}

impl<F> From<TreeVisibility> for TreeFilterResult<F> {
    fn from(visibility: TreeVisibility) -> Self {
        Self::Visibility(visibility)
    }
}

/// Decides the visibility of an element given its parent's visibility (`ITreeFilter`).
pub type TreeFilter<T, F> = Box<dyn Fn(&T, TreeVisibility) -> TreeFilterResult<F>>;

/// An element to insert, with its children (`ITreeElement`).
///
/// Upstream `children` is a lazy iterable; here it is a `Vec`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeElement<T> {
    /// The element.
    pub element: T,
    /// The element's children.
    pub children: Vec<TreeElement<T>>,
    /// Whether the node can be collapsed; defaults to whether `collapsed` is set.
    pub collapsible: Option<bool>,
    /// Whether the node starts collapsed; defaults to the model's `collapse_by_default`.
    pub collapsed: Option<bool>,
}

impl<T> TreeElement<T> {
    /// A leaf with default collapse state.
    pub fn new(element: T) -> Self {
        Self {
            element,
            children: Vec::new(),
            collapsible: None,
            collapsed: None,
        }
    }

    /// Replaces the children.
    pub fn with_children(mut self, children: Vec<TreeElement<T>>) -> Self {
        self.children = children;
        self
    }

    /// Sets `collapsed`.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = Some(collapsed);
        self
    }

    /// Sets `collapsible`.
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = Some(collapsible);
        self
    }
}

/// A handle to a node in a tree model's arena.
///
/// Handles of deleted nodes go stale: the model's `node(id)` returns `None` for them, even after
/// their slot is reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// A change to the rendered (flattened, visible) node list (`ITreeListSpliceData`).
///
/// A list view applies it as `list.splice(start..start + delete_count, elements)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeListSplice {
    /// List index of the first replaced row.
    pub start: usize,
    /// Number of rows removed.
    pub delete_count: usize,
    /// Nodes of the inserted rows, in order.
    pub elements: Vec<NodeId>,
}

/// A change to the model's direct children of one parent (`ITreeModelSpliceEvent`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeModelSplice {
    /// The inserted top-level nodes.
    pub inserted_nodes: Vec<NodeId>,
    /// The deleted top-level nodes. These handles are already stale when listeners run.
    pub deleted_nodes: Vec<NodeId>,
}

/// A node's collapsible or collapsed state changed (`ICollapseStateChangeEvent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollapseStateChange {
    /// The node that changed.
    pub node: NodeId,
    /// Whether it changed as a descendant of a recursive update.
    pub deep: bool,
}

/// An invalid tree operation (`TreeError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeError {
    /// The tree's user label, given at construction.
    pub user: String,
    /// What went wrong.
    pub message: String,
}

impl TreeError {
    pub(crate) fn new(user: &str, message: impl Into<String>) -> Self {
        Self {
            user: user.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TreeError [{}] {}", self.user, self.message)
    }
}

impl std::error::Error for TreeError {}
