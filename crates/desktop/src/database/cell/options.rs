use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, prelude::FluentBuilder as _, px,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{
    BlockId, DatabaseId, OptionColor, OptionId, PropertyId, PropertyKind, PropertyValue,
    SelectOption,
};

use super::{Done, write_value};
use crate::app::{WorkspaceModel, commands};
use crate::database::format::next_option_color;
use crate::database::ids::option_id;
use crate::database::tags::option_tag;
use crate::theme::{RADIUS, UI_TEXT, labeled};

pub struct OptionsPicker {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    row: BlockId,
    property: PropertyId,
    multiple: bool,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<Done> for OptionsPicker {}

impl OptionsPicker {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        row: BlockId,
        property: PropertyId,
        multiple: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search or create…"));
        let subscriptions = vec![
            cx.subscribe_in(&search, window, Self::on_search_event),
            cx.subscribe(&model, |this, _, changes: &Changes, cx| {
                if changes.blocks.contains(&this.row) || changes.databases.contains(&this.database)
                {
                    cx.notify();
                }
            }),
        ];
        Self {
            model,
            database,
            row,
            property,
            multiple,
            search,
            _subscriptions: subscriptions,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
    }

    fn read<'a>(&self, cx: &'a App) -> (&'a [SelectOption], &'a [OptionId]) {
        let workspace = self.model.read(cx).workspace();
        let options = workspace
            .database(self.database)
            .and_then(|database| {
                database
                    .schema
                    .properties
                    .iter()
                    .find(|property| property.id == self.property)
            })
            .map_or(&[][..], |property| match &property.kind {
                PropertyKind::Select { options } | PropertyKind::MultiSelect { options } => {
                    options.as_slice()
                }
                _ => &[],
            });
        let chosen = match workspace.value(self.row, self.property) {
            Some(PropertyValue::Select(option)) => std::slice::from_ref(option),
            Some(PropertyValue::MultiSelect(options)) => options.as_slice(),
            _ => &[],
        };
        (options, chosen)
    }

    fn on_search_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => self.confirm(window, cx),
            InputEvent::Focus | InputEvent::Blur => {}
        }
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.search.read(cx).value().trim().to_owned();
        if typed.is_empty() {
            return;
        }
        let wanted = typed.to_lowercase();
        let (options, _) = self.read(cx);
        let existing = options
            .iter()
            .find(|option| option.name.to_lowercase() == wanted)
            .map(|option| option.id);
        let color = next_option_color(options);
        let option = match existing {
            Some(option) => option,
            None => match self.create(&typed, color, window, cx) {
                Some(option) => option,
                None => return,
            },
        };
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.pick(option, window, cx);
    }

    fn create(
        &mut self,
        name: &str,
        color: OptionColor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<OptionId> {
        commands::run(
            &self.model,
            Command::AddSelectOption {
                database: self.database,
                property: self.property,
                name: name.to_owned(),
                color,
            },
            window,
            cx,
        )?;
        let (options, _) = self.read(cx);
        options
            .iter()
            .rev()
            .find(|option| option.name == name)
            .map(|option| option.id)
    }

    fn pick(&mut self, option: OptionId, window: &mut Window, cx: &mut Context<Self>) {
        let value = if self.multiple {
            let mut next = self.read(cx).1.to_vec();
            match next.iter().position(|candidate| *candidate == option) {
                Some(position) => {
                    next.remove(position);
                }
                None => next.push(option),
            }
            (!next.is_empty()).then_some(PropertyValue::MultiSelect(next))
        } else {
            Some(PropertyValue::Select(option))
        };
        write_value(&self.model, self.row, self.property, value, window, cx);
        if self.multiple {
            cx.notify();
        } else {
            cx.emit(Done);
        }
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        write_value(&self.model, self.row, self.property, None, window, cx);
        cx.emit(Done);
    }
}

impl Render for OptionsPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (options, chosen) = self.read(cx);
        let typed = self.search.read(cx).value().trim().to_owned();
        let wanted = typed.to_lowercase();
        let exact = options
            .iter()
            .any(|option| option.name.to_lowercase() == wanted);
        let rows: Vec<_> = options
            .iter()
            .filter(|option| wanted.is_empty() || option.name.to_lowercase().contains(&wanted))
            .map(|option| {
                let id = option.id;
                let selected = chosen.contains(&id);
                h_flex()
                    .id(option_id(id))
                    .test_support()
                    .aria_label(option.name.clone())
                    .aria_selected(selected)
                    .px_2()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .rounded(RADIUS)
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().accent))
                    .on_click(cx.listener(move |this, _, window, cx| this.pick(id, window, cx)))
                    .child(option_tag(option))
                    .when(selected, |this| {
                        this.child(Icon::new(IconName::Check).size_3())
                    })
            })
            .collect();
        let create = (!typed.is_empty() && !exact).then(|| {
            let label = format!("Create \"{typed}\"");
            h_flex()
                .id("option-create")
                .test_support()
                .aria_label(label.clone())
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .rounded(RADIUS)
                .cursor_pointer()
                .hover(|style| style.bg(cx.theme().accent))
                .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx)))
                .child(Icon::new(IconName::Plus).size_3())
                .child(label)
        });
        v_flex()
            .id("option-picker")
            .test_support()
            .w(px(240.))
            .gap_2()
            .text_size(UI_TEXT)
            .child(
                Input::new(&self.search)
                    .id("option-search")
                    .small()
                    .text_size(UI_TEXT),
            )
            .child(
                v_flex()
                    .id("option-list")
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    .gap_0p5()
                    .children(rows)
                    .children(create),
            )
            .child(
                labeled(Button::new("option-clear").ghost().small(), "Clear")
                    .on_click(cx.listener(|this, _, window, cx| this.clear(window, cx))),
            )
    }
}
