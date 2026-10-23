use std::collections::HashSet;

use remindr_core::application::TextPoint;
use remindr_core::domain::{BlockId, BlockKind, Parent, Workspace};

#[derive(Clone, Copy)]
pub enum Expansion<'a> {
    All,
    Only(&'a HashSet<BlockId>),
}

impl Expansion<'_> {
    fn shows_children_of(&self, toggle: BlockId) -> bool {
        match self {
            Self::All => true,
            Self::Only(open) => open.contains(&toggle),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub block: BlockId,
    pub depth: usize,
    pub number: Option<usize>,
}

pub fn rows(workspace: &Workspace, page: BlockId, expansion: Expansion) -> Vec<Row> {
    let mut rows = Vec::new();
    collect(workspace, Parent::Block(page), 0, expansion, &mut rows);
    rows
}

fn collect(
    workspace: &Workspace,
    parent: Parent,
    depth: usize,
    expansion: Expansion,
    rows: &mut Vec<Row>,
) {
    let mut run = 0;
    for block in workspace.children(parent) {
        let number = if matches!(block.kind, BlockKind::NumberedListItem) {
            run += 1;
            Some(run)
        } else {
            run = 0;
            None
        };
        rows.push(Row {
            block: block.id,
            depth,
            number,
        });
        let descend = match block.kind {
            BlockKind::Page { .. } => false,
            BlockKind::Toggle => expansion.shows_children_of(block.id),
            _ => true,
        };
        if descend {
            collect(
                workspace,
                Parent::Block(block.id),
                depth + 1,
                expansion,
                rows,
            );
        }
    }
}

pub fn position(rows: &[Row], block: BlockId) -> Option<usize> {
    rows.iter().position(|row| row.block == block)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub first: usize,
    pub start: TextPoint,
    pub last: usize,
    pub end: TextPoint,
}

pub fn extent(rows: &[Row], anchor: TextPoint, head: TextPoint) -> Option<Extent> {
    let from = position(rows, anchor.block)?;
    let to = position(rows, head.block)?;
    Some(if (from, anchor.offset) <= (to, head.offset) {
        Extent {
            first: from,
            start: anchor,
            last: to,
            end: head,
        }
    } else {
        Extent {
            first: to,
            start: head,
            last: from,
            end: anchor,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockKind, Parent};

    use super::{Expansion, Row, extent, rows};
    use crate::editor::testing::Doc;

    #[test]
    fn rows_follow_document_order_and_depth() {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "first");
        let nested = doc.under(first, BlockKind::BulletedListItem, "nested");
        let deeper = doc.under(nested, BlockKind::Text, "deeper");
        let second = doc.child(BlockKind::Text, "second");
        let workspace = doc.workspace();
        let found = rows(&workspace, doc.page(), Expansion::All);
        assert_eq!(
            found,
            vec![
                Row {
                    block: first,
                    depth: 0,
                    number: None
                },
                Row {
                    block: nested,
                    depth: 1,
                    number: None
                },
                Row {
                    block: deeper,
                    depth: 2,
                    number: None
                },
                Row {
                    block: second,
                    depth: 0,
                    number: None
                },
            ]
        );
    }

    #[test]
    fn numbered_items_count_consecutive_siblings() {
        let mut doc = Doc::new();
        let one = doc.child(BlockKind::NumberedListItem, "a");
        let two = doc.child(BlockKind::NumberedListItem, "b");
        let text = doc.child(BlockKind::Text, "break");
        let again = doc.child(BlockKind::NumberedListItem, "c");
        let child = doc.under(again, BlockKind::NumberedListItem, "d");
        let workspace = doc.workspace();
        let numbers: Vec<(_, _)> = rows(&workspace, doc.page(), Expansion::All)
            .into_iter()
            .map(|row| (row.block, row.number))
            .collect();
        assert_eq!(
            numbers,
            vec![
                (one, Some(1)),
                (two, Some(2)),
                (text, None),
                (again, Some(1)),
                (child, Some(1)),
            ]
        );
    }

    #[test]
    fn collapsed_toggle_hides_children() {
        let mut doc = Doc::new();
        let toggle = doc.child(BlockKind::Toggle, "toggle");
        let hidden = doc.under(toggle, BlockKind::Text, "hidden");
        let after = doc.child(BlockKind::Text, "after");
        let workspace = doc.workspace();
        let closed = HashSet::new();
        let visible: Vec<_> = rows(&workspace, doc.page(), Expansion::Only(&closed))
            .into_iter()
            .map(|row| row.block)
            .collect();
        assert_eq!(visible, vec![toggle, after]);
        let open = HashSet::from([toggle]);
        let expanded: Vec<_> = rows(&workspace, doc.page(), Expansion::Only(&open))
            .into_iter()
            .map(|row| row.block)
            .collect();
        assert_eq!(expanded, vec![toggle, hidden, after]);
    }

    #[test]
    fn nested_pages_are_leaves_of_the_flow() {
        let mut doc = Doc::new();
        let sub = doc.child(BlockKind::Page { icon: None }, "Sub");
        doc.under(sub, BlockKind::Text, "inside the sub page");
        let after = doc.child(BlockKind::Text, "after");
        let workspace = doc.workspace();
        let visible: Vec<_> = rows(&workspace, doc.page(), Expansion::All)
            .into_iter()
            .map(|row| row.block)
            .collect();
        assert_eq!(visible, vec![sub, after]);
        assert_eq!(workspace.children(Parent::Block(sub)).count(), 1);
    }

    #[test]
    fn extent_orders_points_by_row_then_offset() {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "one");
        let second = doc.child(BlockKind::Text, "two");
        let workspace = doc.workspace();
        let found = rows(&workspace, doc.page(), Expansion::All);
        let early = TextPoint {
            block: first,
            offset: 2,
        };
        let late = TextPoint {
            block: second,
            offset: 1,
        };
        let forward = extent(&found, early, late).expect("extent");
        let backward = extent(&found, late, early).expect("extent");
        assert_eq!(forward, backward);
        assert_eq!((forward.first, forward.last), (0, 1));
        assert_eq!((forward.start, forward.end), (early, late));
    }
}
