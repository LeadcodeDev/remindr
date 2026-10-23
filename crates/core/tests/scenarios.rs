#![allow(clippy::expect_used, clippy::unwrap_used)]

mod steps;

use std::collections::HashMap;
use std::fmt;

use cucumber::World;
use cucumber::writer::Stats as _;
use remindr_core::application::{Changes, Command, Session};
use remindr_core::domain::{
    BlockId, DatabaseId, OptionId, PropertyId, Timestamp, ViewId, WorkspaceSnapshot,
};
use remindr_core::testing::{ManualClock, MemoryStore};

const START_MILLIS: i64 = 1_700_000_000_000;

#[derive(World)]
#[world(init = Self::new)]
pub struct RemindrWorld {
    pub store: MemoryStore,
    pub clock: ManualClock,
    pub session: Session<MemoryStore, ManualClock>,
    pub blocks: HashMap<String, BlockId>,
    pub databases: HashMap<String, DatabaseId>,
    pub properties: HashMap<String, PropertyId>,
    pub options: HashMap<String, OptionId>,
    pub views: HashMap<String, ViewId>,
    pub before: Option<WorkspaceSnapshot>,
    pub last: Option<Result<Changes, String>>,
}

impl RemindrWorld {
    fn new() -> Self {
        let store = MemoryStore::new();
        let clock = ManualClock::new(Timestamp::from_millis(START_MILLIS));
        let session = Session::open(store.shared(), clock.clone()).expect("session opens");
        Self {
            store,
            clock,
            session,
            blocks: HashMap::new(),
            databases: HashMap::new(),
            properties: HashMap::new(),
            options: HashMap::new(),
            views: HashMap::new(),
            before: None,
            last: None,
        }
    }

    pub fn run(&mut self, command: Command) {
        self.before = Some(self.session.workspace().to_snapshot());
        self.last = Some(
            self.session
                .execute(command)
                .map_err(|error| error.to_string()),
        );
    }

    pub fn changes(&self) -> &Changes {
        match &self.last {
            Some(Ok(changes)) => changes,
            other => panic!("expected a successful command, got {other:?}"),
        }
    }

    pub fn block(&self, name: &str) -> BlockId {
        *self
            .blocks
            .get(name)
            .unwrap_or_else(|| panic!("unknown block {name}"))
    }

    pub fn database(&self, name: &str) -> DatabaseId {
        *self
            .databases
            .get(name)
            .unwrap_or_else(|| panic!("unknown database {name}"))
    }

    pub fn property(&self, name: &str) -> PropertyId {
        *self
            .properties
            .get(name)
            .unwrap_or_else(|| panic!("unknown property {name}"))
    }

    pub fn option(&self, name: &str) -> OptionId {
        *self
            .options
            .get(name)
            .unwrap_or_else(|| panic!("unknown option {name}"))
    }

    pub fn view(&self, name: &str) -> ViewId {
        *self
            .views
            .get(name)
            .unwrap_or_else(|| panic!("unknown view {name}"))
    }
}

impl fmt::Debug for RemindrWorld {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemindrWorld")
            .field("blocks", &self.blocks)
            .field("databases", &self.databases)
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

pub fn run_feature(path: &str) {
    let failed = futures::executor::block_on(async {
        RemindrWorld::cucumber()
            .with_cli(cucumber::cli::Opts::<_, _, _, cucumber::cli::Empty>::default())
            .fail_on_skipped()
            .run(path)
            .await
            .execution_has_failed()
    });
    assert!(!failed, "scenarios failed in {path}");
}
