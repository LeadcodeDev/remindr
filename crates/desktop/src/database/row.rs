use std::collections::HashMap;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _,
    Window, div, px,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{BlockId, DatabaseId, Parent, Property, PropertyId, PropertyKind};

use super::cell::Cell;
use super::ids::property_line_id;
use super::kinds::kind_icon;
use super::ui::icon_label;
use crate::app::{WorkspaceModel, commands};
use crate::theme::{TABLE_ROW, UI_TEXT};

const NAME_WIDTH: f32 = 140.;

pub struct RowProperties {
    model: Entity<WorkspaceModel>,
    row: BlockId,
    cells: HashMap<PropertyId, Entity<Cell>>,
    _subscription: Subscription,
}

impl RowProperties {
    pub fn new(
        model: Entity<WorkspaceModel>,
        row: BlockId,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&model, |this, _, changes: &Changes, cx| {
            if !changes.databases.is_empty() || changes.blocks.contains(&this.row) {
                cx.notify();
            }
        });
        Self {
            model,
            row,
            cells: HashMap::new(),
            _subscription: subscription,
        }
    }

    fn schema(&self, cx: &Context<Self>) -> Option<(DatabaseId, Vec<Property>)> {
        let workspace = self.model.read(cx).workspace();
        let Parent::Database(database) = workspace.block(self.row)?.parent else {
            return None;
        };
        let properties = workspace
            .database(database)?
            .schema
            .properties
            .iter()
            .filter(|property| property.kind != PropertyKind::Title)
            .cloned()
            .collect();
        Some((database, properties))
    }

    fn add_property(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((database, _)) = self.schema(cx) else {
            return;
        };
        commands::run(
            &self.model,
            Command::AddProperty {
                database,
                name: "Property".to_owned(),
                kind: PropertyKind::Text,
            },
            window,
            cx,
        );
    }
}

impl Render for RowProperties {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((database, properties)) = self.schema(cx) else {
            return v_flex().id("row-properties").test_support();
        };
        self.cells
            .retain(|id, _| properties.iter().any(|property| property.id == *id));
        let muted = cx.theme().muted_foreground;
        let lines: Vec<_> = properties
            .iter()
            .map(|property| {
                let id = property.id;
                let (model, row) = (self.model.clone(), self.row);
                let cell = self
                    .cells
                    .entry(id)
                    .or_insert_with(|| cx.new(|cx| Cell::new(model, database, row, id, window, cx)))
                    .clone();
                h_flex()
                    .id(property_line_id(id))
                    .test_support()
                    .aria_label(property.name.clone())
                    .items_center()
                    .text_size(UI_TEXT)
                    .child(
                        h_flex()
                            .w(px(NAME_WIDTH))
                            .flex_none()
                            .px_2()
                            .gap_2()
                            .items_center()
                            .text_color(muted)
                            .child(Icon::new(kind_icon(&property.kind)).size_3p5())
                            .child(div().truncate().child(property.name.clone())),
                    )
                    .child(div().flex_1().min_w_0().h(TABLE_ROW).child(cell))
            })
            .collect();
        v_flex()
            .id("row-properties")
            .test_support()
            .w_full()
            .py_2()
            .text_size(UI_TEXT)
            .children(lines)
            .child(
                div().child(
                    Button::new("row-add-property")
                        .ghost()
                        .small()
                        .text_color(muted)
                        .child(icon_label(IconName::Plus, "Add a property"))
                        .on_click(cx.listener(|this, _, window, cx| this.add_property(window, cx))),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{AppContext as _, Entity, TestAppContext};
    use remindr_core::domain::{BlockId, PropertyValue};

    use super::RowProperties;
    use crate::app::testing::{Ui, model_with};
    use crate::app::{Route, WorkspaceModel};
    use crate::database::ids::{cell_id, option_id, property_line_id};
    use crate::database::testing::{Tasks, UiExt as _, open, tasks, value_of};

    type Model = Entity<WorkspaceModel>;

    fn tasks_model(cx: &mut TestAppContext) -> (Model, Tasks) {
        let mut seeded = None;
        let model = model_with(cx, |session| seeded = Some(tasks(session)));
        (model, seeded.unwrap())
    }

    fn open_row(cx: &mut TestAppContext, model: &Model, row: BlockId) -> Ui {
        let shared = model.clone();
        let (ui, _) = open(cx, model, move |window, cx| {
            cx.new(|cx| RowProperties::new(shared, row, window, cx))
        });
        ui
    }

    #[gpui_kit::test]
    fn row_page_shows_its_properties(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let row = tasks.rows[1];
        let ui = open_row(cx, &model, row);
        assert!(ui.exists(cx, "row-properties"));
        assert_eq!(
            ui.label(cx, property_line_id(tasks.status)).as_deref(),
            Some("Status")
        );
        assert_eq!(
            ui.label(cx, property_line_id(tasks.priority)).as_deref(),
            Some("Priority")
        );
        assert!(!ui.exists(cx, property_line_id(tasks.title)));
        assert_eq!(
            ui.label(cx, cell_id(row, tasks.status)).as_deref(),
            Some("Doing")
        );
        assert_eq!(
            ui.label(cx, cell_id(row, tasks.priority)).as_deref(),
            Some("8")
        );
        ui.click(cx, cell_id(row, tasks.status));
        ui.click(cx, option_id(tasks.done));
        assert_eq!(
            ui.stored(cx, row, tasks.status),
            Some(PropertyValue::Select(tasks.done))
        );
        assert_eq!(
            ui.label(cx, cell_id(row, tasks.status)).as_deref(),
            Some("Done")
        );
    }

    #[gpui_kit::test]
    fn adding_a_property_from_a_row_page(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let row = tasks.rows[0];
        let ui = open_row(cx, &model, row);
        ui.click(cx, "row-add-property");
        let added = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .schema
                .properties
                .last()
                .unwrap()
                .clone()
        });
        assert_eq!(added.name, "Property");
        assert_eq!(added.kind, remindr_core::domain::PropertyKind::Text);
        assert_eq!(
            ui.label(cx, property_line_id(added.id)).as_deref(),
            Some("Property")
        );
        ui.click(cx, cell_id(row, added.id));
        ui.input(cx, "note");
        ui.press(cx, "enter");
        assert_eq!(
            value_of(&model, cx, row, added.id),
            Some(PropertyValue::Text("note".to_owned()))
        );
    }

    #[gpui_kit::test]
    fn a_row_page_in_the_app_shows_its_database_properties(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = Ui::open(cx, &model);
        let navigator = ui.navigator(cx);
        navigator.update(cx, |navigator, cx| {
            navigator.open(Route::Page(tasks.rows[2]), cx);
        });
        ui.settle(cx);
        assert!(ui.exists(cx, "row-properties"));
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[2], tasks.status))
                .as_deref(),
            Some("Done")
        );
    }
}
