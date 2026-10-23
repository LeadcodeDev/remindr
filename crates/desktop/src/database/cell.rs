mod date;
mod options;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, Selectable, Sizable as _, h_flex};
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, ElementId, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{
    BlockId, DatabaseId, NumberFormat, Property, PropertyId, PropertyKind, PropertyValue, RichText,
    SelectOption,
};

use self::date::DatePanel;
use self::options::OptionsPicker;
use super::format::{Parsed, date_text, format_number, parse_number, plain_number};
use super::ids::{cell_id, cell_text_id, open_row_id};
use super::tags::option_tag;
use crate::app::{NavigatorHandle, Route, WorkspaceModel, commands};
use crate::theme::{TABLE_ROW, UI_TEXT, UI_TEXT_SMALL};

pub struct Done;

pub fn write_value(
    model: &Entity<WorkspaceModel>,
    row: BlockId,
    property: PropertyId,
    value: Option<PropertyValue>,
    window: &mut Window,
    cx: &mut App,
) {
    let current = model.read(cx).workspace().value(row, property).cloned();
    if current != value {
        commands::run(
            model,
            Command::SetValue {
                row,
                property,
                value,
            },
            window,
            cx,
        );
    }
}

enum Mode {
    Idle,
    Field(Field),
    Options {
        picker: Entity<OptionsPicker>,
        _subscription: Subscription,
    },
    Calendar {
        panel: Entity<DatePanel>,
        _subscription: Subscription,
    },
}

struct Field {
    input: Entity<InputState>,
    _subscription: Subscription,
}

pub struct Cell {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    row: BlockId,
    property: PropertyId,
    mode: Mode,
    invalid: bool,
    _subscription: Subscription,
}

#[derive(Clone, Copy, PartialEq)]
enum Text {
    Title,
    Plain,
    Link,
    Number(NumberFormat),
}

enum Commit {
    Done,
    Invalid,
}

