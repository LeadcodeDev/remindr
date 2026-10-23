use std::collections::HashMap;

use gpui_kit::base::text::editor_host::LeafGeometry;
use gpui_kit::{Bounds, Pixels, Point, point, px, size};
use remindr_core::application::TextPoint;
use remindr_core::domain::BlockId;

pub const CARET_WIDTH: Pixels = gpui_kit::px(1.5);

pub fn leaf_key(block: BlockId) -> u64 {
    block.as_uuid().as_u64_pair().1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Text(TextPoint),
    Atom(BlockId),
}

impl Hit {
    pub fn point(self) -> TextPoint {
        match self {
            Self::Text(point) => point,
            Self::Atom(block) => TextPoint { block, offset: 0 },
        }
    }
}

#[derive(Default)]
pub struct Frame {
    leaves: HashMap<BlockId, LeafGeometry>,
    rows: Vec<(BlockId, Bounds<Pixels>)>,
}

impl Frame {
    pub fn new(
        leaves: HashMap<BlockId, LeafGeometry>,
        rows: Vec<(BlockId, Bounds<Pixels>)>,
    ) -> Self {
        Self { leaves, rows }
    }

    pub fn leaf(&self, block: BlockId) -> Option<&LeafGeometry> {
        self.leaves.get(&block)
    }

    pub fn rows(&self) -> &[(BlockId, Bounds<Pixels>)] {
        &self.rows
    }

    pub fn hit(&self, at: Point<Pixels>) -> Option<Hit> {
        let last = self.rows.len().checked_sub(1)?;
        let index = self
            .rows
            .partition_point(|(_, bounds)| bounds.bottom() <= at.y)
            .min(last);
        let (block, _) = self.rows.get(index)?;
        Some(match self.leaves.get(block) {
            Some(leaf) => Hit::Text(TextPoint {
                block: *block,
                offset: leaf.index_for_position(at).min(leaf.len()),
            }),
            None => Hit::Atom(*block),
        })
    }

    pub fn caret_bounds(
        &self,
        block: BlockId,
        offset: usize,
        width: Pixels,
    ) -> Option<Bounds<Pixels>> {
        let leaf = self.leaves.get(&block)?;
        let origin = leaf.position_for_index(offset.min(leaf.len()));
        let height = leaf.caret_height();
        let inset = (leaf.line_height() - height) / 2.;
        Some(Bounds::new(
            point(origin.x, origin.y + inset),
            size(width, height),
        ))
    }

    pub fn line_box(&self, block: BlockId, offset: usize) -> Option<Bounds<Pixels>> {
        let leaf = self.leaves.get(&block)?;
        let origin = leaf.position_for_index(offset.min(leaf.len()));
        Some(Bounds::new(origin, size(px(1.), leaf.line_height())))
    }

    pub fn row_box(&self, block: BlockId) -> Option<Bounds<Pixels>> {
        self.rows
            .iter()
            .find(|(candidate, _)| *candidate == block)
            .map(|(_, bounds)| *bounds)
    }

    pub fn line_edges(&self, block: BlockId, offset: usize, text: &str) -> Option<(usize, usize)> {
        let leaf = self.leaves.get(&block)?;
        let line = leaf.position_for_index(offset.min(leaf.len())).y;
        let same_line = |index: usize| {
            let found = leaf.position_for_index(index).y;
            (found - line).abs() < leaf.line_height() / 2.
        };
        let mut start = offset;
        let mut end = offset;
        let boundaries = text
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()));
        for index in boundaries {
            if !same_line(index) {
                continue;
            }
            if index < start {
                start = index;
            }
            if index > end {
                end = index;
            }
        }
        Some((start, end))
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Bounds, point, px, size};
    use remindr_core::domain::{BlockId, Uuid};

    use super::{Frame, Hit};

    fn block(n: u128) -> BlockId {
        BlockId::from_uuid(Uuid::from_u128(n))
    }

    fn row(top: f32) -> Bounds<gpui_kit::Pixels> {
        Bounds::new(point(px(0.), px(top)), size(px(100.), px(20.)))
    }

    #[test]
    fn a_point_below_the_rows_hits_the_last_one() {
        let frame = Frame::new(
            Default::default(),
            vec![(block(1), row(0.)), (block(2), row(20.))],
        );
        assert_eq!(
            frame.hit(point(px(5.), px(500.))),
            Some(Hit::Atom(block(2)))
        );
        assert_eq!(
            frame.hit(point(px(5.), px(-30.))),
            Some(Hit::Atom(block(1)))
        );
        assert_eq!(frame.hit(point(px(5.), px(25.))), Some(Hit::Atom(block(2))));
    }

    #[test]
    fn a_frame_without_rows_hits_nothing() {
        assert_eq!(Frame::default().hit(point(px(1.), px(1.))), None);
    }
}
