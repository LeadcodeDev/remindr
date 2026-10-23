use std::collections::HashSet;

use crate::domain::{
    Block, BlockId, BlockKind, DomainError, Emoji, MarkChange, Operation, Parent, RichText,
    SortKey, Timestamp, Transaction, Workspace,
};

use super::{Built, Command, Created, DropTarget, TextPoint};

pub(crate) fn build(
    workspace: &Workspace,
    command: &Command,
    now: Timestamp,
) -> Result<Built, DomainError> {
    match command {
        Command::CreatePage { parent, after } => create_page(workspace, *parent, *after, now),
        Command::InsertBlock { after, kind } => insert_block(workspace, *after, kind, now),
        Command::InsertBlockAt { target, kind } => insert_block_at(workspace, *target, kind, now),
        Command::EditText { block, text } => edit_text(workspace, *block, text),
        Command::SplitBlock { at } => split_block(workspace, *at, now),
        Command::MergeWithPrevious { block } => merge_with_previous(workspace, *block),
        Command::DeleteRange { start, end } => delete_range(workspace, *start, *end),
        Command::SetMarks { start, end, change } => set_marks(workspace, *start, *end, change),
        Command::Indent { block } => indent(workspace, *block),
        Command::Outdent { block } => outdent(workspace, *block),
        Command::ChangeKind { block, kind } => change_kind(workspace, *block, kind),
        Command::MoveBlock { block, target } => move_block(workspace, *block, *target),
        Command::DeleteBlocks { blocks } => delete_blocks(workspace, blocks),
        Command::SetPageIcon { page, icon } => set_page_icon(workspace, *page, icon),
        _ => Err(DomainError::InvalidParent),
    }
}

struct Selection {
    start: TextPoint,
    end: TextPoint,
    flow: Vec<BlockId>,
    first: usize,
    last: usize,
}

fn create_page(
    workspace: &Workspace,
    parent: Parent,
    after: Option<BlockId>,
    now: Timestamp,
) -> Result<Built, DomainError> {
    if matches!(parent, Parent::Database(_)) {
        return Err(DomainError::InvalidParent);
    }
    let anchor = target_key(workspace, parent, after)?;
    let sort_key = key_after(workspace, parent, anchor, &[]);
    let page = new_block(
        BlockKind::Page { icon: None },
        parent,
        sort_key,
        RichText::default(),
        now,
    );
    Ok(created(page))
}

fn insert_block(
    workspace: &Workspace,
    after: BlockId,
    kind: &BlockKind,
    now: Timestamp,
) -> Result<Built, DomainError> {
    if matches!(
        kind,
        BlockKind::Page { .. } | BlockKind::DatabaseView { .. }
    ) {
        return Err(DomainError::InvalidParent);
    }
    let previous = block_of(workspace, after)?;
    let block = sibling_after(workspace, previous, kind.clone(), RichText::default(), now);
    Ok(created(block))
}

fn insert_block_at(
    workspace: &Workspace,
    target: DropTarget,
    kind: &BlockKind,
    now: Timestamp,
) -> Result<Built, DomainError> {
    if matches!(
        kind,
        BlockKind::Page { .. } | BlockKind::DatabaseView { .. }
    ) {
        return Err(DomainError::InvalidParent);
    }
    let anchor = target_key(workspace, target.parent, target.after)?;
    let sort_key = key_after(workspace, target.parent, anchor, &[]);
    let block = new_block(
        kind.clone(),
        target.parent,
        sort_key,
        RichText::default(),
        now,
    );
    Ok(created(block))
}

fn edit_text(workspace: &Workspace, id: BlockId, text: &RichText) -> Result<Built, DomainError> {
    let block = block_of(workspace, id)?;
    if block.kind.is_atomic() && !text.is_empty() {
        return Err(DomainError::TextNotAllowed);
    }
    Ok(Built {
        transaction: set_text_if_changed(block, text.clone()),
        ..Built::default()
    })
}

fn split_block(workspace: &Workspace, at: TextPoint, now: Timestamp) -> Result<Built, DomainError> {
    let block = block_of(workspace, at.block)?;
    if matches!(
        block.kind,
        BlockKind::Divider | BlockKind::DatabaseView { .. }
    ) {
        let empty = sibling_after(workspace, block, BlockKind::Text, RichText::default(), now);
        return Ok(created(empty));
    }
    let (head, tail) = block.text.split_at(at.offset)?;
    if matches!(block.kind, BlockKind::Page { .. }) {
        let inside = Parent::Block(block.id);
        let sort_key = key_after(workspace, inside, None, &[]);
        let first_child = new_block(BlockKind::Text, inside, sort_key, tail, now);
        return Ok(split_into(block, head, first_child));
    }
    if block.text.is_empty() && is_continuable(&block.kind) {
        return Ok(Built {
            transaction: Transaction::new(vec![Operation::SetKind {
                block: block.id,
                kind: BlockKind::Text,
            }]),
            caret: Some(TextPoint {
                block: block.id,
                offset: 0,
            }),
            ..Built::default()
        });
    }
    let kind = match &block.kind {
        BlockKind::BulletedListItem | BlockKind::NumberedListItem => block.kind.clone(),
        BlockKind::ToDo { .. } => BlockKind::ToDo { checked: false },
        _ => BlockKind::Text,
    };
    let next = sibling_after(workspace, block, kind, tail, now);
    Ok(split_into(block, head, next))
}

fn split_into(block: &Block, head: RichText, next: Block) -> Built {
    let next_id = next.id;
    let mut operations = set_text_if_changed(block, head).operations().to_vec();
    operations.push(Operation::InsertBlock(next));
    Built {
        transaction: Transaction::new(operations),
        created: vec![Created::Block(next_id)],
        caret: Some(TextPoint {
            block: next_id,
            offset: 0,
        }),
    }
}

fn merge_with_previous(workspace: &Workspace, id: BlockId) -> Result<Built, DomainError> {
    let block = block_of(workspace, id)?;
    if matches!(block.kind, BlockKind::Page { .. }) {
        return Ok(Built::default());
    }
    let flow = flow_of(workspace, block);
    let position = position_in(&flow, id)?;
    let Some(previous_id) = position
        .checked_sub(1)
        .and_then(|index| flow.get(index))
        .copied()
    else {
        return Ok(Built::default());
    };
    let previous = block_of(workspace, previous_id)?;
    if matches!(previous.kind, BlockKind::Page { .. }) {
        return Ok(Built::default());
    }
    if previous.kind.is_atomic() {
        return Ok(Built {
            transaction: Transaction::new(vec![Operation::SetBlockAlive {
                block: previous_id,
                alive: false,
            }]),
            caret: Some(TextPoint {
                block: id,
                offset: 0,
            }),
            ..Built::default()
        });
    }
    let merged = previous.text.concat(&block.text);
    let mut operations = set_text_if_changed(previous, merged).operations().to_vec();
    let children: Vec<BlockId> = workspace
        .children(Parent::Block(id))
        .map(|child| child.id)
        .collect();
    let skipped: Vec<BlockId> = children.iter().copied().chain([id]).collect();
    let keys = keys_after(
        workspace,
        block.parent,
        Some(&block.sort_key),
        &skipped,
        children.len(),
    );
    operations.extend(
        children
            .iter()
            .zip(keys)
            .map(|(child, sort_key)| Operation::Move {
                block: *child,
                parent: block.parent,
                sort_key,
            }),
    );
    operations.push(Operation::SetBlockAlive {
        block: id,
        alive: false,
    });
    Ok(Built {
        transaction: Transaction::new(operations),
        caret: Some(TextPoint {
            block: previous_id,
            offset: previous.text.len(),
        }),
        ..Built::default()
    })
}

fn delete_range(
    workspace: &Workspace,
    from: TextPoint,
    to: TextPoint,
) -> Result<Built, DomainError> {
    let Selection {
        start,
        end,
        flow,
        first,
        last,
    } = select(workspace, from, to)?;
    let first_block = block_of(workspace, start.block)?;
    if first == last {
        let text = first_block.text.delete(start.offset..end.offset)?;
        return Ok(Built {
            transaction: set_text_if_changed(first_block, text),
            caret: Some(start),
            ..Built::default()
        });
    }
    let last_block = block_of(workspace, end.block)?;
    let head = text_before(first_block, start.offset)?;
    let tail = text_after(last_block, end.offset)?;
    let last_survives = first_block.kind.is_atomic() && !last_block.kind.is_atomic();
    let removed_from = if first_block.kind.is_atomic() {
        first
    } else {
        first + 1
    };
    let removed_to = if last_survives { last } else { last + 1 };
    let removed: Vec<BlockId> = flow
        .get(removed_from..removed_to)
        .unwrap_or_default()
        .to_vec();
    let removed_set: HashSet<BlockId> = removed.iter().copied().collect();
    let relocate_from = if last_survives { last } else { last + 1 };
    let moved: Vec<BlockId> = flow
        .iter()
        .skip(relocate_from)
        .copied()
        .filter(|id| {
            workspace.block(*id).is_some_and(|block| {
                matches!(block.parent, Parent::Block(owner) if removed_set.contains(&owner))
            })
        })
        .collect();
    let skipped: Vec<BlockId> = removed.iter().chain(&moved).copied().collect();
    let keys = keys_after(
        workspace,
        first_block.parent,
        Some(&first_block.sort_key),
        &skipped,
        moved.len(),
    );
    let (mut operations, caret) = if last_survives {
        let ops = set_text_if_changed(last_block, tail).operations().to_vec();
        (ops, Some(start_of(last_block.id)))
    } else if first_block.kind.is_atomic() {
        (Vec::new(), None)
    } else {
        let ops = set_text_if_changed(first_block, head.concat(&tail))
            .operations()
            .to_vec();
        (ops, Some(start))
    };
    operations.extend(
        moved
            .iter()
            .zip(keys)
            .map(|(block, sort_key)| Operation::Move {
                block: *block,
                parent: first_block.parent,
                sort_key,
            }),
    );
    let mut seen = HashSet::new();
    for id in &removed {
        let is_page = workspace
            .block(*id)
            .is_some_and(|block| matches!(block.kind, BlockKind::Page { .. }));
        if is_page {
            kill_subtree(workspace, *id, &mut seen, &mut operations);
        } else if seen.insert(*id) {
            operations.push(Operation::SetBlockAlive {
                block: *id,
                alive: false,
            });
        }
    }
    Ok(Built {
        transaction: Transaction::new(operations),
        caret,
        ..Built::default()
    })
}

