mod icon;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{StyledExt as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled, Subscription, TestSupportExt as _, Window, div, prelude::FluentBuilder as _,
    px,
};
use remindr_core::application::{Changes, Command, TextPoint};
use remindr_core::domain::{BlockId, BlockKind, Emoji, Parent, RichText};

use crate::app::WorkspaceModel;
use crate::app::commands;
use crate::database::RowProperties;
use crate::editor::BlockEditor;

#[cfg(test)]
pub use icon::{ICON_SUGGESTIONS, icon_option_id};

const CONTENT_WIDTH: f32 = 860.;
const PAGE_PADDING: f32 = 48.;
const EDITOR_GUTTER: f32 = 24.;
const TITLE_SIZE: f32 = 36.;
const TITLE_HEIGHT: f32 = 56.;

pub struct PageView {
    model: Entity<WorkspaceModel>,
    page: BlockId,
    title: Entity<InputState>,
    icon_input: Entity<InputState>,
    icon_open: bool,
    properties: Option<Entity<RowProperties>>,
    editor: Entity<BlockEditor>,
    _subscriptions: Vec<Subscription>,
}

impl PageView {
    pub fn new(
        model: Entity<WorkspaceModel>,
        page: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let workspace = model.read(cx).workspace();
        let block = workspace.block(page);
        let text = block.map(|block| block.text.to_plain()).unwrap_or_default();
        let is_row = block.is_some_and(|block| matches!(block.parent, Parent::Database(_)));
        let is_blank = text.is_empty() && workspace.children(Parent::Block(page)).next().is_none();
        let title = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Untitled")
                .default_value(text)
        });
        if is_blank {
            title.update(cx, |title, cx| title.focus(window, cx));
        }
        let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder("Any emoji"));
        let properties = is_row.then(|| {
            let model = model.clone();
            cx.new(|cx| RowProperties::new(model, page, window, cx))
        });
        let editor = {
            let model = model.clone();
            cx.new(|cx| BlockEditor::new(model, page, window, cx))
        };
        let subscriptions = vec![
            cx.subscribe_in(&title, window, Self::on_title_event),
            cx.subscribe_in(&icon_input, window, Self::on_icon_input_event),
            cx.subscribe_in(&model, window, Self::on_changes),
        ];
        Self {
            model,
            page,
            title,
            icon_input,
            icon_open: false,
            properties,
            editor,
            _subscriptions: subscriptions,
        }
    }

    fn current_icon(&self, cx: &Context<Self>) -> Option<String> {
        let block = self.model.read(cx).workspace().block(self.page)?;
        match &block.kind {
            BlockKind::Page { icon } => icon.as_ref().map(|icon| icon.as_str().to_owned()),
            _ => None,
        }
    }

    fn on_title_event(
        &mut self,
        title: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                let value = title.read(cx).value();
                commands::run(
                    &self.model,
                    Command::EditText {
                        block: self.page,
                        text: RichText::plain(&value),
                    },
                    window,
                    cx,
                );
            }
            InputEvent::PressEnter { .. } => {
                let value = title.read(cx).value();
                commands::run(
                    &self.model,
                    Command::SplitBlock {
                        at: TextPoint {
                            block: self.page,
                            offset: value.len(),
                        },
                    },
                    window,
                    cx,
                );
            }
            InputEvent::Focus | InputEvent::Blur => {}
        }
    }

    fn on_icon_input_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::PressEnter { .. }) {
            let value = input.read(cx).value();
            let value = value.trim();
            if !value.is_empty() {
                self.choose_icon(Some(value), window, cx);
            }
        }
    }

    fn on_changes(
        &mut self,
        model: &Entity<WorkspaceModel>,
        changes: &Changes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !changes.blocks.contains(&self.page) {
            return;
        }
        let text = model
            .read(cx)
            .workspace()
            .block(self.page)
            .map(|block| block.text.to_plain())
            .unwrap_or_default();
        if self.title.read(cx).value() != text {
            self.title
                .update(cx, |title, cx| title.set_value(text, window, cx));
        }
        cx.notify();
    }

    fn choose_icon(&mut self, raw: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let icon = match raw.map(Emoji::try_from).transpose() {
            Ok(icon) => icon,
            Err(error) => {
                window.push_notification(Notification::error(error.to_string()), cx);
                return;
            }
        };
        commands::run(
            &self.model,
            Command::SetPageIcon {
                page: self.page,
                icon,
            },
            window,
            cx,
        );
        self.icon_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.icon_open = false;
        cx.notify();
    }
}