impl Cell {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        row: BlockId,
        property: PropertyId,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&model, |this, _, changes: &Changes, cx| {
            if changes.blocks.contains(&this.row) || changes.databases.contains(&this.database) {
                cx.notify();
            }
        });
        Self {
            model,
            database,
            row,
            property,
            mode: Mode::Idle,
            invalid: false,
            _subscription: subscription,
        }
    }

    fn definition(&self, cx: &App) -> Option<Property> {
        self.model
            .read(cx)
            .workspace()
            .database(self.database)?
            .schema
            .properties
            .iter()
            .find(|property| property.id == self.property)
            .cloned()
    }

    fn value(&self, cx: &App) -> Option<PropertyValue> {
        self.model
            .read(cx)
            .workspace()
            .value(self.row, self.property)
            .cloned()
    }

    fn title(&self, cx: &App) -> String {
        self.model
            .read(cx)
            .workspace()
            .block(self.row)
            .map(|block| block.text.to_plain())
            .unwrap_or_default()
    }

    fn raw_text(&self, text: Text, cx: &App) -> String {
        match (text, self.value(cx)) {
            (Text::Title, _) => self.title(cx),
            (Text::Plain, Some(PropertyValue::Text(value)))
            | (Text::Link, Some(PropertyValue::Url(value))) => value,
            (Text::Number(_), Some(PropertyValue::Number(value))) => plain_number(value),
            _ => String::new(),
        }
    }

    fn text_kind(kind: &PropertyKind) -> Option<Text> {
        match kind {
            PropertyKind::Title => Some(Text::Title),
            PropertyKind::Text => Some(Text::Plain),
            PropertyKind::Url => Some(Text::Link),
            PropertyKind::Number { format } => Some(Text::Number(*format)),
            _ => None,
        }
    }

    fn begin_edit(&mut self, text: Text, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.mode, Mode::Field(_)) {
            return;
        }
        let initial = self.raw_text(text, cx);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
        let subscription = cx.subscribe_in(&input, window, move |this, _, event, window, cx| {
            this.on_field_event(text, event, window, cx);
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        self.invalid = false;
        self.mode = Mode::Field(Field {
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        if matches!(self.mode, Mode::Field(_)) {
            self.mode = Mode::Idle;
            self.invalid = false;
            cx.notify();
        }
    }

    fn on_field_event(
        &mut self,
        text: Text,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Field(field) = &self.mode else {
            return;
        };
        let typed = field.input.read(cx).value().to_string();
        match event {
            InputEvent::Change => {
                self.invalid = matches!(text, Text::Number(_))
                    && matches!(parse_number(&typed), Parsed::Invalid);
                cx.notify();
            }
            InputEvent::PressEnter { .. } => self.finish(text, &typed, true, window, cx),
            InputEvent::Blur => self.finish(text, &typed, false, window, cx),
            InputEvent::Focus => {}
        }
    }

    fn finish(
        &mut self,
        text: Text,
        typed: &str,
        keep_invalid: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.commit(text, typed, window, cx), Commit::Invalid) && keep_invalid {
            self.invalid = true;
            cx.notify();
            return;
        }
        self.mode = Mode::Idle;
        self.invalid = false;
        cx.notify();
    }

    fn commit(
        &mut self,
        text: Text,
        typed: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Commit {
        let value = match text {
            Text::Title => {
                if self.title(cx) != typed {
                    commands::run(
                        &self.model,
                        Command::EditText {
                            block: self.row,
                            text: RichText::plain(typed),
                        },
                        window,
                        cx,
                    );
                }
                return Commit::Done;
            }
            Text::Plain => (!typed.is_empty()).then(|| PropertyValue::Text(typed.to_owned())),
            Text::Link => (!typed.is_empty()).then(|| PropertyValue::Url(typed.to_owned())),
            Text::Number(_) => match parse_number(typed) {
                Parsed::Empty => None,
                Parsed::Number(number) => Some(PropertyValue::Number(number)),
                Parsed::Invalid => return Commit::Invalid,
            },
        };
        write_value(&self.model, self.row, self.property, value, window, cx);
        Commit::Done
    }

    fn set_popover(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !open {
            self.mode = Mode::Idle;
            cx.notify();
            return;
        }
        let Some(definition) = self.definition(cx) else {
            return;
        };
        let (model, database, row, property) =
            (self.model.clone(), self.database, self.row, self.property);
        match definition.kind {
            PropertyKind::Select { .. } | PropertyKind::MultiSelect { .. } => {
                let multiple = matches!(definition.kind, PropertyKind::MultiSelect { .. });
                let picker = cx.new(|cx| {
                    OptionsPicker::new(model, database, row, property, multiple, window, cx)
                });
                let subscription = cx.subscribe(&picker, |this, _, _: &Done, cx| {
                    this.mode = Mode::Idle;
                    cx.notify();
                });
                picker.update(cx, |picker, cx| picker.focus(window, cx));
                self.mode = Mode::Options {
                    picker,
                    _subscription: subscription,
                };
            }
            PropertyKind::Date => {
                let panel = cx.new(|cx| DatePanel::new(model, row, property, window, cx));
                let subscription = cx.subscribe(&panel, |this, _, _: &Done, cx| {
                    this.mode = Mode::Idle;
                    cx.notify();
                });
                self.mode = Mode::Calendar {
                    panel,
                    _subscription: subscription,
                };
            }
            _ => return,
        }
        cx.notify();
    }

    fn open_page(&mut self, cx: &mut Context<Self>) {
        let Some(navigator) = cx
            .try_global::<NavigatorHandle>()
            .map(|handle| handle.0.clone())
        else {
            return;
        };
        let row = self.row;
        navigator.update(cx, |navigator, cx| navigator.open(Route::Page(row), cx));
    }

    fn on_text_click(
        &mut self,
        text: Text,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text == Text::Link
            && event.modifiers().secondary()
            && let Some(PropertyValue::Url(url)) = self.value(cx)
        {
            cx.open_url(&url);
            return;
        }
        self.begin_edit(text, window, cx);
    }

    fn text_cell(&mut self, text: Text, cx: &mut Context<Self>) -> AnyElement {
        let id = cell_id(self.row, self.property);
        let raw = self.raw_text(text, cx);
        let display = match text {
            Text::Number(format) => match self.value(cx) {
                Some(PropertyValue::Number(value)) => format_number(value, format),
                _ => String::new(),
            },
            _ => raw,
        };
        let label = if text == Text::Title && display.is_empty() {
            "Untitled".to_owned()
        } else {
            display.clone()
        };
        let editing = match &self.mode {
            Mode::Field(field) => Some(field.input.clone()),
            _ => None,
        };
        let invalid = self.invalid;
        let danger = cx.theme().danger;
        let muted = cx.theme().muted_foreground;
        let background = cx.theme().background;
        let link = cx.theme().link;
        let group = SharedString::from(format!("cell-{}", self.row.as_uuid()));
        let body = match editing {
            Some(input) => div()
                .flex_1()
                .min_w_0()
                .child(
                    Input::new(&input)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .small()
                        .text_size(UI_TEXT),
                )
                .into_any_element(),
            None => {
                let shown = div()
                    .id(cell_text_id(self.row, self.property))
                    .test_support()
                    .flex_1()
                    .min_w_0()
                    .truncate();
                match text {
                    Text::Title if display.is_empty() => {
                        shown.text_color(muted).child("Untitled").into_any_element()
                    }
                    Text::Link if !display.is_empty() => shown
                        .text_color(link)
                        .underline()
                        .child(display)
                        .into_any_element(),
                    _ => shown.child(display).into_any_element(),
                }
            }
        };
        h_flex()
            .id(id)
            .test_support()
            .aria_label(label)
            .when(invalid, |this| this.aria_value("invalid"))
            .group(group.clone())
            .relative()
            .size_full()
            .px_2()
            .items_center()
            .overflow_hidden()
            .cursor_text()
            .text_size(UI_TEXT)
            .when(invalid, |this| this.border_1().border_color(danger))
            .on_action(cx.listener(|this, _: &Escape, _, cx| this.cancel_edit(cx)))
            .on_click(cx.listener(move |this, event, window, cx| {
                this.on_text_click(text, event, window, cx);
            }))
            .child(body)
            .when(text == Text::Title, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_1()
                        .flex()
                        .items_center()
                        .bg(background)
                        .opacity(0.)
                        .group_hover(group, |style| style.opacity(1.))
                        .child(
                            Button::new(open_row_id(self.row))
                                .ghost()
                                .xsmall()
                                .child(div().text_size(UI_TEXT_SMALL).child("Open"))
                                .on_click(cx.listener(|this, _, _, cx| this.open_page(cx))),
                        ),
                )
            })
            .into_any_element()
    }

    fn popover_cell(
        &mut self,
        label: String,
        children: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = cell_id(self.row, self.property);
        let (open, content) = match &self.mode {
            Mode::Options { picker, .. } => (true, Some(AnyView::from(picker.clone()))),
            Mode::Calendar { panel, .. } => (true, Some(AnyView::from(panel.clone()))),
            _ => (false, None),
        };
        let cell = cx.entity();
        let trigger = Trigger {
            id: id.clone(),
            label: label.into(),
            open,
            children,
        };
        let mut popover = Popover::new(ElementId::Name(
            format!(
                "cell-popover-{}-{}",
                self.row.as_uuid(),
                self.property.as_uuid()
            )
            .into(),
        ))
        .open(open)
        .on_open_change(move |open, window, cx| {
            let open = *open;
            cell.update(cx, |this, cx| this.set_popover(open, window, cx));
        })
        .trigger(trigger);
        if let Some(content) = content {
            popover = popover.content(move |_, _, _| content.clone());
        }
        div()
            .size_full()
            .text_size(UI_TEXT)
            .child(popover.w_full().h_full())
            .into_any_element()
    }

    fn options_cell(&mut self, options: &[SelectOption], cx: &mut Context<Self>) -> AnyElement {
        let chosen: Vec<&SelectOption> = match self.value(cx) {
            Some(PropertyValue::Select(id)) => options.iter().filter(|o| o.id == id).collect(),
            Some(PropertyValue::MultiSelect(ids)) => ids
                .iter()
                .filter_map(|id| options.iter().find(|option| option.id == *id))
                .collect(),
            _ => Vec::new(),
        };
        let label = chosen
            .iter()
            .map(|option| option.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let tags = chosen
            .into_iter()
            .map(|option| option_tag(option).into_any_element())
            .collect();
        self.popover_cell(label, tags, cx)
    }

    fn date_cell(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let text = match self.value(cx) {
            Some(PropertyValue::Date(date)) => date_text(date),
            _ => String::new(),
        };
        self.popover_cell(text.clone(), vec![div().child(text).into_any_element()], cx)
    }

    fn checkbox_cell(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let checked = matches!(self.value(cx), Some(PropertyValue::Checkbox(true)));
        let id = cell_id(self.row, self.property);
        h_flex()
            .size_full()
            .px_2()
            .items_center()
            .text_size(UI_TEXT)
            .child(Checkbox::new(id).checked(checked).on_click(cx.listener(
                |this, checked: &bool, window, cx| {
                    write_value(
                        &this.model,
                        this.row,
                        this.property,
                        Some(PropertyValue::Checkbox(*checked)),
                        window,
                        cx,
                    );
                },
            )))
            .into_any_element()
    }
}

impl Render for Cell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(definition) = self.definition(cx) else {
            return div().size_full().into_any_element();
        };
        if let Some(text) = Self::text_kind(&definition.kind) {
            return self.text_cell(text, cx);
        }
        match &definition.kind {
            PropertyKind::Select { options } | PropertyKind::MultiSelect { options } => {
                self.options_cell(options, cx)
            }
            PropertyKind::Date => self.date_cell(cx),
            _ => self.checkbox_cell(cx),
        }
    }
}

