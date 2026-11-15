//! Arena red-black tree with subtree metadata.
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/rbTreeBase.ts` (`TreeNode`,
//! `leftRotate`, `rightRotate`, `rbDelete`, `fixInsert`, `updateTreeMetadata`,
//! `recomputeTreeMetadata`). Nodes live in a `Vec` and refer to each other by index. Index
//! [`NIL`] plays the role of `SENTINEL`: it is black, has an empty piece, and its `parent` field
//! is scratch space used by the delete fix-up, exactly as in the TypeScript.
//!
//! Each node stores `size_left` and `lf_left`, the length and line-feed count of its left
//! subtree. Metadata updates use wrapping arithmetic: the final values are non-negative and
//! correct even when an intermediate step would be negative.

use crate::piece::Piece;

/// Index of the sentinel node (`SENTINEL`).
pub(crate) const NIL: usize = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Color {
    Black,
    Red,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Node {
    pub(crate) parent: usize,
    pub(crate) left: usize,
    pub(crate) right: usize,
    pub(crate) color: Color,
    pub(crate) piece: Piece,
    /// Total length of the left subtree.
    pub(crate) size_left: usize,
    /// Total line feeds in the left subtree.
    pub(crate) lf_left: usize,
}

/// The tree: an arena of nodes and the index of the root.
#[derive(Debug)]
pub(crate) struct RbTree {
    nodes: Vec<Node>,
    free: Vec<usize>,
    pub(crate) root: usize,
}

impl RbTree {
    pub(crate) fn new() -> Self {
        let sentinel = Node {
            parent: NIL,
            left: NIL,
            right: NIL,
            color: Color::Black,
            piece: Piece::default(),
            size_left: 0,
            lf_left: 0,
        };
        Self {
            nodes: vec![sentinel],
            free: Vec::new(),
            root: NIL,
        }
    }

    pub(crate) fn node(&self, id: usize) -> &Node {
        &self.nodes[id]
    }

    pub(crate) fn piece(&self, id: usize) -> Piece {
        self.nodes[id].piece
    }

    pub(crate) fn set_piece(&mut self, id: usize, piece: Piece) {
        self.nodes[id].piece = piece;
    }

    fn n(&self, id: usize) -> &Node {
        &self.nodes[id]
    }

    fn m(&mut self, id: usize) -> &mut Node {
        &mut self.nodes[id]
    }

    fn alloc(&mut self, piece: Piece, color: Color) -> usize {
        let node = Node {
            parent: NIL,
            left: NIL,
            right: NIL,
            color,
            piece,
            size_left: 0,
            lf_left: 0,
        };
        match self.free.pop() {
            Some(id) => {
                self.nodes[id] = node;
                id
            }
            None => {
                self.nodes.push(node);
                self.nodes.len() - 1
            }
        }
    }

    /// Returns a detached node's slot to the arena.
    fn release(&mut self, id: usize) {
        debug_assert_ne!(id, NIL);
        self.free.push(id);
    }

    /// In-order successor (`TreeNode.next`). `NIL` past the last node.
    pub(crate) fn next(&self, id: usize) -> usize {
        if id == NIL {
            return NIL;
        }
        if self.n(id).right != NIL {
            return self.leftest(self.n(id).right);
        }
        let mut node = id;
        while self.n(node).parent != NIL {
            if self.n(self.n(node).parent).left == node {
                break;
            }
            node = self.n(node).parent;
        }
        if self.n(node).parent == NIL {
            NIL
        } else {
            self.n(node).parent
        }
    }

    /// In-order predecessor (`TreeNode.prev`). `NIL` before the first node.
    pub(crate) fn prev(&self, id: usize) -> usize {
        if id == NIL {
            return NIL;
        }
        if self.n(id).left != NIL {
            return self.rightest(self.n(id).left);
        }
        let mut node = id;
        while self.n(node).parent != NIL {
            if self.n(self.n(node).parent).right == node {
                break;
            }
            node = self.n(node).parent;
        }
        if self.n(node).parent == NIL {
            NIL
        } else {
            self.n(node).parent
        }
    }

    /// The first node of the subtree rooted at `id` (`leftest`).
    pub(crate) fn leftest(&self, mut id: usize) -> usize {
        while self.n(id).left != NIL {
            id = self.n(id).left;
        }
        id
    }

    /// The last node of the subtree rooted at `id` (`righttest`).
    fn rightest(&self, mut id: usize) -> usize {
        while self.n(id).right != NIL {
            id = self.n(id).right;
        }
        id
    }

    /// Total length of the subtree rooted at `id` (`calculateSize`).
    fn calc_size(&self, mut id: usize) -> usize {
        let mut total: usize = 0;
        while id != NIL {
            let n = self.n(id);
            total = total.wrapping_add(n.size_left).wrapping_add(n.piece.length);
            id = n.right;
        }
        total
    }

    /// Total line feeds in the subtree rooted at `id` (`calculateLF`).
    fn calc_lf(&self, mut id: usize) -> usize {
        let mut total: usize = 0;
        while id != NIL {
            let n = self.n(id);
            total = total
                .wrapping_add(n.lf_left)
                .wrapping_add(n.piece.line_feed_cnt);
            id = n.right;
        }
        total
    }

    fn left_rotate(&mut self, x: usize) {
        let y = self.n(x).right;
        let x_node = *self.n(x);

        // fix size_left
        let y_node = self.m(y);
        y_node.size_left = y_node
            .size_left
            .wrapping_add(x_node.size_left)
            .wrapping_add(x_node.piece.length);
        y_node.lf_left = y_node
            .lf_left
            .wrapping_add(x_node.lf_left)
            .wrapping_add(x_node.piece.line_feed_cnt);

        let y_left = self.n(y).left;
        self.m(x).right = y_left;
        if y_left != NIL {
            self.m(y_left).parent = x;
        }
        let x_parent = self.n(x).parent;
        self.m(y).parent = x_parent;
        if x_parent == NIL {
            self.root = y;
        } else if self.n(x_parent).left == x {
            self.m(x_parent).left = y;
        } else {
            self.m(x_parent).right = y;
        }
        self.m(y).left = x;
        self.m(x).parent = y;
    }

    fn right_rotate(&mut self, y: usize) {
        let x = self.n(y).left;
        let x_node = *self.n(x);
        let y_left = x_node.right;
        self.m(y).left = y_left;
        if y_left != NIL {
            self.m(y_left).parent = y;
        }
        self.m(x).parent = self.n(y).parent;

        // fix size_left
        let y_node = self.m(y);
        y_node.size_left = y_node
            .size_left
            .wrapping_sub(x_node.size_left)
            .wrapping_sub(x_node.piece.length);
        y_node.lf_left = y_node
            .lf_left
            .wrapping_sub(x_node.lf_left)
            .wrapping_sub(x_node.piece.line_feed_cnt);

        let y_parent = self.n(y).parent;
        if y_parent == NIL {
            self.root = x;
        } else if y == self.n(y_parent).right {
            self.m(y_parent).right = x;
        } else {
            self.m(y_parent).left = x;
        }
        self.m(x).right = y;
        self.m(y).parent = x;
    }

    /// Adds `delta` and `lf_delta` to `size_left`/`lf_left` of every ancestor whose left
    /// subtree contains `x` (`updateTreeMetadata`).
    pub(crate) fn update_metadata(&mut self, mut x: usize, delta: isize, lf_delta: isize) {
        while x != self.root && x != NIL {
            let p = self.n(x).parent;
            if self.n(p).left == x {
                let pn = self.m(p);
                pn.size_left = pn.size_left.wrapping_add_signed(delta);
                pn.lf_left = pn.lf_left.wrapping_add_signed(lf_delta);
            }
            x = p;
        }
    }

    /// Recomputes the metadata along the path above `x` after its subtree changed
    /// (`recomputeTreeMetadata`).
    fn recompute_metadata(&mut self, mut x: usize) {
        if x == self.root {
            return;
        }
        // go upwards till the node whose left subtree is changed.
        while x != self.root && x == self.n(self.n(x).parent).right {
            x = self.n(x).parent;
        }
        if x == self.root {
            // the change is at the end (in order): no left subtree above it changes
            return;
        }
        // x is the node whose right subtree changed.
        x = self.n(x).parent;

        let left = self.n(x).left;
        let delta = self.calc_size(left).wrapping_sub(self.n(x).size_left) as isize;
        let lf_delta = self.calc_lf(left).wrapping_sub(self.n(x).lf_left) as isize;
        let xn = self.m(x);
        xn.size_left = xn.size_left.wrapping_add_signed(delta);
        xn.lf_left = xn.lf_left.wrapping_add_signed(lf_delta);

        while x != self.root && (delta != 0 || lf_delta != 0) {
            let p = self.n(x).parent;
            if self.n(p).left == x {
                let pn = self.m(p);
                pn.size_left = pn.size_left.wrapping_add_signed(delta);
                pn.lf_left = pn.lf_left.wrapping_add_signed(lf_delta);
            }
            x = p;
        }
    }

    /// Inserts `piece` immediately after `node` in order (`rbInsertRight`). With an empty tree,
    /// `node` is ignored and the piece becomes the root.
    pub(crate) fn insert_right(&mut self, node: usize, piece: Piece) -> usize {
        let z = self.alloc(piece, Color::Red);
        if self.root == NIL {
            self.root = z;
            self.m(z).color = Color::Black;
        } else if self.n(node).right == NIL {
            self.m(node).right = z;
            self.m(z).parent = node;
        } else {
            let next = self.leftest(self.n(node).right);
            self.m(next).left = z;
            self.m(z).parent = next;
        }
        self.fix_insert(z);
        z
    }

    /// Inserts `piece` immediately before `node` in order (`rbInsertLeft`). With an empty tree,
    /// `node` is ignored and the piece becomes the root.
    pub(crate) fn insert_left(&mut self, node: usize, piece: Piece) -> usize {
        let z = self.alloc(piece, Color::Red);
        if self.root == NIL {
            self.root = z;
            self.m(z).color = Color::Black;
        } else if self.n(node).left == NIL {
            self.m(node).left = z;
            self.m(z).parent = node;
        } else {
            let prev = self.rightest(self.n(node).left);
            self.m(prev).right = z;
            self.m(z).parent = prev;
        }
        self.fix_insert(z);
        z
    }

    fn fix_insert(&mut self, mut x: usize) {
        self.recompute_metadata(x);

        while x != self.root && self.n(self.n(x).parent).color == Color::Red {
            let parent = self.n(x).parent;
            let grand = self.n(parent).parent;
            if parent == self.n(grand).left {
                let y = self.n(grand).right;
                if self.n(y).color == Color::Red {
                    self.m(parent).color = Color::Black;
                    self.m(y).color = Color::Black;
                    self.m(grand).color = Color::Red;
                    x = grand;
                } else {
                    if x == self.n(parent).right {
                        x = parent;
                        self.left_rotate(x);
                    }
                    let parent = self.n(x).parent;
                    let grand = self.n(parent).parent;
                    self.m(parent).color = Color::Black;
                    self.m(grand).color = Color::Red;
                    self.right_rotate(grand);
                }
            } else {
                let y = self.n(grand).left;
                if self.n(y).color == Color::Red {
                    self.m(parent).color = Color::Black;
                    self.m(y).color = Color::Black;
                    self.m(grand).color = Color::Red;
                    x = grand;
                } else {
                    if x == self.n(parent).left {
                        x = parent;
                        self.right_rotate(x);
                    }
                    let parent = self.n(x).parent;
                    let grand = self.n(parent).parent;
                    self.m(parent).color = Color::Black;
                    self.m(grand).color = Color::Red;
                    self.left_rotate(grand);
                }
            }
        }

        let root = self.root;
        self.m(root).color = Color::Black;
    }

    /// Removes node `z` from the tree and releases it (`rbDelete`).
    pub(crate) fn delete(&mut self, z: usize) {
        let (mut x, y);
        if self.n(z).left == NIL {
            y = z;
            x = self.n(y).right;
        } else if self.n(z).right == NIL {
            y = z;
            x = self.n(y).left;
        } else {
            y = self.leftest(self.n(z).right);
            x = self.n(y).right;
        }

        if y == self.root {
            self.root = x;
            // if x is NIL, we are removing the only node
            self.m(x).color = Color::Black;
            self.release(z);
            self.m(NIL).parent = NIL;
            let root = self.root;
            self.m(root).parent = NIL;
            return;
        }

        let y_was_red = self.n(y).color == Color::Red;
        let y_parent = self.n(y).parent;
        if y == self.n(y_parent).left {
            self.m(y_parent).left = x;
        } else {
            self.m(y_parent).right = x;
        }

        if y == z {
            let p = self.n(y).parent;
            self.m(x).parent = p;
            self.recompute_metadata(x);
        } else {
            if self.n(y).parent == z {
                self.m(x).parent = y;
            } else {
                self.m(x).parent = self.n(y).parent;
            }

            // as we make changes to x's hierarchy, update size_left of subtree first
            self.recompute_metadata(x);

            let zn = *self.n(z);
            {
                let yn = self.m(y);
                yn.left = zn.left;
                yn.right = zn.right;
                yn.parent = zn.parent;
                yn.color = zn.color;
            }
            if z == self.root {
                self.root = y;
            } else if z == self.n(zn.parent).left {
                self.m(zn.parent).left = y;
            } else {
                self.m(zn.parent).right = y;
            }
            let (yl, yr) = (self.n(y).left, self.n(y).right);
            if yl != NIL {
                self.m(yl).parent = y;
            }
            if yr != NIL {
                self.m(yr).parent = y;
            }
            // y takes z's place, so in this subtree the length change is z's length
            {
                let yn = self.m(y);
                yn.size_left = zn.size_left;
                yn.lf_left = zn.lf_left;
            }
            self.recompute_metadata(y);
        }

        self.release(z);

        let x_parent = self.n(x).parent;
        if self.n(x_parent).left == x {
            let new_size_left = self.calc_size(x);
            let new_lf_left = self.calc_lf(x);
            let pn = *self.n(x_parent);
            if new_size_left != pn.size_left || new_lf_left != pn.lf_left {
                let delta = new_size_left.wrapping_sub(pn.size_left) as isize;
                let lf_delta = new_lf_left.wrapping_sub(pn.lf_left) as isize;
                let pm = self.m(x_parent);
                pm.size_left = new_size_left;
                pm.lf_left = new_lf_left;
                self.update_metadata(x_parent, delta, lf_delta);
            }
        }

        let x_parent = self.n(x).parent;
        self.recompute_metadata(x_parent);

        if y_was_red {
            self.m(NIL).parent = NIL;
            return;
        }

        // RB-DELETE-FIXUP
        while x != self.root && self.n(x).color == Color::Black {
            let parent = self.n(x).parent;
            if x == self.n(parent).left {
                let mut w = self.n(parent).right;
                if self.n(w).color == Color::Red {
                    self.m(w).color = Color::Black;
                    self.m(parent).color = Color::Red;
                    self.left_rotate(parent);
                    w = self.n(self.n(x).parent).right;
                }

                let (wl, wr) = (self.n(w).left, self.n(w).right);
                if self.n(wl).color == Color::Black && self.n(wr).color == Color::Black {
                    self.m(w).color = Color::Red;
                    x = self.n(x).parent;
                } else {
                    if self.n(wr).color == Color::Black {
                        self.m(wl).color = Color::Black;
                        self.m(w).color = Color::Red;
                        self.right_rotate(w);
                        w = self.n(parent).right;
                    }

                    self.m(w).color = self.n(parent).color;
                    self.m(parent).color = Color::Black;
                    let wr = self.n(w).right;
                    self.m(wr).color = Color::Black;
                    self.left_rotate(parent);
                    x = self.root;
                }
            } else {
                let mut w = self.n(parent).left;
                if self.n(w).color == Color::Red {
                    self.m(w).color = Color::Black;
                    self.m(parent).color = Color::Red;
                    self.right_rotate(parent);
                    w = self.n(self.n(x).parent).left;
                }

                let (wl, wr) = (self.n(w).left, self.n(w).right);
                if self.n(wl).color == Color::Black && self.n(wr).color == Color::Black {
                    self.m(w).color = Color::Red;
                    x = self.n(x).parent;
                } else {
                    if self.n(wl).color == Color::Black {
                        self.m(wr).color = Color::Black;
                        self.m(w).color = Color::Red;
                        self.left_rotate(w);
                        w = self.n(parent).left;
                    }

                    self.m(w).color = self.n(parent).color;
                    self.m(parent).color = Color::Black;
                    let wl = self.n(w).left;
                    self.m(wl).color = Color::Black;
                    self.right_rotate(parent);
                    x = self.root;
                }
            }
        }
        self.m(x).color = Color::Black;
        self.m(NIL).parent = NIL;
    }
}

#[cfg(test)]
impl RbTree {
    /// Checks the red-black and metadata invariants (`assertValidTree`). Panics on violation.
    pub(crate) fn assert_valid(&self) {
        let sentinel = self.n(NIL);
        assert_eq!(sentinel.color, Color::Black);
        assert_eq!(
            (sentinel.parent, sentinel.left, sentinel.right),
            (NIL, NIL, NIL)
        );
        assert_eq!((sentinel.size_left, sentinel.lf_left), (0, 0));
        if self.root == NIL {
            return;
        }
        assert_eq!(self.n(self.root).color, Color::Black);
        assert_eq!(
            self.black_depth(self.n(self.root).left),
            self.black_depth(self.n(self.root).right)
        );
        self.check_subtree(self.root);
    }

    fn black_depth(&self, id: usize) -> usize {
        if id == NIL {
            return 1;
        }
        let n = self.n(id);
        assert_eq!(self.black_depth(n.left), self.black_depth(n.right));
        usize::from(n.color == Color::Black) + self.black_depth(n.left)
    }

    /// Returns `(length, line feeds)` of the subtree and checks every node on the way.
    fn check_subtree(&self, id: usize) -> (usize, usize) {
        if id == NIL {
            return (0, 0);
        }
        let n = self.n(id);
        if n.color == Color::Red {
            assert_eq!(self.n(n.left).color, Color::Black);
            assert_eq!(self.n(n.right).color, Color::Black);
        }
        let left = self.check_subtree(n.left);
        assert_eq!(left, (n.size_left, n.lf_left), "left subtree metadata");
        let right = self.check_subtree(n.right);
        (
            n.size_left + n.piece.length + right.0,
            n.lf_left + n.piece.line_feed_cnt + right.1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::Cursor;

    /// A piece that only carries a length, which is all the tree metadata looks at here.
    fn piece(length: usize) -> Piece {
        Piece::new(0, Cursor::default(), Cursor::default(), 0, length)
    }

    /// Node ids in order, found by walking `next` from the first node.
    fn in_order(tree: &RbTree) -> Vec<usize> {
        let mut ids = Vec::new();
        let mut x = if tree.root == NIL {
            NIL
        } else {
            tree.leftest(tree.root)
        };
        while x != NIL {
            ids.push(x);
            x = tree.next(x);
        }
        ids
    }

    /// Random inserts at every position and deletes of random nodes, checked against a `Vec`
    /// model after each step. Seeded, so the sequence is reproducible.
    #[test]
    fn random_inserts_and_deletes_keep_order_and_metadata() {
        for seed in 1..=100u64 {
            let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let mut below = |bound: usize| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state % bound as u64) as usize
            };

            let mut tree = RbTree::new();
            let mut ids: Vec<usize> = Vec::new();
            let mut lengths: Vec<usize> = Vec::new();
            for _ in 0..200 {
                if ids.is_empty() || below(3) != 0 {
                    let at = below(ids.len() + 1);
                    let length = 1 + below(9);
                    let id = if ids.is_empty() {
                        tree.insert_right(NIL, piece(length))
                    } else if at == 0 {
                        tree.insert_left(ids[0], piece(length))
                    } else {
                        tree.insert_right(ids[at - 1], piece(length))
                    };
                    ids.insert(at, id);
                    lengths.insert(at, length);
                } else {
                    let at = below(ids.len());
                    tree.delete(ids[at]);
                    ids.remove(at);
                    lengths.remove(at);
                }

                tree.assert_valid();
                assert_eq!(in_order(&tree), ids, "seed {seed}");
                let stored: Vec<usize> = ids.iter().map(|&id| tree.piece(id).length).collect();
                assert_eq!(stored, lengths, "seed {seed}");
            }
        }
    }

    /// Deleting every node leaves an empty tree that accepts inserts again.
    #[test]
    fn delete_all_then_reuse() {
        let mut tree = RbTree::new();
        let mut ids: Vec<usize> = Vec::new();
        for n in 1..=20 {
            let after = ids.last().copied().unwrap_or(NIL);
            ids.push(tree.insert_right(after, piece(n)));
        }
        for id in ids {
            tree.delete(id);
            tree.assert_valid();
        }
        assert_eq!(tree.root, NIL);
        let id = tree.insert_right(NIL, piece(5));
        assert_eq!(in_order(&tree), vec![id]);
        tree.assert_valid();
    }
}