impl Render for PageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let icon = self.current_icon(cx);
        let picker = icon::Picker {
            view: cx.entity(),
            input: self.icon_input.clone(),
            open: self.icon_open,
            current: icon,
        };
        let background = cx.theme().background;
        let title = Input::new(&self.title)
            .id("page-title")
            .appearance(false)
            .bordered(false)
            .focus_bordered(false);
        let title = Styled::h(title, px(TITLE_HEIGHT))
            .px_0()
            .text_size(px(TITLE_SIZE))
            .font_bold();
        v_flex()
            .id("page-view")
            .test_support()
            .size_full()
            .bg(background)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(CONTENT_WIDTH))
                    .mx_auto()
                    .pl(px(PAGE_PADDING + EDITOR_GUTTER))
                    .pr(px(PAGE_PADDING))
                    .pt_10()
                    .gap_1()
                    .flex_none()
                    .child(
                        v_flex()
                            .id("page-header")
                            .test_support()
                            .group("page-header")
                            .child(h_flex().child(picker))
                            .child(title),
                    )
                    .when_some(self.properties.clone(), |this, properties| {
                        this.child(properties)
                    }),
            )
            .child(
                h_flex().flex_1().min_h_0().justify_center().child(
                    div()
                        .size_full()
                        .max_w(px(CONTENT_WIDTH))
                        .px(px(PAGE_PADDING))
                        .child(self.editor.clone()),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;
    use remindr_core::application::Command;
    use remindr_core::domain::{BlockId, BlockKind, Parent, RichText};

    use super::{ICON_SUGGESTIONS, icon_option_id};
    use crate::app::testing::{Ui, add_page, create_database, execute, model, model_with};
    use crate::app::{Route, WorkspaceModel};
    use crate::sidebar::{database_open_id, page_icon_id, page_open_id, page_row_id};
    use gpui_kit::Entity;

    fn title_of(model: &Entity<WorkspaceModel>, cx: &mut TestAppContext, page: BlockId) -> String {
        model.read_with(cx, |model, _| {
            model
                .workspace()
                .block(page)
                .map(|block| block.text.to_plain())
                .unwrap_or_default()
        })
    }

    fn one_page(cx: &mut TestAppContext, title: &str) -> (Entity<WorkspaceModel>, BlockId) {
        let mut page = None;
        let model = model_with(cx, |session| {
            page = Some(add_page(session, Parent::Root, title));
        });
        (model, page.unwrap())
    }

    #[gpui_kit::test]
    fn title_edit_updates_the_sidebar(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Old");
        let ui = Ui::open(cx, &model);
        assert_eq!(
            ui.find(cx, "page-title")
                .and_then(|input| input.value().map(str::to_owned)),
            Some("Old".to_owned())
        );
        ui.click(cx, "page-title");
        ui.press(cx, "cmd-a");
        ui.input(cx, "New");
        assert_eq!(title_of(&model, cx, page), "New");
        assert_eq!(ui.label(cx, page_open_id(page)).as_deref(), Some("New"));
        assert_eq!(ui.label(cx, "breadcrumb").as_deref(), Some("New"));
    }

    #[gpui_kit::test]
    fn a_new_page_focuses_its_title(cx: &mut TestAppContext) {
        let model = model(cx);
        let ui = Ui::open(cx, &model);
        ui.click(cx, "new-page");
        let page = model.read_with(cx, |model, _| {
            crate::app::navigation::root_pages(model.workspace())[0]
        });
        ui.input(cx, "Fresh");
        assert_eq!(title_of(&model, cx, page), "Fresh");
        assert_eq!(ui.label(cx, page_open_id(page)).as_deref(), Some("Fresh"));
    }

    #[gpui_kit::test]
    fn title_follows_changes_made_elsewhere(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Old");
        let ui = Ui::open(cx, &model);
        execute(
            &model,
            cx,
            Command::EditText {
                block: page,
                text: RichText::plain("Elsewhere"),
            },
        );
        ui.settle(cx);
        assert_eq!(
            ui.find(cx, "page-title")
                .and_then(|input| input.value().map(str::to_owned)),
            Some("Elsewhere".to_owned())
        );
    }

    #[gpui_kit::test]
    async fn icon_pick_updates_the_sidebar(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Page");
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, page_icon_id(page)).is_none());
        ui.hover(cx, "page-header");
        ui.click(cx, "page-icon");
        ui.wait_for(cx, |window, _| {
            gpui_kit::test::TestWindowExt::try_find(window, icon_option_id(3)).is_some()
        })
        .await;
        ui.click(cx, icon_option_id(3));
        let picked = ICON_SUGGESTIONS[3];
        let stored = model.read_with(cx, |model, _| {
            model
                .workspace()
                .block(page)
                .map(|block| block.kind.clone())
        });
        assert!(matches!(
            stored,
            Some(BlockKind::Page { icon: Some(ref icon) }) if icon.as_str() == picked
        ));
        assert_eq!(ui.label(cx, page_icon_id(page)).as_deref(), Some(picked));
        assert_eq!(ui.label(cx, "page-icon").as_deref(), Some(picked));
    }

    #[gpui_kit::test]
    async fn icon_can_be_typed_and_removed(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Page");
        let ui = Ui::open(cx, &model);
        ui.hover(cx, "page-header");
        ui.click(cx, "page-icon");
        ui.wait_for(cx, |window, _| {
            gpui_kit::test::TestWindowExt::try_find(window, "icon-input").is_some()
        })
        .await;
        ui.click(cx, "icon-input");
        ui.input(cx, "🦀");
        ui.press(cx, "enter");
        assert_eq!(ui.label(cx, page_icon_id(page)).as_deref(), Some("🦀"));
        ui.click(cx, "page-icon");
        ui.wait_for(cx, |window, _| {
            gpui_kit::test::TestWindowExt::try_find(window, "icon-remove").is_some()
        })
        .await;
        ui.click(cx, "icon-remove");
        assert!(ui.find(cx, page_icon_id(page)).is_none());
    }

    #[gpui_kit::test]
    fn enter_in_title_creates_the_first_block(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Title");
        let ui = Ui::open(cx, &model);
        ui.click(cx, "page-title");
        ui.press(cx, "enter");
        let children = model.read_with(cx, |model, _| {
            model
                .workspace()
                .children(Parent::Block(page))
                .map(|block| block.kind.clone())
                .collect::<Vec<_>>()
        });
        assert_eq!(children, vec![BlockKind::Text]);
        assert_eq!(title_of(&model, cx, page), "Title");
    }

    #[gpui_kit::test]
    fn deleting_the_open_page_falls_back_to_the_previous_one(cx: &mut TestAppContext) {
        let (mut first, mut second) = (None, None);
        let model = model_with(cx, |session| {
            first = Some(add_page(session, Parent::Root, "First"));
            second = Some(add_page(session, Parent::Root, "Second"));
        });
        let (first, second) = (first.unwrap(), second.unwrap());
        let ui = Ui::open(cx, &model);
        ui.click(cx, page_open_id(second));
        assert_eq!(ui.route(cx), Some(Route::Page(second)));
        execute(
            &model,
            cx,
            Command::DeleteBlocks {
                blocks: vec![second],
            },
        );
        ui.settle(cx);
        assert_eq!(ui.route(cx), Some(Route::Page(first)));
        assert!(ui.find(cx, "page-view").is_some());
    }

    #[gpui_kit::test]
    fn deleting_the_open_page_without_history_opens_the_first_root_page(cx: &mut TestAppContext) {
        let (mut first, mut second) = (None, None);
        let model = model_with(cx, |session| {
            first = Some(add_page(session, Parent::Root, "First"));
            second = Some(add_page(session, Parent::Root, "Second"));
        });
        let (first, second) = (first.unwrap(), second.unwrap());
        let ui = Ui::open(cx, &model);
        assert_eq!(ui.route(cx), Some(Route::Page(first)));
        execute(
            &model,
            cx,
            Command::DeleteBlocks {
                blocks: vec![first],
            },
        );
        ui.settle(cx);
        assert_eq!(ui.route(cx), Some(Route::Page(second)));
    }

    #[gpui_kit::test]
    fn deleting_the_open_database_falls_back_to_the_previous_route(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Page");
        create_database(&model, cx);
        let database = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .databases()
                    .next()
                    .map(|database| database.id)
            })
            .unwrap();
        let ui = Ui::open(cx, &model);
        ui.click(cx, database_open_id(database));
        assert_eq!(ui.route(cx), Some(Route::Database(database)));
        execute(&model, cx, Command::DeleteDatabase { database });
        ui.settle(cx);
        assert_eq!(ui.route(cx), Some(Route::Page(page)));
        assert!(ui.find(cx, "database-screen").is_none());
        assert!(ui.find(cx, database_open_id(database)).is_none());
    }

    #[gpui_kit::test]
    fn deleting_the_last_page_shows_the_empty_state(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Only");
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, "page-view").is_some());
        execute(&model, cx, Command::DeleteBlocks { blocks: vec![page] });
        ui.settle(cx);
        assert_eq!(ui.route(cx), None);
        assert!(ui.find(cx, "page-view").is_none());
        assert!(ui.find(cx, "empty-state").is_some());
        model.update(cx, |model, cx| model.undo(cx).unwrap());
        ui.settle(cx);
        assert_eq!(ui.route(cx), Some(Route::Page(page)));
        assert!(ui.find(cx, "empty-state").is_none());
    }

    #[gpui_kit::test]
    fn database_route_shows_the_database_screen(cx: &mut TestAppContext) {
        let (model, page) = one_page(cx, "Page");
        create_database(&model, cx);
        let database = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .databases()
                    .next()
                    .map(|database| database.id)
            })
            .unwrap();
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, "database-screen").is_none());
        ui.click(cx, database_open_id(database));
        assert!(ui.find(cx, "database-screen").is_some());
        assert!(ui.find(cx, "page-view").is_none());
        assert_eq!(ui.label(cx, "breadcrumb").as_deref(), Some("Untitled"));
        ui.click(cx, page_open_id(page));
        assert!(ui.find(cx, "database-screen").is_none());
        assert!(ui.find(cx, "page-view").is_some());
        let _ = page_row_id(page);
    }

    #[gpui_kit::test]
    fn row_pages_show_their_properties_between_title_and_editor(cx: &mut TestAppContext) {
        let model = model(cx);
        create_database(&model, cx);
        let database = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .databases()
                    .next()
                    .map(|database| database.id)
            })
            .unwrap();
        execute(
            &model,
            cx,
            Command::AddRow {
                database,
                values: Vec::new(),
            },
        );
        let row = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .rows(database)
                    .next()
                    .map(|block| block.id)
            })
            .unwrap();
        let ui = Ui::open(cx, &model);
        let navigator = ui.navigator(cx);
        navigator.update(cx, |navigator, cx| navigator.open(Route::Page(row), cx));
        ui.settle(cx);
        let title = ui.find(cx, "page-title").unwrap().bounds();
        let properties = ui.find(cx, "row-properties").unwrap().bounds();
        let editor = ui.find(cx, "block-editor").unwrap().bounds();
        assert!(properties.top() >= title.bottom());
        assert!(editor.top() >= properties.bottom());
    }

    #[gpui_kit::test]
    fn plain_pages_have_no_row_properties(cx: &mut TestAppContext) {
        let (model, _) = one_page(cx, "Page");
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, "row-properties").is_none());
        assert!(ui.find(cx, "block-editor").is_some());
    }
}