#[derive(IntoElement)]
struct Trigger {
    id: ElementId,
    label: SharedString,
    open: bool,
    children: Vec<AnyElement>,
}

impl gpui_kit::ParentElement for Trigger {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Selectable for Trigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for Trigger {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        h_flex()
            .id(self.id)
            .test_support()
            .aria_label(self.label)
            .aria_expanded(self.open)
            .w_full()
            .h(TABLE_ROW)
            .text_size(UI_TEXT)
            .px_2()
            .gap_1()
            .items_center()
            .overflow_hidden()
            .cursor_pointer()
            .children(self.children)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, Modifiers,
        ParentElement as _, Render, Styled as _, TestAppContext, TestSupportExt as _, Window, div,
        px,
    };
    use remindr_core::application::Command;
    use remindr_core::domain::{
        BlockId, CalendarDate, DatabaseId, NumberFormat, OptionColor, PropertyId, PropertyKind,
        PropertyValue, RichText,
    };

    use super::Cell;
    use crate::app::testing::{Ui, execute, model_with};
    use crate::app::{Route, WorkspaceModel};
    use crate::database::ids::{cell_id, open_row_id, option_id};
    use crate::database::testing::{UiExt as _, add_option, one, open, value_of};

    struct Host {
        cell: Entity<Cell>,
        elsewhere: gpui_kit::FocusHandle,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .p_4()
                .child(div().w(px(260.)).h(px(32.)).child(self.cell.clone()))
                .child(
                    div()
                        .id("elsewhere")
                        .test_support()
                        .track_focus(&self.elsewhere)
                        .size_4(),
                )
        }
    }

    type Model = Entity<WorkspaceModel>;

    fn mount(
        cx: &mut TestAppContext,
        model: &Model,
        database: DatabaseId,
        row: BlockId,
        property: PropertyId,
    ) -> Ui {
        let shared = model.clone();
        let (ui, _) = open(cx, model, move |window, cx| {
            let cell = cx.new(|cx| Cell::new(shared, database, row, property, window, cx));
            let elsewhere = cx.focus_handle();
            cx.new(|_| Host { cell, elsewhere })
        });
        ui
    }

    fn fixture(
        cx: &mut TestAppContext,
        kind: PropertyKind,
        value: Option<PropertyValue>,
    ) -> (Model, Ui, crate::database::testing::One) {
        let mut made = None;
        let model = model_with(cx, |session| made = Some(one(session, kind, value)));
        let made = made.unwrap();
        let ui = mount(cx, &model, made.database, made.row, made.property);
        (model, ui, made)
    }

    #[gpui_kit::test]
    fn text_cell_sets_a_value(cx: &mut TestAppContext) {
        let (model, ui, one) = fixture(cx, PropertyKind::Text, None);
        let id = cell_id(one.row, one.property);
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some(""));
        ui.click(cx, id.clone());
        ui.input(cx, "hello");
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
        ui.press(cx, "enter");
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Text("hello".to_owned()))
        );
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("hello"));
        ui.click(cx, id.clone());
        ui.press(cx, "cmd-a");
        ui.press(cx, "backspace");
        ui.press(cx, "enter");
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
    }

    #[gpui_kit::test]
    fn escape_cancels_an_inline_edit(cx: &mut TestAppContext) {
        let (model, ui, one) = fixture(cx, PropertyKind::Text, None);
        let id = cell_id(one.row, one.property);
        ui.click(cx, id.clone());
        ui.input(cx, "draft");
        ui.press(cx, "escape");
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
        ui.click(cx, "elsewhere");
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
        ui.click(cx, id.clone());
        ui.input(cx, "kept");
        ui.press(cx, "enter");
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Text("kept".to_owned()))
        );
    }

    #[gpui_kit::test]
    fn text_cell_commits_on_blur(cx: &mut TestAppContext) {
        let (model, ui, one) = fixture(cx, PropertyKind::Text, None);
        ui.click(cx, cell_id(one.row, one.property));
        ui.input(cx, "kept");
        ui.click(cx, "elsewhere");
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Text("kept".to_owned()))
        );
    }

    #[gpui_kit::test]
    fn title_cell_edits_the_row_text_and_opens_the_page(cx: &mut TestAppContext) {
        let mut made = None;
        let model = model_with(cx, |session| {
            made = Some(one(session, PropertyKind::Text, None));
        });
        let made = made.unwrap();
        let title = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(made.database)
                .unwrap()
                .schema
                .properties[0]
                .id
        });
        let ui = mount(cx, &model, made.database, made.row, title);
        let id = cell_id(made.row, title);
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("Row"));
        ui.click(cx, id.clone());
        ui.press(cx, "cmd-a");
        ui.input(cx, "Renamed");
        ui.press(cx, "enter");
        model.read_with(cx, |model, _| {
            assert_eq!(
                model.workspace().block(made.row).unwrap().text,
                RichText::plain("Renamed")
            );
        });
        assert_eq!(ui.label(cx, id).as_deref(), Some("Renamed"));
        ui.hover(cx, cell_id(made.row, title));
        ui.click(cx, open_row_id(made.row));
        assert_eq!(ui.route(cx), Some(Route::Page(made.row)));
    }

    #[gpui_kit::test]
    fn url_cell_opens_the_link_on_a_secondary_click(cx: &mut TestAppContext) {
        let value = PropertyValue::Url("https://example.com".to_owned());
        let (_, ui, one) = fixture(cx, PropertyKind::Url, Some(value));
        let id = cell_id(one.row, one.property);
        ui.with(cx, |window, cx| {
            window.click_with_modifiers(id.clone(), Modifiers::secondary_key(), cx);
        });
        ui.settle(cx);
        assert_eq!(cx.opened_url().as_deref(), Some("https://example.com"));
    }

    #[gpui_kit::test]
    fn number_cell_refuses_invalid_input(cx: &mut TestAppContext) {
        let kind = PropertyKind::Number {
            format: NumberFormat::Euro,
        };
        let (model, ui, one) = fixture(cx, kind, Some(PropertyValue::Number(7.0)));
        let id = cell_id(one.row, one.property);
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("7,00 €"));
        ui.click(cx, id.clone());
        ui.press(cx, "cmd-a");
        ui.input(cx, "abc");
        assert_eq!(
            ui.find(cx, id.clone())
                .and_then(|cell| cell.value().map(str::to_owned)),
            Some("invalid".to_owned())
        );
        ui.press(cx, "enter");
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Number(7.0))
        );
        ui.press(cx, "cmd-a");
        ui.input(cx, "1234.5");
        assert_eq!(ui.find(cx, id.clone()).unwrap().value(), None);
        ui.press(cx, "enter");
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Number(1234.5))
        );
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("1 234,50 €"));
        ui.click(cx, id.clone());
        ui.press(cx, "cmd-a");
        ui.press(cx, "backspace");
        ui.press(cx, "enter");
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
    }

    #[gpui_kit::test]
    fn select_cell_creates_an_option_from_typed_text(cx: &mut TestAppContext) {
        let mut made = None;
        let model = model_with(cx, |session| {
            let one = one(
                session,
                PropertyKind::Select {
                    options: Vec::new(),
                },
                None,
            );
            add_option(
                session,
                one.database,
                one.property,
                "Todo",
                OptionColor::Gray,
            );
            made = Some(one);
        });
        let one = made.unwrap();
        let ui = mount(cx, &model, one.database, one.row, one.property);
        let id = cell_id(one.row, one.property);
        assert_eq!(ui.find(cx, id.clone()).unwrap().expanded(), Some(false));
        ui.click(cx, id.clone());
        assert_eq!(ui.find(cx, id.clone()).unwrap().expanded(), Some(true));
        ui.input(cx, "Doing");
        assert!(ui.exists(cx, "option-create"));
        ui.press(cx, "enter");
        let options = model.read_with(cx, |model, _| {
            match &model
                .workspace()
                .database(one.database)
                .unwrap()
                .schema
                .properties[1]
                .kind
            {
                PropertyKind::Select { options } => options.clone(),
                _ => Vec::new(),
            }
        });
        assert_eq!(options.len(), 2);
        assert_eq!(options[1].name, "Doing");
        assert_eq!(options[1].color, OptionColor::Brown);
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Select(options[1].id))
        );
        assert_eq!(ui.find(cx, id.clone()).unwrap().expanded(), Some(false));
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("Doing"));
        ui.click(cx, id.clone());
        ui.click(cx, option_id(options[0].id));
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Select(options[0].id))
        );
        ui.click(cx, id.clone());
        ui.click(cx, "option-clear");
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
        ui.click(cx, id.clone());
        ui.input(cx, "todo");
        ui.press(cx, "enter");
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Select(options[0].id))
        );
    }

    #[gpui_kit::test]
    fn multi_select_cell_toggles_options(cx: &mut TestAppContext) {
        let mut made = None;
        let mut ids = Vec::new();
        let model = model_with(cx, |session| {
            let one = one(
                session,
                PropertyKind::MultiSelect {
                    options: Vec::new(),
                },
                None,
            );
            for name in ["Red", "Blue"] {
                ids.push(add_option(
                    session,
                    one.database,
                    one.property,
                    name,
                    OptionColor::Red,
                ));
            }
            made = Some(one);
        });
        let one = made.unwrap();
        let ui = mount(cx, &model, one.database, one.row, one.property);
        let id = cell_id(one.row, one.property);
        ui.click(cx, id.clone());
        ui.click(cx, option_id(ids[0]));
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::MultiSelect(vec![ids[0]]))
        );
        assert_eq!(ui.find(cx, id.clone()).unwrap().expanded(), Some(true));
        ui.click(cx, option_id(ids[1]));
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::MultiSelect(vec![ids[0], ids[1]]))
        );
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("Red, Blue"));
        ui.click(cx, option_id(ids[0]));
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::MultiSelect(vec![ids[1]]))
        );
        ui.click(cx, option_id(ids[1]));
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
    }

    #[gpui_kit::test]
    fn date_cell_sets_a_date(cx: &mut TestAppContext) {
        let start = CalendarDate::new(2026, 9, 15).unwrap();
        let (model, ui, one) = fixture(cx, PropertyKind::Date, Some(PropertyValue::Date(start)));
        let id = cell_id(one.row, one.property);
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("2026-09-15"));
        ui.click(cx, id.clone());
        assert_eq!(ui.find(cx, id.clone()).unwrap().expanded(), Some(true));
        ui.click(cx, "calendar-2026-09-16-0-2");
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Date(CalendarDate::new(2026, 9, 16).unwrap()))
        );
        assert_eq!(ui.find(cx, id.clone()).unwrap().expanded(), Some(false));
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("2026-09-16"));
        ui.click(cx, id.clone());
        ui.click(cx, "date-clear");
        assert_eq!(value_of(&model, cx, one.row, one.property), None);
        assert_eq!(ui.label(cx, id).as_deref(), Some(""));
    }

    #[gpui_kit::test]
    fn checkbox_cell_toggles(cx: &mut TestAppContext) {
        let (model, ui, one) = fixture(cx, PropertyKind::Checkbox, None);
        let id = cell_id(one.row, one.property);
        assert_eq!(ui.find(cx, id.clone()).unwrap().checked(), Some(false));
        ui.click(cx, id.clone());
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Checkbox(true))
        );
        assert_eq!(ui.find(cx, id.clone()).unwrap().checked(), Some(true));
        ui.click(cx, id.clone());
        assert_eq!(
            value_of(&model, cx, one.row, one.property),
            Some(PropertyValue::Checkbox(false))
        );
    }

    #[gpui_kit::test]
    fn cells_follow_changes_made_elsewhere(cx: &mut TestAppContext) {
        let (model, ui, one) = fixture(cx, PropertyKind::Text, None);
        let id = cell_id(one.row, one.property);
        execute(
            &model,
            cx,
            Command::SetValue {
                row: one.row,
                property: one.property,
                value: Some(PropertyValue::Text("outside".to_owned())),
            },
        );
        ui.settle(cx);
        assert_eq!(ui.label(cx, id.clone()).as_deref(), Some("outside"));
    }
}