fn start_of(block: BlockId) -> TextPoint {
    TextPoint { block, offset: 0 }
}

fn text_before(block: &Block, offset: usize) -> Result<RichText, DomainError> {
    if block.kind.is_atomic() {
        Ok(RichText::default())
    } else {
        block.text.split_at(offset).map(|(head, _)| head)
    }
}

fn text_after(block: &Block, offset: usize) -> Result<RichText, DomainError> {
    if block.kind.is_atomic() {
        Ok(RichText::default())
    } else {
        block.text.split_at(offset).map(|(_, tail)| tail)
    }
}

fn set_marks(
    workspace: &Workspace,
    from: TextPoint,
    to: TextPoint,
    change: &MarkChange,
) -> Result<Built, DomainError> {
    let Selection {
        start,
        end,
        flow,
        first,
        last,
    } = select(workspace, from, to)?;
    let mut operations = Vec::new();
    for id in flow.get(first..=last).unwrap_or_default() {
        let block = block_of(workspace, *id)?;
        if block.kind.is_atomic() || matches!(block.kind, BlockKind::Page { .. }) {
            continue;
        }
        let begin = if *id == start.block { start.offset } else { 0 };
        let finish = if *id == end.block {
            end.offset
        } else {
            block.text.len()
        };
        let marked = block.text.apply_mark(begin..finish, change)?;
        operations.extend(set_text_if_changed(block, marked).operations().to_vec());
    }
    Ok(Built {
        transaction: Transaction::new(operations),
        ..Built::default()
    })
}

fn indent(workspace: &Workspace, id: BlockId) -> Result<Built, DomainError> {
    let block = block_of(workspace, id)?;
    if !matches!(block.parent, Parent::Block(_)) {
        return Err(DomainError::InvalidParent);
    }
    let previous = workspace
        .children(block.parent)
        .take_while(|sibling| sibling.id != id)
        .last()
        .ok_or(DomainError::InvalidParent)?;
    if !previous.kind.accepts_children() {
        return Err(DomainError::ChildrenNotAllowed);
    }
    let target = Parent::Block(previous.id);
    let last_key = workspace
        .children
        .get(&target)
        .and_then(|siblings| siblings.last())
        .map(|(sort_key, _)| sort_key);
    Ok(moved(id, target, SortKey::between(last_key, None)))
}

fn outdent(workspace: &Workspace, id: BlockId) -> Result<Built, DomainError> {
    let block = block_of(workspace, id)?;
    let Parent::Block(owner_id) = block.parent else {
        return Err(DomainError::InvalidParent);
    };
    let owner = block_of(workspace, owner_id)?;
    if matches!(owner.kind, BlockKind::Page { .. }) {
        return Err(DomainError::InvalidParent);
    }
    let sort_key = key_after(workspace, owner.parent, Some(&owner.sort_key), &[id]);
    Ok(moved(id, owner.parent, sort_key))
}

fn change_kind(workspace: &Workspace, id: BlockId, kind: &BlockKind) -> Result<Built, DomainError> {
    let block = block_of(workspace, id)?;
    let fixed = |kind: &BlockKind| {
        matches!(
            kind,
            BlockKind::Page { .. } | BlockKind::DatabaseView { .. }
        )
    };
    if fixed(kind) || fixed(&block.kind) {
        return Err(DomainError::InvalidParent);
    }
    if block.kind == *kind {
        return Ok(Built::default());
    }
    let mut operations = Vec::new();
    if kind.is_atomic() {
        operations.extend(
            set_text_if_changed(block, RichText::default())
                .operations()
                .to_vec(),
        );
    }
    operations.push(Operation::SetKind {
        block: id,
        kind: kind.clone(),
    });
    Ok(Built {
        transaction: Transaction::new(operations),
        ..Built::default()
    })
}

fn move_block(
    workspace: &Workspace,
    id: BlockId,
    target: DropTarget,
) -> Result<Built, DomainError> {
    block_of(workspace, id)?;
    if target.after == Some(id) {
        return Ok(Built::default());
    }
    let anchor = target_key(workspace, target.parent, target.after)?;
    let sort_key = key_after(workspace, target.parent, anchor, &[id]);
    Ok(moved(id, target.parent, sort_key))
}

fn delete_blocks(workspace: &Workspace, blocks: &[BlockId]) -> Result<Built, DomainError> {
    let mut seen = HashSet::new();
    let mut operations = Vec::new();
    for id in blocks {
        let block = workspace.block(*id).ok_or(DomainError::UnknownBlock(*id))?;
        if block.alive {
            kill_subtree(workspace, *id, &mut seen, &mut operations);
        }
    }
    Ok(Built {
        transaction: Transaction::new(operations),
        ..Built::default()
    })
}

fn set_page_icon(
    workspace: &Workspace,
    id: BlockId,
    icon: &Option<Emoji>,
) -> Result<Built, DomainError> {
    let block = block_of(workspace, id)?;
    let BlockKind::Page { icon: current } = &block.kind else {
        return Err(DomainError::InvalidParent);
    };
    if current == icon {
        return Ok(Built::default());
    }
    Ok(Built {
        transaction: Transaction::new(vec![Operation::SetKind {
            block: id,
            kind: BlockKind::Page { icon: icon.clone() },
        }]),
        ..Built::default()
    })
}

fn kill_subtree(
    workspace: &Workspace,
    id: BlockId,
    seen: &mut HashSet<BlockId>,
    operations: &mut Vec<Operation>,
) {
    if seen.insert(id) {
        operations.push(Operation::SetBlockAlive {
            block: id,
            alive: false,
        });
        for child in workspace.children(Parent::Block(id)) {
            kill_subtree(workspace, child.id, seen, operations);
        }
    }
}

fn is_continuable(kind: &BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::BulletedListItem
            | BlockKind::NumberedListItem
            | BlockKind::ToDo { .. }
            | BlockKind::Quote
            | BlockKind::Callout { .. }
    )
}

fn block_of(workspace: &Workspace, id: BlockId) -> Result<&Block, DomainError> {
    workspace
        .block(id)
        .filter(|block| block.alive)
        .ok_or(DomainError::UnknownBlock(id))
}

fn new_block(
    kind: BlockKind,
    parent: Parent,
    sort_key: SortKey,
    text: RichText,
    now: Timestamp,
) -> Block {
    Block {
        id: BlockId::new(),
        kind,
        parent,
        sort_key,
        text,
        alive: true,
        created_at: now,
    }
}

fn created(block: Block) -> Built {
    let id = block.id;
    Built {
        transaction: Transaction::new(vec![Operation::InsertBlock(block)]),
        created: vec![Created::Block(id)],
        caret: Some(TextPoint {
            block: id,
            offset: 0,
        }),
    }
}

fn moved(id: BlockId, parent: Parent, sort_key: SortKey) -> Built {
    Built {
        transaction: Transaction::new(vec![Operation::Move {
            block: id,
            parent,
            sort_key,
        }]),
        ..Built::default()
    }
}

fn set_text_if_changed(block: &Block, text: RichText) -> Transaction {
    if block.text == text {
        Transaction::default()
    } else {
        Transaction::new(vec![Operation::SetText {
            block: block.id,
            text,
        }])
    }
}

fn sibling_after(
    workspace: &Workspace,
    previous: &Block,
    kind: BlockKind,
    text: RichText,
    now: Timestamp,
) -> Block {
    let sort_key = key_after(workspace, previous.parent, Some(&previous.sort_key), &[]);
    new_block(kind, previous.parent, sort_key, text, now)
}

fn target_key(
    workspace: &Workspace,
    parent: Parent,
    after: Option<BlockId>,
) -> Result<Option<&SortKey>, DomainError> {
    if let Parent::Block(owner) = parent {
        block_of(workspace, owner)?;
    }
    let Some(id) = after else {
        return Ok(None);
    };
    let anchor = block_of(workspace, id)?;
    if anchor.parent == parent {
        Ok(Some(&anchor.sort_key))
    } else {
        Err(DomainError::InvalidParent)
    }
}

fn next_key<'a>(
    workspace: &'a Workspace,
    parent: Parent,
    after: Option<&SortKey>,
    skipped: &[BlockId],
) -> Option<&'a SortKey> {
    workspace
        .children
        .get(&parent)
        .into_iter()
        .flatten()
        .find(|(sort_key, id)| {
            !skipped.contains(id) && after.is_none_or(|anchor| sort_key > anchor)
        })
        .map(|(sort_key, _)| sort_key)
}

fn key_after(
    workspace: &Workspace,
    parent: Parent,
    after: Option<&SortKey>,
    skipped: &[BlockId],
) -> SortKey {
    SortKey::between(after, next_key(workspace, parent, after, skipped))
}

fn keys_after(
    workspace: &Workspace,
    parent: Parent,
    after: Option<&SortKey>,
    skipped: &[BlockId],
    count: usize,
) -> Vec<SortKey> {
    let next = next_key(workspace, parent, after, skipped);
    let mut previous = after.cloned();
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        let sort_key = SortKey::between(previous.as_ref(), next);
        previous = Some(sort_key.clone());
        keys.push(sort_key);
    }
    keys
}

fn flow_of(workspace: &Workspace, block: &Block) -> Vec<BlockId> {
    let mut parent = block.parent;
    let page = loop {
        let Parent::Block(owner_id) = parent else {
            break None;
        };
        let Some(owner) = workspace.block(owner_id) else {
            break None;
        };
        if matches!(owner.kind, BlockKind::Page { .. }) {
            break Some(owner_id);
        }
        parent = owner.parent;
    };
    match page {
        Some(page) => {
            let mut flow = Vec::new();
            collect_flow(workspace, Parent::Block(page), &mut flow);
            flow
        }
        None => vec![block.id],
    }
}

fn collect_flow(workspace: &Workspace, parent: Parent, flow: &mut Vec<BlockId>) {
    for child in workspace.children(parent) {
        flow.push(child.id);
        if !matches!(child.kind, BlockKind::Page { .. }) {
            collect_flow(workspace, Parent::Block(child.id), flow);
        }
    }
}

fn position_in(flow: &[BlockId], id: BlockId) -> Result<usize, DomainError> {
    flow.iter()
        .position(|candidate| *candidate == id)
        .ok_or(DomainError::InvalidParent)
}

