use std::path::PathBuf;

use remindr_core::application::{Command, Created, DropTarget, Session, TextPoint};
use remindr_core::domain::{BlockKind, Parent, RichText};
use remindr_store::SqliteStore;

use crate::app::{AppStore, SystemClock};

const BLOCKS: usize = 2000;
const OUT: &str = "REMINDR_BENCH_OUT";

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
    session
        .execute(Command::PasteText {
            at: TextPoint {
                block: first.block,
                offset: 0,
            },
            text: lines.join("\n"),
        })
        .expect("paste the lines");
    let written = session.workspace().children(Parent::Block(page)).count();
    assert_eq!(written, BLOCKS);
}
