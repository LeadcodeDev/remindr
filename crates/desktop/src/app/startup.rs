use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context as _, bail};
use remindr_core::application::{Command, Created, Session};
use remindr_core::domain::{Parent, RichText};

use super::clock::SystemClock;
use super::store::AppStore;

const WORKSPACE_FILE: &str = "workspace.db";
const SEED_TITLE: &str = "Getting started";

pub fn workspace_path(
    override_path: Option<OsString>,
    home: Option<OsString>,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path.filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let Some(home) = home.filter(|home| !home.is_empty()) else {
        bail!("HOME is not set; set REMINDR_WORKSPACE to the workspace file to open");
    };
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("Remindr")
        .join(WORKSPACE_FILE))
}

pub fn open_session(
    open_store: impl Fn() -> anyhow::Result<AppStore>,
) -> anyhow::Result<Session<AppStore, SystemClock>> {
    let mut session =
        Session::open(open_store()?, SystemClock).context("cannot load the workspace")?;
    if seed_if_empty(&mut session)? {
        drop(session);
        session = Session::open(open_store()?, SystemClock)
            .context("cannot reload the seeded workspace")?;
    }
    Ok(session)
}

fn seed_if_empty(session: &mut Session<AppStore, SystemClock>) -> anyhow::Result<bool> {
    let workspace = session.workspace();
    let empty =
        workspace.children(Parent::Root).next().is_none() && workspace.databases().next().is_none();
    if !empty {
        return Ok(false);
    }
    let created = session
        .execute(Command::CreatePage {
            parent: Parent::Root,
            after: None,
        })
        .context("cannot create the first page")?;
    let Some(page) = created.created.iter().find_map(|created| match created {
        Created::Block(block) => Some(*block),
        _ => None,
    }) else {
        bail!("the first page was not reported as created");
    };
    session
        .execute(Command::EditText {
            block: page,
            text: RichText::plain(SEED_TITLE),
        })
        .context("cannot title the first page")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use remindr_core::domain::{Parent, Workspace};
    use remindr_core::testing::MemoryStore;

    use super::{open_session, workspace_path};
    use crate::app::AppStore;

    fn titles(workspace: &Workspace) -> Vec<String> {
        workspace
            .children(Parent::Root)
            .map(|page| page.text.to_plain())
            .collect()
    }

    #[test]
    fn workspace_path_uses_env_override() {
        let path = workspace_path(
            Some(OsString::from("/tmp/custom.db")),
            Some(OsString::from("/Users/test")),
        )
        .unwrap();
        assert_eq!(path, PathBuf::from("/tmp/custom.db"));
    }

    #[test]
    fn workspace_path_defaults_to_application_support() {
        let path = workspace_path(None, Some(OsString::from("/Users/test"))).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/test/Library/Application Support/Remindr/workspace.db")
        );
    }

    #[test]
    fn workspace_path_ignores_an_empty_override() {
        let path =
            workspace_path(Some(OsString::new()), Some(OsString::from("/Users/test"))).unwrap();
        assert!(path.starts_with("/Users/test/Library"));
    }

    #[test]
    fn workspace_path_without_home_or_override_is_an_error() {
        assert!(workspace_path(None, None).is_err());
    }

    #[test]
    fn seeding_an_empty_workspace_creates_getting_started() {
        let store = MemoryStore::new();
        let session = open_session(|| Ok(AppStore::Memory(store.shared()))).unwrap();
        assert_eq!(titles(session.workspace()), vec!["Getting started"]);
        assert_eq!(store.committed(), 2);
    }

    #[test]
    fn seeding_leaves_no_undo_history() {
        let store = MemoryStore::new();
        let session = open_session(|| Ok(AppStore::Memory(store.shared()))).unwrap();
        assert!(!session.can_undo());
    }

    #[test]
    fn seeding_a_non_empty_workspace_does_nothing() {
        let store = MemoryStore::new();
        open_session(|| Ok(AppStore::Memory(store.shared()))).unwrap();
        let committed = store.committed();
        let session = open_session(|| Ok(AppStore::Memory(store.shared()))).unwrap();
        assert_eq!(store.committed(), committed);
        assert_eq!(titles(session.workspace()), vec!["Getting started"]);
    }
}