fn select(workspace: &Workspace, from: TextPoint, to: TextPoint) -> Result<Selection, DomainError> {
    let anchor = block_of(workspace, from.block)?;
    block_of(workspace, to.block)?;
    let flow = flow_of(workspace, anchor);
    let from_position = position_in(&flow, from.block)?;
    let to_position = position_in(&flow, to.block)?;
    let (start, end, first, last) = if (from_position, from.offset) <= (to_position, to.offset) {
        (from, to, from_position, to_position)
    } else {
        (to, from, to_position, from_position)
    };
    Ok(Selection {
        start,
        end,
        flow,
        first,
        last,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::build;
    use crate::application::{Built, Command, Created, DropTarget, TextPoint};
    use crate::domain::{
        Block, BlockId, BlockKind, CodeLanguage, Database, DatabaseId, DomainError, Emoji,
        HeadingLevel, MarkChange, Marks, Operation, Parent, Property, PropertyId, PropertyKind,
        PropertyValue, RichText, Schema, SortKey, Span, Timestamp, Transaction, Uuid, View, ViewId,
        ViewKind, Workspace, WorkspaceSnapshot,
    };

    fn id(n: u128) -> BlockId {
        BlockId::from_uuid(Uuid::from_u128(n))
    }

    fn under(n: u128) -> Parent {
        Parent::Block(id(n))
    }

    fn now() -> Timestamp {
        Timestamp::from_millis(42)
    }

    fn page() -> BlockKind {
        BlockKind::Page { icon: None }
    }

    fn at(n: u128, offset: usize) -> TextPoint {
        at_block(id(n), offset)
    }

    fn at_block(block: BlockId, offset: usize) -> TextPoint {
        TextPoint { block, offset }
    }

    fn block(n: u128, kind: BlockKind, parent: Parent, order: u32, text: &str) -> Block {
        Block {
            id: id(n),
            kind,
            parent,
            sort_key: SortKey::parse(&order.to_string()).unwrap(),
            text: RichText::plain(text),
            alive: true,
            created_at: now(),
        }
    }

    fn workspace(blocks: Vec<Block>) -> Workspace {
        Workspace::from_snapshot(WorkspaceSnapshot {
            blocks,
            ..WorkspaceSnapshot::default()
        })
        .unwrap()
    }

    fn notes(blocks: Vec<Block>) -> Workspace {
        let mut all = vec![block(1, page(), Parent::Root, 1, "Notes")];
        all.extend(blocks);
        workspace(all)
    }

    fn try_run(workspace: &mut Workspace, command: Command) -> Result<Built, DomainError> {
        let built = build(workspace, &command, now())?;
        workspace.apply(&built.transaction)?;
        Ok(built)
    }

    fn run(workspace: &mut Workspace, command: Command) -> Built {
        try_run(workspace, command).unwrap()
    }

    fn rejection(workspace: &mut Workspace, command: Command) -> DomainError {
        let before = workspace.to_snapshot();
        let error = try_run(workspace, command).unwrap_err();
        assert_eq!(workspace.to_snapshot(), before);
        error
    }

    fn texts(workspace: &Workspace, parent: Parent) -> Vec<String> {
        workspace
            .children(parent)
            .map(|block| block.text.to_plain())
            .collect()
    }

    fn ids(workspace: &Workspace, parent: Parent) -> Vec<BlockId> {
        workspace.children(parent).map(|block| block.id).collect()
    }

    fn created_block(built: &Built) -> BlockId {
        match built.created.first() {
            Some(Created::Block(created)) => *created,
            other => panic!("no created block: {other:?}"),
        }
    }

    fn kind_of(workspace: &Workspace, n: u128) -> BlockKind {
        workspace.block(id(n)).unwrap().kind.clone()
    }

    fn text_of(workspace: &Workspace, n: u128) -> String {
        workspace.block(id(n)).unwrap().text.to_plain()
    }

    fn alive(workspace: &Workspace, n: u128) -> bool {
        workspace.block(id(n)).unwrap().alive
    }

    fn bold() -> Marks {
        Marks {
            bold: true,
            ..Marks::default()
        }
    }

    fn callout() -> BlockKind {
        BlockKind::Callout {
            icon: Emoji::try_from("💡").unwrap(),
        }
    }

    #[test]
    fn create_page_without_anchor_goes_first() {
        let mut workspace = workspace(vec![block(1, page(), Parent::Root, 5, "Old")]);
        let built = run(
            &mut workspace,
            Command::CreatePage {
                parent: Parent::Root,
                after: None,
            },
        );
        let created = created_block(&built);
        assert_eq!(ids(&workspace, Parent::Root), [created, id(1)]);
        assert_eq!(
            built.caret,
            Some(TextPoint {
                block: created,
                offset: 0
            })
        );
        let page = workspace.block(created).unwrap();
        assert_eq!(page.kind, BlockKind::Page { icon: None });
        assert!(page.text.is_empty());
        assert_eq!(page.created_at, now());
    }

    #[test]
    fn create_page_after_a_sibling_lands_between_siblings() {
        let mut workspace = workspace(vec![
            block(1, page(), Parent::Root, 1, ""),
            block(2, page(), Parent::Root, 2, ""),
        ]);
        let built = run(
            &mut workspace,
            Command::CreatePage {
                parent: Parent::Root,
                after: Some(id(1)),
            },
        );
        assert_eq!(
            ids(&workspace, Parent::Root),
            [id(1), created_block(&built), id(2)]
        );
    }

    #[test]
    fn create_sub_page_inside_a_page() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "intro")]);
        let built = run(
            &mut workspace,
            Command::CreatePage {
                parent: under(1),
                after: Some(id(2)),
            },
        );
        assert_eq!(ids(&workspace, under(1)), [id(2), created_block(&built)]);
    }

    #[test]
    fn create_page_under_a_database_is_rejected() {
        let mut workspace = notes(Vec::new());
        let error = rejection(
            &mut workspace,
            Command::CreatePage {
                parent: Parent::Database(DatabaseId::new()),
                after: None,
            },
        );
        assert_eq!(error, DomainError::InvalidParent);
    }

    #[test]
    fn create_page_after_a_block_of_another_parent_is_rejected() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "intro")]);
        let error = rejection(
            &mut workspace,
            Command::CreatePage {
                parent: Parent::Root,
                after: Some(id(2)),
            },
        );
        assert_eq!(error, DomainError::InvalidParent);
    }

    #[test]
    fn insert_block_adds_an_empty_next_sibling() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        let built = run(
            &mut workspace,
            Command::InsertBlock {
                after: id(2),
                kind: BlockKind::Heading(HeadingLevel::Two),
            },
        );
        let created = created_block(&built);
        assert_eq!(ids(&workspace, under(1)), [id(2), created, id(3)]);
        assert_eq!(
            workspace.block(created).unwrap().kind,
            BlockKind::Heading(HeadingLevel::Two)
        );
        assert!(workspace.block(created).unwrap().text.is_empty());
        assert_eq!(
            built.caret,
            Some(TextPoint {
                block: created,
                offset: 0
            })
        );
    }

    #[test]
    fn insert_block_rejects_page_and_database_view_kinds() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "a")]);
        for kind in [
            page(),
            BlockKind::DatabaseView {
                database: DatabaseId::new(),
            },
        ] {
            let error = rejection(&mut workspace, Command::InsertBlock { after: id(2), kind });
            assert_eq!(error, DomainError::InvalidParent);
        }
    }

    #[test]
    fn insert_block_after_a_root_page_is_rejected() {
        let mut workspace = notes(Vec::new());
        let error = rejection(
            &mut workspace,
            Command::InsertBlock {
                after: id(1),
                kind: BlockKind::Text,
            },
        );
        assert_eq!(error, DomainError::InvalidParent);
    }

    #[test]
    fn insert_block_at_creates_the_first_child_of_a_sub_page() {
        let mut workspace = notes(vec![block(2, page(), under(1), 1, "Sub")]);
        let first = run(
            &mut workspace,
            Command::InsertBlockAt {
                target: DropTarget {
                    parent: under(2),
                    after: None,
                },
                kind: BlockKind::Text,
            },
        );
        let created = created_block(&first);
        assert_eq!(ids(&workspace, under(2)), [created]);
        assert_eq!(workspace.block(created).unwrap().kind, BlockKind::Text);
        assert!(workspace.block(created).unwrap().text.is_empty());
        assert_eq!(first.created, [Created::Block(created)]);
        assert_eq!(first.caret, Some(at_block(created, 0)));
        assert_eq!(ids(&workspace, under(1)), [id(2)]);
    }

    #[test]
    fn insert_block_at_without_anchor_goes_before_the_existing_children() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        let built = run(
            &mut workspace,
            Command::InsertBlockAt {
                target: DropTarget {
                    parent: under(1),
                    after: None,
                },
                kind: BlockKind::Heading(HeadingLevel::One),
            },
        );
        assert_eq!(
            ids(&workspace, under(1)),
            [created_block(&built), id(2), id(3)]
        );
    }

    #[test]
    fn insert_block_at_lands_right_after_the_anchor() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        let built = run(
            &mut workspace,
            Command::InsertBlockAt {
                target: DropTarget {
                    parent: under(1),
                    after: Some(id(2)),
                },
                kind: BlockKind::Quote,
            },
        );
        assert_eq!(
            ids(&workspace, under(1)),
            [id(2), created_block(&built), id(3)]
        );
    }

    #[test]
    fn insert_block_at_rejects_page_and_database_view_kinds() {
        let mut workspace = notes(Vec::new());
        for kind in [
            page(),
            BlockKind::DatabaseView {
                database: DatabaseId::new(),
            },
        ] {
            let error = rejection(
                &mut workspace,
                Command::InsertBlockAt {
                    target: DropTarget {
                        parent: under(1),
                        after: None,
                    },
                    kind,
                },
            );
            assert_eq!(error, DomainError::InvalidParent);
        }
    }

    #[test]
    fn insert_block_at_rejects_bad_parents_and_anchors() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Heading(HeadingLevel::One), under(1), 2, "h"),
            block(4, BlockKind::Text, under(2), 1, "kid"),
        ]);
        let attempts = [
            (Parent::Root, None, DomainError::InvalidParent),
            (under(99), None, DomainError::UnknownBlock(id(99))),
            (under(3), None, DomainError::ChildrenNotAllowed),
            (under(1), Some(id(4)), DomainError::InvalidParent),
            (under(1), Some(id(99)), DomainError::UnknownBlock(id(99))),
        ];
        for (parent, after, expected) in attempts {
            let error = rejection(
                &mut workspace,
                Command::InsertBlockAt {
                    target: DropTarget { parent, after },
                    kind: BlockKind::Text,
                },
            );
            assert_eq!(error, expected);
        }
    }

    #[test]
    fn insert_block_at_under_an_unknown_database_reports_unknown_database() {
        let mut workspace = notes(Vec::new());
        let missing = DatabaseId::new();
        let error = rejection(
            &mut workspace,
            Command::InsertBlockAt {
                target: DropTarget {
                    parent: Parent::Database(missing),
                    after: None,
                },
                kind: BlockKind::Text,
            },
        );
        assert_eq!(error, DomainError::UnknownDatabase(missing));
    }

    #[test]
    fn insert_block_after_a_sub_page_adds_a_sibling() {
        let mut workspace = notes(vec![block(2, page(), under(1), 1, "Sub")]);
        let built = run(
            &mut workspace,
            Command::InsertBlock {
                after: id(2),
                kind: BlockKind::Text,
            },
        );
        assert_eq!(ids(&workspace, under(1)), [id(2), created_block(&built)]);
    }

    #[test]
    fn insert_block_after_an_unknown_block_is_rejected() {
        let mut workspace = notes(Vec::new());
        let error = rejection(
            &mut workspace,
            Command::InsertBlock {
                after: id(99),
                kind: BlockKind::Text,
            },
        );
        assert_eq!(error, DomainError::UnknownBlock(id(99)));
    }

    #[test]
    fn edit_text_sets_the_text_without_a_caret() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "")]);
        let built = run(
            &mut workspace,
            Command::EditText {
                block: id(2),
                text: RichText::plain("Hello"),
            },
        );
        assert_eq!(text_of(&workspace, 2), "Hello");
        assert_eq!(built.caret, None);
        assert!(built.created.is_empty());
    }

    #[test]
    fn edit_text_with_the_same_text_is_an_empty_transaction() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "same")]);
        let built = build(
            &workspace,
            &Command::EditText {
                block: id(2),
                text: RichText::plain("same"),
            },
            now(),
        )
        .unwrap();
        assert!(built.transaction.is_empty());
        let error = rejection(
            &mut workspace,
            Command::EditText {
                block: id(99),
                text: RichText::plain("x"),
            },
        );
        assert_eq!(error, DomainError::UnknownBlock(id(99)));
    }

    #[test]
    fn spec_2_split_hello_world_at_offset_five() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "Hello world"),
            block(3, BlockKind::Text, under(1), 2, "tail"),
        ]);
        let built = run(&mut workspace, Command::SplitBlock { at: at(2, 5) });
        let created = created_block(&built);
        assert_eq!(texts(&workspace, under(1)), ["Hello", " world", "tail"]);
        assert_eq!(ids(&workspace, under(1)), [id(2), created, id(3)]);
        assert_eq!(
            built.caret,
            Some(TextPoint {
                block: created,
                offset: 0
            })
        );
    }

    #[test]
    fn split_keeps_the_children_with_the_original_block() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "abcd"),
            block(3, BlockKind::Text, under(2), 1, "child"),
        ]);
        let built = run(&mut workspace, Command::SplitBlock { at: at(2, 2) });
        assert_eq!(ids(&workspace, under(2)), [id(3)]);
        assert!(ids(&workspace, Parent::Block(created_block(&built))).is_empty());
    }

    #[test]
    fn split_continues_list_items_and_starts_unchecked_to_dos() {
        let cases = [
            (BlockKind::BulletedListItem, BlockKind::BulletedListItem),
            (BlockKind::NumberedListItem, BlockKind::NumberedListItem),
            (
                BlockKind::ToDo { checked: true },
                BlockKind::ToDo { checked: false },
            ),
        ];
        for (before, after) in cases {
            let mut workspace = notes(vec![block(2, before.clone(), under(1), 1, "abcd")]);
            let built = run(&mut workspace, Command::SplitBlock { at: at(2, 2) });
            assert_eq!(kind_of(&workspace, 2), before);
            assert_eq!(workspace.block(created_block(&built)).unwrap().kind, after);
        }
    }

    #[test]
    fn split_of_any_other_kind_starts_a_text_block() {
        let kinds = [
            BlockKind::Heading(HeadingLevel::One),
            BlockKind::Toggle,
            BlockKind::Quote,
            callout(),
            BlockKind::Code {
                language: CodeLanguage::plain_text(),
            },
            BlockKind::Text,
        ];
        for kind in kinds {
            let mut workspace = notes(vec![block(2, kind.clone(), under(1), 1, "abcd")]);
            let built = run(&mut workspace, Command::SplitBlock { at: at(2, 2) });
            assert_eq!(kind_of(&workspace, 2), kind);
            assert_eq!(
                workspace.block(created_block(&built)).unwrap().kind,
                BlockKind::Text
            );
        }
    }

    #[test]
    fn spec_3_enter_on_an_empty_bulleted_item_turns_it_into_text() {
        let kinds = [
            BlockKind::BulletedListItem,
            BlockKind::NumberedListItem,
            BlockKind::ToDo { checked: false },
            BlockKind::Quote,
            callout(),
        ];
        for kind in kinds {
            let mut workspace = notes(vec![block(2, kind, under(1), 1, "")]);
            let built = run(&mut workspace, Command::SplitBlock { at: at(2, 0) });
            assert_eq!(kind_of(&workspace, 2), BlockKind::Text);
            assert_eq!(ids(&workspace, under(1)), [id(2)]);
            assert!(built.created.is_empty());
            assert_eq!(built.caret, Some(at(2, 0)));
        }
    }

    #[test]
    fn split_of_an_empty_text_block_adds_an_empty_text_block() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "")]);
        let built = run(&mut workspace, Command::SplitBlock { at: at(2, 0) });
        assert_eq!(ids(&workspace, under(1)), [id(2), created_block(&built)]);
        assert_eq!(kind_of(&workspace, 2), BlockKind::Text);
    }

    #[test]
    fn split_at_the_edges_moves_all_or_none_of_the_text() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "abc")]);
        run(&mut workspace, Command::SplitBlock { at: at(2, 3) });
        assert_eq!(texts(&workspace, under(1)), ["abc", ""]);
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "abc")]);
        run(&mut workspace, Command::SplitBlock { at: at(2, 0) });
        assert_eq!(texts(&workspace, under(1)), ["", "abc"]);
    }

    #[test]
    fn split_keeps_marks_on_both_halves() {
        let text = RichText::new([
            Span {
                text: "ab".to_owned(),
                marks: Marks::default(),
            },
            Span {
                text: "cd".to_owned(),
                marks: bold(),
            },
        ]);
        let mut block = block(2, BlockKind::Text, under(1), 1, "");
        block.text = text;
        let mut workspace = notes(vec![block]);
        let built = run(&mut workspace, Command::SplitBlock { at: at(2, 3) });
        let tail = &workspace.block(created_block(&built)).unwrap().text;
        assert_eq!(tail.spans().len(), 1);
        assert_eq!(tail.spans()[0].marks, bold());
        assert_eq!(
            workspace.block(id(2)).unwrap().text.spans()[1].marks,
            bold()
        );
    }

    #[test]
    fn split_on_a_divider_or_database_view_inserts_an_empty_text_block_after_it() {
        let database = Database {
            id: DatabaseId::new(),
            name: "Tasks".to_owned(),
            icon: None,
            schema: Schema {
                properties: vec![Property {
                    id: PropertyId::new(),
                    name: "Name".to_owned(),
                    kind: PropertyKind::Title,
                }],
            },
            sort_key: SortKey::parse("V").unwrap(),
            alive: true,
            created_at: now(),
        };
        let kinds = [
            BlockKind::Divider,
            BlockKind::DatabaseView {
                database: database.id,
            },
        ];
        for kind in kinds {
            let mut workspace = Workspace::from_snapshot(WorkspaceSnapshot {
                blocks: vec![
                    block(1, page(), Parent::Root, 1, "Notes"),
                    block(2, kind, under(1), 1, ""),
                    block(3, BlockKind::Text, under(1), 2, "tail"),
                ],
                databases: vec![database.clone()],
                ..WorkspaceSnapshot::default()
            })
            .unwrap();
            let built = run(&mut workspace, Command::SplitBlock { at: at(2, 0) });
            let created = created_block(&built);
            assert_eq!(ids(&workspace, under(1)), [id(2), created, id(3)]);
            assert_eq!(workspace.block(created).unwrap().kind, BlockKind::Text);
            assert!(workspace.block(created).unwrap().text.is_empty());
            assert_eq!(built.caret, Some(at_block(created, 0)));
        }
    }

    #[test]
    fn split_page_title_moves_the_rest_into_a_first_child() {
        let layouts = [
            vec![
                block(1, page(), Parent::Root, 1, "Hello world"),
                block(2, BlockKind::Text, under(1), 1, "old"),
            ],
            vec![
                block(9, page(), Parent::Root, 1, "Outer"),
                block(1, page(), under(9), 1, "Hello world"),
                block(2, BlockKind::Text, under(1), 1, "old"),
            ],
        ];
        for blocks in layouts {
            let mut workspace = workspace(blocks);
            let built = run(&mut workspace, Command::SplitBlock { at: at(1, 5) });
            let created = created_block(&built);
            assert_eq!(text_of(&workspace, 1), "Hello");
            assert_eq!(kind_of(&workspace, 1), page());
            assert_eq!(ids(&workspace, under(1)), [created, id(2)]);
            let first = workspace.block(created).unwrap();
            assert_eq!(first.kind, BlockKind::Text);
            assert_eq!(first.text.to_plain(), " world");
            assert_eq!(first.parent, under(1));
            assert_eq!(built.caret, Some(at_block(created, 0)));
            assert_eq!(built.created, [Created::Block(created)]);
        }
    }

    #[test]
    fn split_page_title_at_the_end_adds_an_empty_first_child_and_keeps_the_title() {
        let mut workspace = notes(Vec::new());
        let built = run(&mut workspace, Command::SplitBlock { at: at(1, 5) });
        assert_eq!(text_of(&workspace, 1), "Notes");
        assert_eq!(texts(&workspace, under(1)), [""]);
        assert_eq!(ids(&workspace, under(1)), [created_block(&built)]);
    }

    #[test]
    fn split_page_title_at_an_invalid_offset_changes_nothing() {
        let mut workspace = workspace(vec![block(1, page(), Parent::Root, 1, "é")]);
        let error = rejection(&mut workspace, Command::SplitBlock { at: at(1, 1) });
        assert_eq!(error, DomainError::InvalidOffset(1));
    }

    #[test]
    fn split_at_an_invalid_offset_changes_nothing() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "é")]);
        let error = rejection(&mut workspace, Command::SplitBlock { at: at(2, 1) });
        assert_eq!(error, DomainError::InvalidOffset(1));
        let error = rejection(&mut workspace, Command::SplitBlock { at: at(2, 9) });
        assert_eq!(error, DomainError::InvalidOffset(9));
    }

    #[test]
    fn spec_4_backspace_at_offset_zero_merges_into_the_previous_block() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "ab"),
            block(3, BlockKind::Text, under(1), 2, "cd"),
        ]);
        let built = run(&mut workspace, Command::MergeWithPrevious { block: id(3) });
        assert_eq!(texts(&workspace, under(1)), ["abcd"]);
        assert!(!alive(&workspace, 3));
        assert_eq!(built.caret, Some(at(2, 2)));
    }

    #[test]
    fn merge_keeps_the_marks_of_both_sides() {
        let mut first = block(2, BlockKind::Text, under(1), 1, "");
        first.text = RichText::new([Span {
            text: "ab".to_owned(),
            marks: bold(),
        }]);
        let mut workspace = notes(vec![first, block(3, BlockKind::Text, under(1), 2, "cd")]);
        run(&mut workspace, Command::MergeWithPrevious { block: id(3) });
        let spans = workspace.block(id(2)).unwrap().text.spans().to_vec();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].marks, bold());
        assert_eq!(spans[1].marks, Marks::default());
    }

    #[test]
    fn merge_moves_the_children_to_the_position_of_the_merged_block() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(1), 2, "b"),
            block(4, BlockKind::Text, under(3), 1, "x"),
            block(5, BlockKind::Text, under(3), 2, "y"),
            block(6, BlockKind::Text, under(1), 3, "z"),
        ]);
        run(&mut workspace, Command::MergeWithPrevious { block: id(3) });
        assert_eq!(text_of(&workspace, 2), "ab");
        assert_eq!(ids(&workspace, under(1)), [id(2), id(4), id(5), id(6)]);
    }

    #[test]
    fn merge_into_the_parent_when_the_block_is_its_first_child() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(2), 1, "b"),
        ]);
        let built = run(&mut workspace, Command::MergeWithPrevious { block: id(3) });
        assert_eq!(text_of(&workspace, 2), "ab");
        assert!(ids(&workspace, under(2)).is_empty());
        assert_eq!(built.caret, Some(at(2, 1)));
    }

    #[test]
    fn merge_targets_the_last_descendant_of_the_previous_sibling() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(4, BlockKind::Text, under(2), 1, "x"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        run(&mut workspace, Command::MergeWithPrevious { block: id(3) });
        assert_eq!(text_of(&workspace, 4), "xb");
        assert_eq!(text_of(&workspace, 2), "a");
    }

    #[test]
    fn merge_with_an_atomic_previous_block_removes_it() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Divider, under(1), 1, ""),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        let built = run(&mut workspace, Command::MergeWithPrevious { block: id(3) });
        assert!(!alive(&workspace, 2));
        assert_eq!(text_of(&workspace, 3), "b");
        assert_eq!(built.caret, Some(at(3, 0)));
    }

    #[test]
    fn merge_does_nothing_after_a_page_or_at_the_start_of_a_page() {
        let mut workspace = notes(vec![
            block(2, page(), under(1), 1, "Sub"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        for block in [3, 2] {
            let built = build(
                &workspace,
                &Command::MergeWithPrevious { block: id(block) },
                now(),
            )
            .unwrap();
            assert!(built.transaction.is_empty());
        }
        assert_eq!(texts(&workspace, under(1)), ["Sub", "b"]);
        let error = rejection(&mut workspace, Command::MergeWithPrevious { block: id(99) });
        assert_eq!(error, DomainError::UnknownBlock(id(99)));
    }

    #[test]
    fn delete_range_within_one_block_removes_the_text() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "Hello world")]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 5),
                end: at(2, 11),
            },
        );
        assert_eq!(text_of(&workspace, 2), "Hello");
        assert_eq!(built.caret, Some(at(2, 5)));
    }

    #[test]
    fn delete_range_with_reversed_points_in_one_block_is_normalised() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "abcd")]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 3),
                end: at(2, 1),
            },
        );
        assert_eq!(text_of(&workspace, 2), "ad");
        assert_eq!(built.caret, Some(at(2, 1)));
    }

    #[test]
    fn delete_range_of_an_empty_selection_changes_nothing() {
        let workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "abcd")]);
        let built = build(
            &workspace,
            &Command::DeleteRange {
                start: at(2, 2),
                end: at(2, 2),
            },
            now(),
        )
        .unwrap();
        assert!(built.transaction.is_empty());
        assert_eq!(built.caret, Some(at(2, 2)));
    }

    #[test]
    fn spec_5_delete_across_two_blocks_leaves_held() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "Hello"),
            block(3, BlockKind::Text, under(1), 2, "world"),
        ]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 2),
                end: at(3, 3),
            },
        );
        assert_eq!(texts(&workspace, under(1)), ["Held"]);
        assert_eq!(built.caret, Some(at(2, 2)));
    }

    #[test]
    fn delete_range_with_reversed_points_across_blocks_is_normalised() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "Hello"),
            block(3, BlockKind::Text, under(1), 2, "world"),
        ]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(3, 3),
                end: at(2, 2),
            },
        );
        assert_eq!(texts(&workspace, under(1)), ["Held"]);
        assert_eq!(built.caret, Some(at(2, 2)));
    }

    #[test]
    fn delete_range_removes_every_block_in_between() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "ab"),
            block(3, BlockKind::Text, under(1), 2, "mid"),
            block(4, BlockKind::Text, under(3), 1, "inner"),
            block(5, BlockKind::Text, under(1), 3, "cd"),
        ]);
        run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 1),
                end: at(5, 1),
            },
        );
        assert_eq!(texts(&workspace, under(1)), ["ad"]);
        assert!(!alive(&workspace, 3));
        assert!(!alive(&workspace, 4));
        assert!(!alive(&workspace, 5));
    }

    #[test]
    fn delete_range_moves_the_children_of_the_last_block_after_the_first_block() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "ab"),
            block(3, BlockKind::Text, under(1), 2, "cd"),
            block(4, BlockKind::Text, under(3), 1, "x"),
            block(5, BlockKind::Text, under(3), 2, "y"),
            block(6, BlockKind::Text, under(1), 3, "tail"),
        ]);
        run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 1),
                end: at(3, 1),
            },
        );
        assert_eq!(text_of(&workspace, 2), "ad");
        assert_eq!(ids(&workspace, under(1)), [id(2), id(4), id(5), id(6)]);
    }

    #[test]
    fn delete_range_from_a_parent_into_its_child_keeps_the_grandchildren() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "ab"),
            block(3, BlockKind::Text, under(2), 1, "cd"),
            block(4, BlockKind::Text, under(3), 1, "ef"),
        ]);
        run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 1),
                end: at(3, 1),
            },
        );
        assert_eq!(text_of(&workspace, 2), "ad");
        assert_eq!(ids(&workspace, under(1)), [id(2), id(4)]);
    }

    #[test]
    fn delete_range_rejects_other_pages_and_bad_offsets() {
        let mut workspace = workspace(vec![
            block(1, page(), Parent::Root, 1, "A"),
            block(2, BlockKind::Text, under(1), 1, "ab"),
            block(3, page(), Parent::Root, 2, "B"),
            block(4, BlockKind::Text, under(3), 1, "cd"),
        ]);
        let error = rejection(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 0),
                end: at(4, 1),
            },
        );
        assert_eq!(error, DomainError::InvalidParent);
        let error = rejection(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 0),
                end: at(2, 9),
            },
        );
        assert_eq!(error, DomainError::InvalidOffset(9));
    }

    #[test]
    fn set_marks_in_one_block_splits_the_spans() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "Hello world")]);
        let built = run(
            &mut workspace,
            Command::SetMarks {
                start: at(2, 6),
                end: at(2, 11),
                change: MarkChange::Bold(true),
            },
        );
        let spans = workspace.block(id(2)).unwrap().text.spans().to_vec();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].text, "world");
        assert_eq!(spans[1].marks, bold());
        assert_eq!(built.caret, None);
    }

    #[test]
    fn set_marks_across_blocks_covers_the_tail_the_middle_and_the_head() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "abcd"),
            block(3, BlockKind::Text, under(1), 2, "mid"),
            block(4, BlockKind::Text, under(1), 3, "efgh"),
        ]);
        run(
            &mut workspace,
            Command::SetMarks {
                start: at(2, 2),
                end: at(4, 2),
                change: MarkChange::Bold(true),
            },
        );
        let bold_text = |n: u128| -> String {
            workspace
                .block(id(n))
                .unwrap()
                .text
                .spans()
                .iter()
                .filter(|span| span.marks.bold)
                .map(|span| span.text.as_str())
                .collect()
        };
        assert_eq!(bold_text(2), "cd");
        assert_eq!(bold_text(3), "mid");
        assert_eq!(bold_text(4), "ef");
    }

    #[test]
    fn set_marks_skips_atomic_blocks_pages_and_unchanged_text() {
        let workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "ab"),
            block(3, BlockKind::Divider, under(1), 2, ""),
            block(4, page(), under(1), 3, "Sub"),
            block(5, BlockKind::Text, under(1), 4, ""),
            block(6, BlockKind::Text, under(1), 5, "cd"),
        ]);
        let built = build(
            &workspace,
            &Command::SetMarks {
                start: at(2, 0),
                end: at(6, 2),
                change: MarkChange::Bold(true),
            },
            now(),
        )
        .unwrap();
        let touched: Vec<BlockId> = built
            .transaction
            .operations()
            .iter()
            .map(|operation| match operation {
                Operation::SetText { block, .. } => *block,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(touched, [id(2), id(6)]);
    }

    #[test]
    fn set_marks_with_reversed_points_and_bad_offsets() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "abcd")]);
        run(
            &mut workspace,
            Command::SetMarks {
                start: at(2, 3),
                end: at(2, 1),
                change: MarkChange::Italic(true),
            },
        );
        let italic: String = workspace
            .block(id(2))
            .unwrap()
            .text
            .spans()
            .iter()
            .filter(|span| span.marks.italic)
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(italic, "bc");
        let error = rejection(
            &mut workspace,
            Command::SetMarks {
                start: at(2, 0),
                end: at(2, 9),
                change: MarkChange::Italic(true),
            },
        );
        assert_eq!(error, DomainError::InvalidOffset(9));
    }

    #[test]
    fn spec_8_indent_nests_under_the_previous_sibling_and_outdent_returns_after_it() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(1), 2, "b"),
            block(4, BlockKind::Text, under(1), 3, "c"),
        ]);
        run(&mut workspace, Command::Indent { block: id(3) });
        assert_eq!(ids(&workspace, under(1)), [id(2), id(4)]);
        assert_eq!(ids(&workspace, under(2)), [id(3)]);
        run(&mut workspace, Command::Outdent { block: id(3) });
        assert_eq!(ids(&workspace, under(1)), [id(2), id(3), id(4)]);
        assert!(ids(&workspace, under(2)).is_empty());
    }

    #[test]
    fn indent_appends_after_the_last_child_of_the_previous_sibling() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(5, BlockKind::Text, under(2), 1, "kid"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        run(&mut workspace, Command::Indent { block: id(3) });
        assert_eq!(ids(&workspace, under(2)), [id(5), id(3)]);
    }

    #[test]
    fn spec_9_indent_on_a_first_child_is_rejected() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        let error = rejection(&mut workspace, Command::Indent { block: id(2) });
        assert_eq!(error, DomainError::InvalidParent);
    }

    #[test]
    fn spec_9_indent_under_a_heading_is_rejected() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Heading(HeadingLevel::One), under(1), 1, "h"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        let error = rejection(&mut workspace, Command::Indent { block: id(3) });
        assert_eq!(error, DomainError::ChildrenNotAllowed);
    }

    #[test]
    fn indent_and_outdent_reject_top_level_pages_and_page_children() {
        let mut workspace = workspace(vec![
            block(1, page(), Parent::Root, 1, "A"),
            block(2, page(), Parent::Root, 2, "B"),
            block(3, BlockKind::Text, under(2), 1, "x"),
        ]);
        let error = rejection(&mut workspace, Command::Indent { block: id(2) });
        assert_eq!(error, DomainError::InvalidParent);
        let error = rejection(&mut workspace, Command::Outdent { block: id(2) });
        assert_eq!(error, DomainError::InvalidParent);
        let error = rejection(&mut workspace, Command::Outdent { block: id(3) });
        assert_eq!(error, DomainError::InvalidParent);
    }

    #[test]
    fn outdent_leaves_the_following_siblings_under_the_old_parent() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(2), 1, "b"),
            block(4, BlockKind::Text, under(2), 2, "c"),
            block(5, BlockKind::Text, under(2), 3, "d"),
            block(6, BlockKind::Text, under(1), 2, "e"),
        ]);
        run(&mut workspace, Command::Outdent { block: id(4) });
        assert_eq!(ids(&workspace, under(1)), [id(2), id(4), id(6)]);
        assert_eq!(ids(&workspace, under(2)), [id(3), id(5)]);
    }

    #[test]
    fn change_kind_sets_the_new_kind_and_keeps_the_text() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "keep")]);
        let built = run(
            &mut workspace,
            Command::ChangeKind {
                block: id(2),
                kind: BlockKind::Heading(HeadingLevel::Three),
            },
        );
        assert_eq!(
            kind_of(&workspace, 2),
            BlockKind::Heading(HeadingLevel::Three)
        );
        assert_eq!(text_of(&workspace, 2), "keep");
        assert_eq!(built.caret, None);
    }

    #[test]
    fn change_kind_toggles_a_to_do() {
        let mut workspace = notes(vec![block(
            2,
            BlockKind::ToDo { checked: false },
            under(1),
            1,
            "task",
        )]);
        run(
            &mut workspace,
            Command::ChangeKind {
                block: id(2),
                kind: BlockKind::ToDo { checked: true },
            },
        );
        assert_eq!(kind_of(&workspace, 2), BlockKind::ToDo { checked: true });
    }

    #[test]
    fn change_kind_to_divider_clears_the_text_first() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "gone")]);
        let built = build(
            &workspace,
            &Command::ChangeKind {
                block: id(2),
                kind: BlockKind::Divider,
            },
            now(),
        )
        .unwrap();
        assert!(matches!(
            built.transaction.operations(),
            [Operation::SetText { .. }, Operation::SetKind { .. }]
        ));
        workspace.apply(&built.transaction).unwrap();
        assert_eq!(kind_of(&workspace, 2), BlockKind::Divider);
        assert!(workspace.block(id(2)).unwrap().text.is_empty());
    }

    #[test]
    fn change_kind_rejects_pages_database_views_and_kinds_that_refuse_children() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(2), 1, "kid"),
            block(4, page(), under(1), 2, "Sub"),
        ]);
        for (block, kind) in [
            (2, page()),
            (
                2,
                BlockKind::DatabaseView {
                    database: DatabaseId::new(),
                },
            ),
            (4, BlockKind::Text),
        ] {
            let error = rejection(
                &mut workspace,
                Command::ChangeKind {
                    block: id(block),
                    kind,
                },
            );
            assert_eq!(error, DomainError::InvalidParent);
        }
        let error = rejection(
            &mut workspace,
            Command::ChangeKind {
                block: id(2),
                kind: BlockKind::Divider,
            },
        );
        assert_eq!(error, DomainError::ChildrenNotAllowed);
    }

    #[test]
    fn change_kind_to_the_same_kind_is_an_empty_transaction() {
        let workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "a")]);
        let built = build(
            &workspace,
            &Command::ChangeKind {
                block: id(2),
                kind: BlockKind::Text,
            },
            now(),
        )
        .unwrap();
        assert!(built.transaction.is_empty());
    }

    #[test]
    fn move_block_lands_right_after_the_target_sibling() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(1), 2, "b"),
            block(4, BlockKind::Text, under(1), 3, "c"),
        ]);
        run(
            &mut workspace,
            Command::MoveBlock {
                block: id(2),
                target: DropTarget {
                    parent: under(1),
                    after: Some(id(3)),
                },
            },
        );
        assert_eq!(ids(&workspace, under(1)), [id(3), id(2), id(4)]);
        run(
            &mut workspace,
            Command::MoveBlock {
                block: id(4),
                target: DropTarget {
                    parent: under(1),
                    after: None,
                },
            },
        );
        assert_eq!(ids(&workspace, under(1)), [id(4), id(3), id(2)]);
    }

    #[test]
    fn spec_11_moving_a_block_with_two_children_keeps_the_children_in_order() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(4, BlockKind::Text, under(2), 1, "x"),
            block(5, BlockKind::Text, under(2), 2, "y"),
            block(3, BlockKind::Text, under(1), 2, "b"),
        ]);
        run(
            &mut workspace,
            Command::MoveBlock {
                block: id(2),
                target: DropTarget {
                    parent: under(3),
                    after: None,
                },
            },
        );
        assert_eq!(ids(&workspace, under(1)), [id(3)]);
        assert_eq!(ids(&workspace, under(3)), [id(2)]);
        assert_eq!(ids(&workspace, under(2)), [id(4), id(5)]);
    }

    #[test]
    fn spec_12_moving_a_block_under_a_descendant_is_rejected() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(2), 1, "b"),
            block(4, BlockKind::Text, under(3), 1, "c"),
        ]);
        let error = rejection(
            &mut workspace,
            Command::MoveBlock {
                block: id(2),
                target: DropTarget {
                    parent: under(4),
                    after: None,
                },
            },
        );
        assert_eq!(error, DomainError::CycleDetected);
        let error = rejection(
            &mut workspace,
            Command::MoveBlock {
                block: id(2),
                target: DropTarget {
                    parent: under(2),
                    after: None,
                },
            },
        );
        assert_eq!(error, DomainError::CycleDetected);
    }

    #[test]
    fn move_block_rejects_anchors_of_another_parent_and_parents_that_refuse_children() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Heading(HeadingLevel::One), under(1), 2, "h"),
            block(4, BlockKind::Text, under(2), 1, "kid"),
        ]);
        let error = rejection(
            &mut workspace,
            Command::MoveBlock {
                block: id(3),
                target: DropTarget {
                    parent: under(1),
                    after: Some(id(4)),
                },
            },
        );
        assert_eq!(error, DomainError::InvalidParent);
        let error = rejection(
            &mut workspace,
            Command::MoveBlock {
                block: id(2),
                target: DropTarget {
                    parent: under(3),
                    after: None,
                },
            },
        );
        assert_eq!(error, DomainError::ChildrenNotAllowed);
    }

    #[test]
    fn move_block_after_itself_is_an_empty_transaction() {
        let workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "a")]);
        let built = build(
            &workspace,
            &Command::MoveBlock {
                block: id(2),
                target: DropTarget {
                    parent: under(1),
                    after: Some(id(2)),
                },
            },
            now(),
        )
        .unwrap();
        assert!(built.transaction.is_empty());
    }

    #[test]
    fn move_a_page_between_top_level_pages() {
        let mut workspace = workspace(vec![
            block(1, page(), Parent::Root, 1, "A"),
            block(2, page(), Parent::Root, 2, "B"),
            block(3, page(), under(1), 1, "Sub"),
        ]);
        run(
            &mut workspace,
            Command::MoveBlock {
                block: id(3),
                target: DropTarget {
                    parent: Parent::Root,
                    after: Some(id(1)),
                },
            },
        );
        assert_eq!(ids(&workspace, Parent::Root), [id(1), id(3), id(2)]);
    }

    #[test]
    fn spec_14_deleting_a_page_hides_its_subtree_and_undo_restores_it() {
        let mut workspace = workspace(vec![
            block(1, page(), Parent::Root, 1, "Keep"),
            block(2, page(), Parent::Root, 2, "Gone"),
            block(3, BlockKind::Text, under(2), 1, "a"),
            block(4, BlockKind::Text, under(3), 1, "b"),
            block(5, page(), under(2), 2, "Sub"),
        ]);
        let before = workspace.to_snapshot();
        let built = build(
            &workspace,
            &Command::DeleteBlocks {
                blocks: vec![id(2)],
            },
            now(),
        )
        .unwrap();
        let inverse = workspace.apply(&built.transaction).unwrap();
        assert_eq!(ids(&workspace, Parent::Root), [id(1)]);
        for n in 2..=5 {
            assert!(!alive(&workspace, n));
        }
        workspace.apply(&inverse).unwrap();
        assert_eq!(workspace.to_snapshot(), before);
        assert_eq!(ids(&workspace, Parent::Root), [id(1), id(2)]);
    }

    #[test]
    fn delete_blocks_skips_dead_blocks_and_visits_each_block_once() {
        let mut dead = block(4, BlockKind::Text, under(1), 3, "dead");
        dead.alive = false;
        let workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(2), 1, "b"),
            dead,
        ]);
        let built = build(
            &workspace,
            &Command::DeleteBlocks {
                blocks: vec![id(3), id(2), id(4), id(3)],
            },
            now(),
        )
        .unwrap();
        let killed: Vec<BlockId> = built
            .transaction
            .operations()
            .iter()
            .map(|operation| match operation {
                Operation::SetBlockAlive {
                    block,
                    alive: false,
                } => *block,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(killed, [id(3), id(2)]);
    }

    #[test]
    fn delete_blocks_kills_parents_before_children() {
        let workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, BlockKind::Text, under(2), 1, "b"),
            block(4, BlockKind::Text, under(3), 1, "c"),
        ]);
        let built = build(
            &workspace,
            &Command::DeleteBlocks {
                blocks: vec![id(2)],
            },
            now(),
        )
        .unwrap();
        let killed: Vec<&Operation> = built.transaction.operations().iter().collect();
        assert_eq!(
            killed,
            [
                &Operation::SetBlockAlive {
                    block: id(2),
                    alive: false
                },
                &Operation::SetBlockAlive {
                    block: id(3),
                    alive: false
                },
                &Operation::SetBlockAlive {
                    block: id(4),
                    alive: false
                },
            ]
        );
    }

    #[test]
    fn delete_blocks_rejects_unknown_blocks() {
        let mut workspace = notes(Vec::new());
        let error = rejection(
            &mut workspace,
            Command::DeleteBlocks {
                blocks: vec![id(99)],
            },
        );
        assert_eq!(error, DomainError::UnknownBlock(id(99)));
    }

    #[test]
    fn set_page_icon_sets_and_clears_the_icon() {
        let mut workspace = notes(Vec::new());
        let icon = Emoji::try_from("📄").unwrap();
        run(
            &mut workspace,
            Command::SetPageIcon {
                page: id(1),
                icon: Some(icon.clone()),
            },
        );
        assert_eq!(kind_of(&workspace, 1), BlockKind::Page { icon: Some(icon) });
        run(
            &mut workspace,
            Command::SetPageIcon {
                page: id(1),
                icon: None,
            },
        );
        assert_eq!(kind_of(&workspace, 1), page());
    }

    #[test]
    fn set_page_icon_rejects_non_pages_and_ignores_unchanged_icons() {
        let mut workspace = notes(vec![block(2, BlockKind::Text, under(1), 1, "a")]);
        let error = rejection(
            &mut workspace,
            Command::SetPageIcon {
                page: id(2),
                icon: None,
            },
        );
        assert_eq!(error, DomainError::InvalidParent);
        let built = build(
            &workspace,
            &Command::SetPageIcon {
                page: id(1),
                icon: None,
            },
            now(),
        )
        .unwrap();
        assert!(built.transaction.is_empty());
    }

    #[test]
    fn database_commands_are_not_built_here() {
        let workspace = notes(Vec::new());
        assert!(build(&workspace, &Command::CreateDatabase, now()).is_err());
    }

    fn dead(mut block: Block) -> Block {
        block.alive = false;
        block
    }

    #[test]
    fn commands_naming_a_dead_block_are_unknown_block() {
        let workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "abcd"),
            dead(block(3, BlockKind::Text, under(1), 2, "gone")),
            dead(block(5, page(), under(1), 3, "Old")),
        ]);
        let bare = DropTarget {
            parent: under(1),
            after: None,
        };
        let commands = [
            (
                Command::EditText {
                    block: id(3),
                    text: RichText::plain("x"),
                },
                3,
            ),
            (Command::SplitBlock { at: at(3, 0) }, 3),
            (Command::MergeWithPrevious { block: id(3) }, 3),
            (
                Command::DeleteRange {
                    start: at(3, 0),
                    end: at(2, 1),
                },
                3,
            ),
            (
                Command::DeleteRange {
                    start: at(2, 0),
                    end: at(3, 1),
                },
                3,
            ),
            (
                Command::SetMarks {
                    start: at(3, 0),
                    end: at(2, 1),
                    change: MarkChange::Bold(true),
                },
                3,
            ),
            (
                Command::SetMarks {
                    start: at(2, 0),
                    end: at(3, 1),
                    change: MarkChange::Bold(true),
                },
                3,
            ),
            (
                Command::ChangeKind {
                    block: id(3),
                    kind: BlockKind::Quote,
                },
                3,
            ),
            (
                Command::MoveBlock {
                    block: id(3),
                    target: bare,
                },
                3,
            ),
            (
                Command::MoveBlock {
                    block: id(2),
                    target: DropTarget {
                        parent: under(1),
                        after: Some(id(3)),
                    },
                },
                3,
            ),
            (
                Command::MoveBlock {
                    block: id(2),
                    target: DropTarget {
                        parent: under(3),
                        after: None,
                    },
                },
                3,
            ),
            (
                Command::InsertBlock {
                    after: id(3),
                    kind: BlockKind::Text,
                },
                3,
            ),
            (
                Command::InsertBlockAt {
                    target: DropTarget {
                        parent: under(3),
                        after: None,
                    },
                    kind: BlockKind::Text,
                },
                3,
            ),
            (
                Command::InsertBlockAt {
                    target: DropTarget {
                        parent: under(1),
                        after: Some(id(3)),
                    },
                    kind: BlockKind::Text,
                },
                3,
            ),
            (
                Command::CreatePage {
                    parent: under(3),
                    after: None,
                },
                3,
            ),
            (
                Command::CreatePage {
                    parent: under(1),
                    after: Some(id(3)),
                },
                3,
            ),
            (
                Command::SetPageIcon {
                    page: id(5),
                    icon: None,
                },
                5,
            ),
            (Command::Indent { block: id(3) }, 3),
            (Command::Outdent { block: id(3) }, 3),
        ];
        for (command, dead_block) in commands {
            let result = build(&workspace, &command, now());
            assert_eq!(
                result.map(|_| ()),
                Err(DomainError::UnknownBlock(id(dead_block))),
                "{command:?}"
            );
        }
    }

    #[test]
    fn split_block_on_a_dead_block_creates_nothing() {
        let mut workspace = notes(vec![dead(block(2, BlockKind::Text, under(1), 1, "gone"))]);
        let error = rejection(&mut workspace, Command::SplitBlock { at: at(2, 0) });
        assert_eq!(error, DomainError::UnknownBlock(id(2)));
    }

    #[test]
    fn insert_block_after_a_dead_block_creates_nothing() {
        let mut workspace = notes(vec![dead(block(2, BlockKind::Text, under(1), 1, "gone"))]);
        let error = rejection(
            &mut workspace,
            Command::InsertBlock {
                after: id(2),
                kind: BlockKind::Text,
            },
        );
        assert_eq!(error, DomainError::UnknownBlock(id(2)));
    }

    #[test]
    fn delete_blocks_of_dead_blocks_is_an_empty_transaction() {
        let workspace = notes(vec![dead(block(2, BlockKind::Text, under(1), 1, "gone"))]);
        let built = build(
            &workspace,
            &Command::DeleteBlocks {
                blocks: vec![id(2)],
            },
            now(),
        )
        .unwrap();
        assert!(built.transaction.is_empty());
    }

    #[test]
    fn edit_text_on_an_atomic_block_is_text_not_allowed() {
        let mut workspace = notes(vec![block(2, BlockKind::Divider, under(1), 1, "")]);
        let error = rejection(
            &mut workspace,
            Command::EditText {
                block: id(2),
                text: RichText::plain("x"),
            },
        );
        assert_eq!(error, DomainError::TextNotAllowed);
        let built = build(
            &workspace,
            &Command::EditText {
                block: id(2),
                text: RichText::default(),
            },
            now(),
        )
        .unwrap();
        assert!(built.transaction.is_empty());
    }

    #[test]
    fn delete_range_starting_on_a_divider_keeps_the_text_block_and_removes_the_divider() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Divider, under(1), 1, ""),
            block(3, BlockKind::Text, under(1), 2, "hello"),
        ]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 0),
                end: at(3, 2),
            },
        );
        assert!(!alive(&workspace, 2));
        assert!(alive(&workspace, 3));
        assert_eq!(text_of(&workspace, 3), "llo");
        assert_eq!(built.caret, Some(at(3, 0)));
        assert_eq!(ids(&workspace, under(1)), [id(3)]);
    }

    #[test]
    fn delete_range_ending_on_a_divider_keeps_the_head_and_removes_the_divider() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "hello"),
            block(3, BlockKind::Divider, under(1), 2, ""),
        ]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 2),
                end: at(3, 0),
            },
        );
        assert_eq!(text_of(&workspace, 2), "he");
        assert!(!alive(&workspace, 3));
        assert_eq!(built.caret, Some(at(2, 2)));
    }

    #[test]
    fn delete_range_between_two_dividers_removes_everything_and_has_no_caret() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Divider, under(1), 1, ""),
            block(3, BlockKind::Text, under(1), 2, "mid"),
            block(4, BlockKind::Divider, under(1), 3, ""),
            block(5, BlockKind::Text, under(1), 4, "tail"),
        ]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 0),
                end: at(4, 0),
            },
        );
        assert_eq!(ids(&workspace, under(1)), [id(5)]);
        assert_eq!(text_of(&workspace, 5), "tail");
        assert_eq!(built.caret, None);
    }

    #[test]
    fn delete_range_from_a_divider_relocates_the_surviving_block_and_its_followers() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Divider, under(1), 1, ""),
            block(3, BlockKind::Text, under(1), 2, "b"),
            block(4, BlockKind::Text, under(3), 1, "x"),
            block(5, BlockKind::Text, under(3), 2, "tail"),
            block(6, BlockKind::Text, under(3), 3, "y"),
            block(7, BlockKind::Text, under(1), 3, "z"),
        ]);
        let built = run(
            &mut workspace,
            Command::DeleteRange {
                start: at(2, 0),
                end: at(5, 2),
            },
        );
        assert_eq!(ids(&workspace, under(1)), [id(5), id(6), id(7)]);
        assert_eq!(text_of(&workspace, 5), "il");
        assert_eq!(built.caret, Some(at(5, 0)));
        for removed in [2, 3, 4] {
            assert!(!alive(&workspace, removed));
        }
    }

    #[test]
    fn delete_range_kills_the_whole_subtree_of_a_removed_page() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "a"),
            block(3, page(), under(1), 2, "Sub"),
            block(4, BlockKind::Text, under(3), 1, "inside"),
            block(5, BlockKind::Text, under(4), 1, "deeper"),
            block(6, BlockKind::Text, under(1), 3, "b"),
        ]);
        let before = workspace.clone();
        let built = build(
            &workspace,
            &Command::DeleteRange {
                start: at(2, 0),
                end: at(6, 0),
            },
            now(),
        )
        .unwrap();
        let inverse = workspace.apply(&built.transaction).unwrap();
        for removed in [3, 4, 5, 6] {
            assert!(!alive(&workspace, removed), "{removed}");
        }
        assert_eq!(text_of(&workspace, 2), "b");
        workspace.apply(&inverse).unwrap();
        assert_eq!(workspace, before);
    }

    #[test]
    fn move_block_of_a_row_out_of_its_database_is_rejected() {
        let mut workspace = seed();
        for parent in [Parent::Root, under(1)] {
            let error = rejection(
                &mut workspace,
                Command::MoveBlock {
                    block: id(9),
                    target: DropTarget {
                        parent,
                        after: None,
                    },
                },
            );
            assert_eq!(error, DomainError::InvalidParent);
        }
    }

    #[test]
    fn move_block_of_a_page_into_a_database_is_rejected() {
        let mut workspace = seed();
        let error = rejection(
            &mut workspace,
            Command::MoveBlock {
                block: id(1),
                target: DropTarget {
                    parent: Parent::Database(database_id()),
                    after: None,
                },
            },
        );
        assert_eq!(error, DomainError::InvalidParent);
    }

    #[test]
    fn move_block_reorders_rows_inside_their_database() {
        let mut workspace = seed();
        run(
            &mut workspace,
            Command::MoveBlock {
                block: id(10),
                target: DropTarget {
                    parent: Parent::Database(database_id()),
                    after: None,
                },
            },
        );
        assert_eq!(
            ids(&workspace, Parent::Database(database_id())),
            [id(10), id(9)]
        );
    }

    #[test]
    fn every_command_has_an_inverse_that_restores_the_workspace() {
        let mut workspace = notes(vec![
            block(2, BlockKind::Text, under(1), 1, "Hello world"),
            block(3, BlockKind::BulletedListItem, under(1), 2, ""),
            block(4, BlockKind::Text, under(1), 3, "tail"),
            block(5, BlockKind::Text, under(4), 1, "kid"),
        ]);
        let commands = vec![
            Command::SplitBlock { at: at(2, 5) },
            Command::SplitBlock { at: at(3, 0) },
            Command::MergeWithPrevious { block: id(4) },
            Command::Indent { block: id(4) },
            Command::Outdent { block: id(5) },
            Command::DeleteRange {
                start: at(2, 2),
                end: at(4, 1),
            },
            Command::SetMarks {
                start: at(2, 0),
                end: at(4, 1),
                change: MarkChange::Bold(true),
            },
            Command::ChangeKind {
                block: id(2),
                kind: BlockKind::Quote,
            },
            Command::MoveBlock {
                block: id(5),
                target: DropTarget {
                    parent: under(1),
                    after: None,
                },
            },
            Command::EditText {
                block: id(2),
                text: RichText::plain("x"),
            },
            Command::DeleteBlocks {
                blocks: vec![id(1)],
            },
        ];
        for command in commands {
            let before = workspace.clone();
            let Ok(built) = build(&workspace, &command, now()) else {
                continue;
            };
            let inverse = workspace.apply(&built.transaction).unwrap();
            let mut restored = workspace.clone();
            restored.apply(&inverse).unwrap();
            assert_eq!(restored, before, "{command:?}");
        }
    }

    fn pool() -> impl Strategy<Value = BlockId> {
        (1..=10u128).prop_map(id)
    }

    fn point() -> impl Strategy<Value = TextPoint> {
        (pool(), 0usize..4).prop_map(|(block, offset)| TextPoint { block, offset })
    }

    fn any_kind() -> impl Strategy<Value = BlockKind> {
        prop_oneof![
            Just(BlockKind::Text),
            Just(BlockKind::Heading(HeadingLevel::One)),
            Just(BlockKind::BulletedListItem),
            Just(BlockKind::ToDo { checked: false }),
            Just(BlockKind::Divider),
            Just(BlockKind::Toggle),
            Just(BlockKind::Quote),
        ]
    }

    fn any_parent() -> impl Strategy<Value = Parent> {
        prop_oneof![
            Just(Parent::Root),
            Just(Parent::Database(database_id())),
            pool().prop_map(Parent::Block),
        ]
    }

    fn any_command() -> impl Strategy<Value = Command> {
        prop_oneof![
            (pool(), any_kind()).prop_map(|(after, kind)| Command::InsertBlock { after, kind }),
            (any_parent(), proptest::option::of(pool()), any_kind()).prop_map(
                |(parent, after, kind)| Command::InsertBlockAt {
                    target: DropTarget { parent, after },
                    kind
                }
            ),
            (any_parent(), proptest::option::of(pool()))
                .prop_map(|(parent, after)| Command::CreatePage { parent, after }),
            (pool(), "[a-z]{0,4}").prop_map(|(block, text)| Command::EditText {
                block,
                text: RichText::plain(&text)
            }),
            point().prop_map(|at| Command::SplitBlock { at }),
            pool().prop_map(|block| Command::MergeWithPrevious { block }),
            (point(), point()).prop_map(|(start, end)| Command::DeleteRange { start, end }),
            (point(), point(), any::<bool>()).prop_map(|(start, end, on)| Command::SetMarks {
                start,
                end,
                change: MarkChange::Bold(on)
            }),
            pool().prop_map(|block| Command::Indent { block }),
            pool().prop_map(|block| Command::Outdent { block }),
            (pool(), any_kind()).prop_map(|(block, kind)| Command::ChangeKind { block, kind }),
            (pool(), any_parent(), proptest::option::of(pool())).prop_map(
                |(block, parent, after)| Command::MoveBlock {
                    block,
                    target: DropTarget { parent, after }
                }
            ),
            proptest::collection::vec(pool(), 0..3)
                .prop_map(|blocks| Command::DeleteBlocks { blocks }),
            (pool(), proptest::option::of(Just("📄"))).prop_map(|(page, icon)| {
                Command::SetPageIcon {
                    page,
                    icon: icon.map(|raw| Emoji::try_from(raw).unwrap()),
                }
            }),
        ]
    }

    fn database_id() -> DatabaseId {
        DatabaseId::from_uuid(Uuid::from_u128(200))
    }

    fn seed() -> Workspace {
        let title = Property {
            id: PropertyId::from_uuid(Uuid::from_u128(100)),
            name: "Name".to_owned(),
            kind: PropertyKind::Title,
        };
        let notes = Property {
            id: PropertyId::from_uuid(Uuid::from_u128(101)),
            name: "Notes".to_owned(),
            kind: PropertyKind::Text,
        };
        let database = Database {
            id: database_id(),
            name: "Tasks".to_owned(),
            icon: None,
            schema: Schema {
                properties: vec![title.clone(), notes.clone()],
            },
            sort_key: SortKey::parse("V").unwrap(),
            alive: true,
            created_at: now(),
        };
        let view = View {
            id: ViewId::from_uuid(Uuid::from_u128(300)),
            block: id(8),
            name: "Table".to_owned(),
            sort_key: SortKey::parse("V").unwrap(),
            kind: ViewKind::Table,
            filters: Vec::new(),
            sorts: Vec::new(),
            visible: vec![title.id, notes.id],
            column_widths: BTreeMap::new(),
            manual_order: vec![id(9), id(10)],
        };
        Workspace::from_snapshot(WorkspaceSnapshot {
            blocks: vec![
                block(1, page(), Parent::Root, 1, "Notes"),
                block(2, BlockKind::Text, under(1), 1, "ab"),
                block(3, BlockKind::BulletedListItem, under(1), 2, "cd"),
                block(4, BlockKind::Text, under(2), 1, "ef"),
                block(5, BlockKind::Toggle, under(1), 3, "gh"),
                block(6, BlockKind::Text, under(5), 1, "ij"),
                block(7, BlockKind::Divider, under(1), 4, ""),
                block(
                    8,
                    BlockKind::DatabaseView {
                        database: database.id,
                    },
                    under(1),
                    5,
                    "",
                ),
                block(9, page(), Parent::Database(database.id), 1, "Row one"),
                block(10, page(), Parent::Database(database.id), 2, "Row two"),
            ],
            databases: vec![database],
            values: vec![(id(9), notes.id, PropertyValue::Text("x".to_owned()))],
            views: vec![view],
        })
        .unwrap()
    }

    fn removes_a_record(transaction: &Transaction) -> bool {
        transaction.operations().iter().any(|operation| {
            matches!(
                operation,
                Operation::RemoveBlock { .. } | Operation::RemoveDatabase { .. }
            )
        })
    }

    proptest! {
        #[test]
        fn random_command_sequences_keep_the_workspace_valid_and_reversible(
            commands in proptest::collection::vec(any_command(), 0..30)
        ) {
            let mut workspace = seed();
            for command in &commands {
                let before = workspace.clone();
                let Ok(built) = build(&workspace, command, now()) else { continue };
                prop_assert!(!removes_a_record(&built.transaction), "{:?}", command);
                match workspace.apply(&built.transaction) {
                    Ok(inverse) => {
                        let reloaded = Workspace::from_snapshot(workspace.to_snapshot());
                        prop_assert_eq!(reloaded.as_ref(), Ok(&workspace), "{:?}", command);
                        let mut restored = workspace.clone();
                        prop_assert!(restored.apply(&inverse).is_ok());
                        prop_assert_eq!(&restored, &before, "{:?}", command);
                    }
                    Err(_) => prop_assert_eq!(&workspace, &before, "{:?}", command),
                }
            }
        }
    }
}
