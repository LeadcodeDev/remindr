use std::path::PathBuf;
use std::time::{Duration, Instant};

use remindr_core::application::{Command, Created, DropTarget, Session, TextPoint};
use remindr_core::domain::{BlockKind, Parent, RichText};
use remindr_store::SqliteStore;

use crate::app::{AppStore, SystemClock};

const BLOCKS: usize = 2000;
const SPLITS: usize = 5;
const OUT: &str = "REMINDR_BENCH_OUT";

fn millis(elapsed: Duration) -> f64 {
    elapsed.as_secs_f64() * 1000.
}

#[test]
#[ignore = "writes the 2,000 block workspace used for the first paint measurement"]
fn write_a_2000_block_workspace() {
    let path = PathBuf::from(std::env::var_os(OUT).expect("REMINDR_BENCH_OUT names the file"));
    let _ = std::fs::remove_file(&path);
    let store = SqliteStore::open(&path).expect("open the workspace file");
    let mut session = Session::open(AppStore::Sqlite(store), SystemClock).expect("open session");
    let page = session
        .execute(Command::CreatePage {
            parent: Parent::Root,
            after: None,
        })
        .expect("create the page")
        .created
        .iter()
        .find_map(|created| match created {
            Created::Block(block) => Some(*block),
            _ => None,
        })
        .expect("page id");
    session
        .execute(Command::EditText {
            block: page,
            text: RichText::plain("Two thousand blocks"),
        })
        .expect("title");
    let first = session
        .execute(Command::InsertBlockAt {
            target: DropTarget {
                parent: Parent::Block(page),
                after: None,
            },
            kind: BlockKind::Text,
        })
        .expect("first block")
        .caret
        .expect("caret");
    let lines: Vec<String> = (0..BLOCKS)
        .map(|number| format!("Line {number}: the quick brown fox jumps over the lazy dog"))
        .collect();
    let started = Instant::now();
    session
        .execute(Command::PasteText {
            at: TextPoint {
                block: first.block,
                offset: 0,
            },
            text: lines.join("\n"),
        })
        .expect("paste the lines");
    let pasted = started.elapsed();
    let written = session.workspace().children(Parent::Block(page)).count();
    assert_eq!(written, BLOCKS);
    eprintln!(
        "PasteText of {BLOCKS} lines committed to SqliteStore: {:.1} ms",
        millis(pasted)
    );
    let targets: Vec<_> = session
        .workspace()
        .children(Parent::Block(page))
        .skip(BLOCKS / 4)
        .step_by(BLOCKS / (2 * SPLITS))
        .take(SPLITS)
        .map(|block| block.id)
        .collect();
    let mut splits = Vec::new();
    for block in targets {
        let started = Instant::now();
        session
            .execute(Command::SplitBlock {
                at: TextPoint { block, offset: 5 },
            })
            .expect("split a block");
        splits.push(millis(started.elapsed()));
    }
    splits.sort_by(f64::total_cmp);
    eprintln!(
        "SplitBlock in the {BLOCKS}-block workspace, {SPLITS} runs: min {:.2} ms, median {:.2} ms, max {:.2} ms",
        splits[0],
        splits[splits.len() / 2],
        splits[splits.len() - 1]
    );
    assert_eq!(
        session.workspace().children(Parent::Block(page)).count(),
        BLOCKS + SPLITS
    );
}
