use std::collections::{BTreeMap, HashMap};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableEvent, TableState};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, v_flex};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Subscription, TestSupportExt as _, WeakEntity, Window, div, px,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{BlockId, DatabaseId, Property, PropertyId, PropertyKind, ViewId};

use super::cell::Cell;
use super::changes::touches;
use super::header::{self, MenuState};
use super::ids::{grip_id, header_id, rename_id, row_id};
use super::kinds::{kind_choices, kind_icon};
use super::ui::icon_label;
use crate::app::navigation::page_title;
use crate::app::{WorkspaceModel, commands};
use crate::theme::{
    RADIUS_LG, TABLE_COLUMN_WIDTH, TABLE_HEADER, TABLE_ROW, TABLE_TITLE_WIDTH, UI_TEXT,
    UI_TEXT_SMALL,
};

const ADD_WIDTH: f32 = 40.;
const MAX_ROWS: usize = 20;

fn content_height(rows: usize) -> Pixels {
    TABLE_HEADER + TABLE_ROW * rows as f32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    View(ViewId),
    Database,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Height {
    Rows,
    Fill,
}

fn row_group(row: BlockId) -> SharedString {
    SharedString::from(format!("row-{}", row.as_uuid()))
}

#[derive(Clone)]
struct RowDrag {
    row: BlockId,
    title: SharedString,
}

struct RowPreview {
    title: SharedString,
}

impl Render for RowPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .rounded(RADIUS_LG)
            .text_size(UI_TEXT)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .opacity(0.9)
            .shadow_md()
            .child(self.title.clone())
    }
}

pub struct Grid {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    scope: Scope,
    owner: WeakEntity<TableView>,
    columns: Vec<Property>,
    rows: Vec<BlockId>,
    widths: BTreeMap<PropertyId, u32>,
    cells: HashMap<(BlockId, PropertyId), Entity<Cell>>,
    menus: HashMap<PropertyId, (Entity<MenuState>, Subscription)>,
    renaming: Option<(PropertyId, Entity<InputState>)>,
    reorderable: bool,
}

impl Grid {
    fn width_of(&self, property: &Property) -> u32 {
        self.widths
            .get(&property.id)
            .copied()
            .unwrap_or(if property.kind == PropertyKind::Title {
                TABLE_TITLE_WIDTH
            } else {
                TABLE_COLUMN_WIDTH
            })
    }

    fn load(&mut self, cx: &App) -> bool {
        let workspace = self.model.read(cx).workspace();
        let before = (self.columns.clone(), self.widths.clone());
        let database = workspace.database(self.database);
        let (columns, rows) = match (self.scope, database) {
            (Scope::View(id), Some(database)) => match workspace.view(id) {
                Some(view) => {
                    self.widths.clone_from(&view.column_widths);
                    let columns = view
                        .visible
                        .iter()
                        .filter_map(|visible| {
                            database
                                .schema
                                .properties
                                .iter()
                                .find(|property| property.id == *visible)
                        })
                        .cloned()
                        .collect();
                    (columns, workspace.query(id).unwrap_or_default())
                }
                None => (Vec::new(), Vec::new()),
            },
            (Scope::Database, Some(database)) => (
                database.schema.properties.clone(),
                workspace.rows(self.database).map(|row| row.id).collect(),
            ),
            (_, None) => (Vec::new(), Vec::new()),
        };
        self.reorderable = match self.scope {
            Scope::View(id) => workspace.view(id).is_some_and(|view| view.sorts.is_empty()),
            Scope::Database => false,
        };
        self.columns = columns;
        self.rows = rows;
        let live: Vec<(BlockId, PropertyId)> = self
            .rows
            .iter()
            .flat_map(|row| self.columns.iter().map(move |property| (*row, property.id)))
            .collect();
        self.cells.retain(|key, _| live.contains(key));
        let shown: Vec<PropertyId> = self.columns.iter().map(|column| column.id).collect();
        self.menus.retain(|id, _| shown.contains(id));
        before != (self.columns.clone(), self.widths.clone())
    }

    fn add_button(&self) -> AnyElement {
        let owner = self.owner.clone();
        Button::new("column-add")
            .ghost()
            .xsmall()
            .child(Icon::new(IconName::Plus).size_3())
            .on_click(move |_, window, cx| {
                owner
                    .update(cx, |view, cx| view.add_property(window, cx))
                    .ok();
            })
            .into_any_element()
    }
    fn header(&mut self, property: &Property, cx: &mut Context<TableState<Self>>) -> AnyElement {
        let id = property.id;
        let (state, _) = self.menus.entry(id).or_insert_with(|| {
            let state = cx.new(|_| MenuState::default());
            let subscription = cx.observe(&state, |_, _, cx| cx.notify());
            (state, subscription)
        });
        let state = state.clone();
        let open = state.read(cx).is_open();
        let owner = self.owner.clone();
        let title = property.kind == PropertyKind::Title;
        let hideable = matches!(self.scope, Scope::View(_));
        let current = property.kind.clone();
        let children = vec![
            Icon::new(kind_icon(&property.kind))
                .size_3p5()
                .into_any_element(),
            div()
                .truncate()
                .child(property.name.clone())
                .into_any_element(),
        ];
        header::cell(
            header_id(id),
            property.name.clone().into(),
            &state,
            open,
            children,
            move |menu, window, cx| {
                let renamer = owner.clone();
                let retyper = owner.clone();
                let hider = owner.clone();
                let remover = owner.clone();
                let current = current.clone();
                let menu = menu
                    .item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                        renamer
                            .update(cx, |view, cx| view.begin_rename(id, window, cx))
                            .ok();
                    }))
                    .submenu("Type", window, cx, move |menu, _, _| {
                        kind_choices()
                            .into_iter()
                            .fold(menu, |menu, (label, kind)| {
                                let owner = retyper.clone();
                                let chosen = kind.clone();
                                let same = std::mem::discriminant(&kind)
                                    == std::mem::discriminant(&current);
                                menu.item(
                                    PopupMenuItem::new(label)
                                        .checked(same)
                                        .disabled(kind == PropertyKind::Title)
                                        .on_click(move |_, window, cx| {
                                            let kind = chosen.clone();
                                            owner
                                                .update(cx, |view, cx| {
                                                    view.change_kind(id, kind, window, cx);
                                                })
                                                .ok();
                                        }),
                                )
                            })
                    });
                let menu = if hideable {
                    menu.item(PopupMenuItem::new("Hide").on_click(move |_, window, cx| {
                        hider.update(cx, |view, cx| view.hide(id, window, cx)).ok();
                    }))
                } else {
                    menu
                };
                if title {
                    menu
                } else {
                    menu.separator().item(PopupMenuItem::new("Delete").on_click(
                        move |_, window, cx| {
                            remover
                                .update(cx, |view, cx| view.delete(id, window, cx))
                                .ok();
                        },
                    ))
                }
            },
        )
    }
}

impl TableDelegate for Grid {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len() + 1
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match self.columns.get(col_ix) {
            Some(property) => Column::new(property.id.as_uuid().to_string(), property.name.clone())
                .width(px(self.width_of(property) as f32))
                .min_width(px(60.))
                .p_0(),
            None => Column::new("add", "")
                .width(px(ADD_WIDTH))
                .resizable(false)
                .movable(false)
                .selectable(false)
                .p_0(),
        }
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(property) = self.columns.get(col_ix).cloned() else {
            return self.add_button();
        };
        match &self.renaming {
            Some((id, input)) if *id == property.id => {
                let owner = self.owner.clone();
                div()
                    .w_full()
                    .px_1()
                    .on_action(move |_: &Escape, _, cx| {
                        owner.update(cx, |view, cx| view.cancel_rename(cx)).ok();
                    })
                    .child(Input::new(input).id(rename_id(property.id)).small())
                    .into_any_element()
            }
            _ => self.header(&property, cx),
        }
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let Some(row) = self.rows.get(row_ix).copied() else {
            return div().id(("row", row_ix));
        };
        let line = div().id(row_id(row)).group(row_group(row));
        if !self.reorderable {
            return line;
        }
        let owner = self.owner.clone();
        let accent = cx.theme().primary;
        line.drag_over::<RowDrag>(move |style, _, _, _| style.border_t_2().border_color(accent))
            .on_drop(move |dragged: &RowDrag, window, cx| {
                let moved = dragged.row;
                owner
                    .update(cx, |view, cx| view.move_row(moved, Some(row), window, cx))
                    .ok();
            })
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let (Some(row), Some(property)) = (self.rows.get(row_ix), self.columns.get(col_ix)) else {
            return div().into_any_element();
        };
        let (row, property, title) = (*row, property.id, property.kind == PropertyKind::Title);
        let (model, database) = (self.model.clone(), self.database);
        let cell = self
            .cells
            .entry((row, property))
            .or_insert_with(|| cx.new(|cx| Cell::new(model, database, row, property, window, cx)))
            .clone();
        if !(self.reorderable && title) {
            return cell.into_any_element();
        }
        let name = self
            .model
            .read(cx)
            .workspace()
            .block(row)
            .map(page_title)
            .unwrap_or_default();
        let group = row_group(row);
        let drag = RowDrag {
            row,
            title: name.into(),
        };
        div()
            .relative()
            .size_full()
            .child(cell)
            .child(
                div()
                    .id(grip_id(row))
                    .test_support()
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(UI_TEXT_SMALL)
                    .text_color(cx.theme().muted_foreground)
                    .bg(cx.theme().background)
                    .cursor_grab()
                    .opacity(0.)
                    .group_hover(group, |style| style.opacity(1.))
                    .on_drag(drag, |dragged, _, _, cx| {
                        let title = dragged.title.clone();
                        cx.new(|_| RowPreview { title })
                    })
                    .child("⋮⋮"),
            )
            .into_any_element()
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
    }
}

struct Renaming {
    _subscription: Subscription,
}

pub struct TableView {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    scope: Scope,
    height: Height,
    state: Entity<TableState<Grid>>,
    renaming: Option<Renaming>,
    _subscriptions: Vec<Subscription>,
}

impl TableView {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        scope: Scope,
        height: Height,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut grid = Grid {
            model: model.clone(),
            database,
            scope,
            owner: cx.weak_entity(),
            columns: Vec::new(),
            rows: Vec::new(),
            widths: BTreeMap::new(),
            cells: HashMap::new(),
            menus: HashMap::new(),
            renaming: None,
            reorderable: false,
        };
        grid.load(cx);
        let state = cx.new(|cx| {
            TableState::new(grid, window, cx)
                .col_resizable(true)
                .col_movable(true)
                .row_selectable(false)
                .col_selectable(false)
                .sortable(false)
        });
        let subscriptions = vec![
            cx.subscribe_in(&model, window, Self::on_changes),
            cx.subscribe_in(&state, window, Self::on_table_event),
        ];
        Self {
            model,
            database,
            scope,
            height,
            state,
            renaming: None,
            _subscriptions: subscriptions,
        }
    }

    fn on_changes(
        &mut self,
        model: &Entity<WorkspaceModel>,
        changes: &Changes,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if touches(model.read(cx).workspace(), changes, self.database, None) {
            self.reload(cx);
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if state.delegate_mut().load(cx) {
                state.refresh(cx);
            }
            cx.notify();
        });
        cx.notify();
    }

    fn realign(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.refresh(cx);
            cx.notify();
        });
    }

    fn on_table_event(
        &mut self,
        _: &Entity<TableState<Grid>>,
        event: &TableEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TableEvent::ColumnWidthsChanged(widths) => {
                let widths: Vec<u32> = widths
                    .iter()
                    .map(|width| f32::from(*width).round().max(0.) as u32)
                    .collect();
                self.store_widths(&widths, window, cx);
            }
            TableEvent::MoveColumn(from, to) => self.move_column(*from, *to, window, cx),
            _ => {}
        }
    }

    fn store_widths(&mut self, widths: &[u32], window: &mut Window, cx: &mut Context<Self>) {
        let changed: Vec<(PropertyId, u32)> = {
            let grid = self.state.read(cx).delegate();
            grid.columns
                .iter()
                .zip(widths)
                .filter(|(property, width)| grid.width_of(property) != **width)
                .map(|(property, width)| (property.id, *width))
                .collect()
        };
        match self.scope {
            Scope::View(view) => {
                for (property, width) in changed {
                    commands::run(
                        &self.model,
                        Command::SetColumnWidth {
                            view,
                            property,
                            width,
                        },
                        window,
                        cx,
                    );
                }
            }
            Scope::Database => self.state.update(cx, |state, _| {
                for (property, width) in changed {
                    state.delegate_mut().widths.insert(property, width);
                }
            }),
        }
    }

    fn move_column(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut order: Vec<PropertyId> = self
            .state
            .read(cx)
            .delegate()
            .columns
            .iter()
            .map(|property| property.id)
            .collect();
        if from < order.len() {
            let moved = order.remove(from);
            let target = to.min(order.len());
            order.insert(target, moved);
            match self.scope {
                Scope::View(view) => {
                    commands::run(
                        &self.model,
                        Command::SetVisibleProperties {
                            view,
                            properties: order,
                        },
                        window,
                        cx,
                    );
                }
                Scope::Database => {
                    commands::run(
                        &self.model,
                        Command::MoveProperty {
                            database: self.database,
                            property: moved,
                            index: target,
                        },
                        window,
                        cx,
                    );
                }
            }
        }
        self.realign(cx);
    }

    fn add_property(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let added = commands::run(
            &self.model,
            Command::AddProperty {
                database: self.database,
                name: "Property".to_owned(),
                kind: PropertyKind::Text,
            },
            window,
            cx,
        );
        if added.is_none() {
            return;
        }
        let property = self
            .model
            .read(cx)
            .workspace()
            .database(self.database)
            .and_then(|database| database.schema.properties.last())
            .map(|property| property.id);
        if let Some(property) = property {
            self.begin_rename(property, window, cx);
        }
    }

    fn begin_rename(&mut self, property: PropertyId, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .state
            .read(cx)
            .delegate()
            .columns
            .iter()
            .find(|column| column.id == property)
            .map(|column| column.name.clone())
            .or_else(|| {
                self.model
                    .read(cx)
                    .workspace()
                    .database(self.database)
                    .and_then(|database| {
                        database
                            .schema
                            .properties
                            .iter()
                            .find(|candidate| candidate.id == property)
                            .map(|candidate| candidate.name.clone())
                    })
            })
            .unwrap_or_default();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let subscription =
            cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
                this.on_rename_event(property, input, event, window, cx);
            });
        input.update(cx, |input, cx| input.focus(window, cx));
        self.renaming = Some(Renaming {
            _subscription: subscription,
        });
        self.state.update(cx, |state, cx| {
            state.delegate_mut().renaming = Some((property, input));
            cx.notify();
        });
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.renaming = None;
        self.state.update(cx, |state, cx| {
            state.delegate_mut().renaming = None;
            cx.notify();
        });
        cx.notify();
    }

    fn on_rename_event(
        &mut self,
        property: PropertyId,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
            return;
        }
        let name = input.read(cx).value().trim().to_owned();
        self.renaming = None;
        self.state.update(cx, |state, cx| {
            state.delegate_mut().renaming = None;
            cx.notify();
        });
        let current = self
            .model
            .read(cx)
            .workspace()
            .database(self.database)
            .and_then(|database| {
                database
                    .schema
                    .properties
                    .iter()
                    .find(|candidate| candidate.id == property)
                    .map(|candidate| candidate.name.clone())
            });
        if !name.is_empty() && current.is_some_and(|current| current != name) {
            commands::run(
                &self.model,
                Command::RenameProperty {
                    database: self.database,
                    property,
                    name,
                },
                window,
                cx,
            );
        }
        cx.notify();
    }

    fn change_kind(
        &mut self,
        property: PropertyId,
        kind: PropertyKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .state
            .read(cx)
            .delegate()
            .columns
            .iter()
            .find(|column| column.id == property)
            .map(|column| column.kind.clone());
        let same = current.is_some_and(|current| {
            std::mem::discriminant(&current) == std::mem::discriminant(&kind)
        });
        if same {
            return;
        }
        commands::run(
            &self.model,
            Command::ChangePropertyKind {
                database: self.database,
                property,
                kind,
            },
            window,
            cx,
        );
    }

    fn hide(&mut self, property: PropertyId, window: &mut Window, cx: &mut Context<Self>) {
        let Scope::View(view) = self.scope else {
            return;
        };
        let properties: Vec<PropertyId> = self
            .state
            .read(cx)
            .delegate()
            .columns
            .iter()
            .map(|column| column.id)
            .filter(|id| *id != property)
            .collect();
        commands::run(
            &self.model,
            Command::SetVisibleProperties { view, properties },
            window,
            cx,
        );
    }

    fn delete(&mut self, property: PropertyId, window: &mut Window, cx: &mut Context<Self>) {
        commands::run(
            &self.model,
            Command::RemoveProperty {
                database: self.database,
                property,
            },
            window,
            cx,
        );
    }

    fn move_row(
        &mut self,
        row: BlockId,
        before: Option<BlockId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Scope::View(view) = self.scope else {
            return;
        };
        if before == Some(row) {
            return;
        }
        commands::run(
            &self.model,
            Command::MoveRow { view, row, before },
            window,
            cx,
        );
    }

    fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        commands::run(
            &self.model,
            Command::AddRow {
                database: self.database,
                values: Vec::new(),
            },
            window,
            cx,
        );
    }
}

impl Render for TableView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.state.read(cx).delegate().rows.len();
        let table = DataTable::new(&self.state)
            .bordered(false)
            .small()
            .header_row_height(TABLE_HEADER);
        let new_row = Button::new("table-new-row")
            .ghost()
            .small()
            .w_full()
            .text_color(cx.theme().muted_foreground)
            .child(icon_label(IconName::Plus, "New").w_full())
            .on_click(cx.listener(|this, _, window, cx| this.add_row(window, cx)));
        let accent = cx.theme().primary;
        let footer = div()
            .w_full()
            .drag_over::<RowDrag>(move |style, _, _, _| style.border_t_2().border_color(accent))
            .on_drop(cx.listener(|this, dragged: &RowDrag, window, cx| {
                this.move_row(dragged.row, None, window, cx);
            }))
            .child(new_row);
        match self.height {
            Height::Rows => v_flex()
                .id("table-view")
                .w_full()
                .text_size(UI_TEXT)
                .child(
                    div()
                        .w_full()
                        .h(content_height(rows.min(MAX_ROWS)))
                        .child(table),
                )
                .child(footer),
            Height::Fill => v_flex()
                .id("table-view")
                .size_full()
                .text_size(UI_TEXT)
                .child(
                    div()
                        .w_full()
                        .h(content_height(rows))
                        .min_h_0()
                        .child(table),
                )
                .child(footer.flex_none()),
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{ElementId, Entity, ScrollDelta, TestAppContext, point, px};
    use remindr_core::application::Command;
    use remindr_core::domain::{BlockId, PropertyKind, PropertyValue, Sort, SortDirection, ViewId};

    use crate::app::WorkspaceModel;
    use crate::app::testing::{execute, model_with};
    use crate::database::ids::{
        cell_id, cell_text_id, grip_id, header_id, open_row_id, option_id, rename_id, row_id,
    };
    use crate::database::testing::{
        Tasks, UiExt as _, open_block, open_block_in_width, tasks, titles, value_of,
    };

    type Model = Entity<WorkspaceModel>;

    fn tasks_model(cx: &mut TestAppContext) -> (Model, Tasks) {
        let mut seeded = None;
        let model = model_with(cx, |session| seeded = Some(tasks(session)));
        (model, seeded.unwrap())
    }

    fn row_top(ui: &crate::app::testing::Ui, cx: &mut TestAppContext, row: BlockId) -> f32 {
        let found = ui.find(cx, row_id(row)).unwrap();
        found.bounds().origin.y.into()
    }

    fn visible_columns(model: &Model, cx: &mut TestAppContext, view: ViewId) -> usize {
        model.read_with(cx, |model, _| {
            model.workspace().view(view).unwrap().visible.len()
        })
    }

    #[gpui_kit::test]
    fn table_lists_rows_in_query_order(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        assert_eq!(
            titles(&model, cx, tasks.view),
            ["Write spec", "Build core", "Ship UI", "Celebrate"]
        );
        let tops: Vec<f32> = tasks
            .rows
            .iter()
            .map(|row| row_top(&ui, cx, *row))
            .collect();
        assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[1], tasks.title)).as_deref(),
            Some("Build core")
        );
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[1], tasks.status))
                .as_deref(),
            Some("Doing")
        );
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[1], tasks.priority))
                .as_deref(),
            Some("8")
        );
        assert_eq!(
            ui.label(cx, header_id(tasks.status)).as_deref(),
            Some("Status")
        );
        assert_eq!(
            ui.label(cx, header_id(tasks.title)).as_deref(),
            Some("Name")
        );
    }

    #[gpui_kit::test]
    fn columns_render_at_their_default_widths(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        let width = |ui: &crate::app::testing::Ui, cx: &mut TestAppContext, id: ElementId| {
            ui.find(cx, id).unwrap().bounds().size.width
        };
        let column = |index: u64| ElementId::NamedInteger("col-header".into(), index);
        assert_eq!(width(&ui, cx, column(0)), px(280.));
        assert_eq!(width(&ui, cx, column(1)), px(180.));
        assert_eq!(width(&ui, cx, column(2)), px(180.));
    }

    #[gpui_kit::test]
    fn the_title_cell_uses_the_whole_column_width(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        execute(
            &model,
            cx,
            Command::SetColumnWidth {
                view: tasks.view,
                property: tasks.title,
                width: 120,
            },
        );
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        let column = ui
            .find(cx, ElementId::NamedInteger("col-header".into(), 0))
            .unwrap()
            .bounds();
        assert_eq!(column.size.width, px(120.));
        let cell = ui
            .find(cx, cell_id(tasks.rows[0], tasks.title))
            .unwrap()
            .bounds();
        assert_eq!(cell.size.width, px(120.));
        let text = ui
            .find(cx, cell_text_id(tasks.rows[0], tasks.title))
            .unwrap()
            .bounds();
        assert!(text.size.width >= cell.size.width - px(16.), "{text:?}");
        let open = ui.find(cx, open_row_id(tasks.rows[0])).unwrap().bounds();
        assert!(open.right() <= cell.right());
        assert!(open.left() > cell.left());
    }

    #[gpui_kit::test]
    fn a_table_wider_than_its_block_scrolls_horizontally(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block_in_width(cx, &model, tasks.block, tasks.database, px(500.));
        let last = header_id(tasks.priority);
        let before = ui.find(cx, last.clone()).unwrap().bounds();
        assert!(before.right() > px(500.), "{before:?}");
        ui.with(cx, |window, cx| {
            window.scroll("table", ScrollDelta::Pixels(point(px(-160.), px(0.))), cx);
        });
        ui.settle(cx);
        let after = ui.find(cx, last).unwrap().bounds();
        assert!(
            after.left() < before.left() - px(100.),
            "{before:?} {after:?}"
        );
        assert!(after.right() <= px(500.) + px(1.), "{after:?}");
    }

    #[gpui_kit::test]
    fn dragging_a_row_handle_reorders_the_rows(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.hover(cx, row_id(tasks.rows[3]));
        ui.drag_to(cx, grip_id(tasks.rows[3]), row_id(tasks.rows[0]));
        assert_eq!(
            titles(&model, cx, tasks.view),
            ["Celebrate", "Write spec", "Build core", "Ship UI"]
        );
        let tops: Vec<f32> = [3usize, 0, 1, 2]
            .iter()
            .map(|index| {
                let found = ui.find(cx, row_id(tasks.rows[*index])).unwrap();
                found.bounds().origin.y.into()
            })
            .collect();
        assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
        ui.hover(cx, row_id(tasks.rows[3]));
        ui.drag_to(cx, grip_id(tasks.rows[3]), "table-new-row");
        assert_eq!(
            titles(&model, cx, tasks.view),
            ["Write spec", "Build core", "Ship UI", "Celebrate"]
        );
    }

    #[gpui_kit::test]
    fn the_row_handle_is_hidden_while_sorts_are_active(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        assert!(ui.exists(cx, grip_id(tasks.rows[0])));
        execute(
            &model,
            cx,
            Command::SetSorts {
                view: tasks.view,
                sorts: vec![Sort {
                    property: tasks.priority,
                    direction: SortDirection::Ascending,
                }],
            },
        );
        ui.settle(cx);
        assert!(!ui.exists(cx, grip_id(tasks.rows[0])));
        assert!(ui.exists(cx, row_id(tasks.rows[0])));
    }

    #[gpui_kit::test]
    fn cells_edit_in_place_inside_the_table(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, cell_id(tasks.rows[0], tasks.status));
        ui.click(cx, option_id(tasks.done));
        assert_eq!(
            value_of(&model, cx, tasks.rows[0], tasks.status),
            Some(PropertyValue::Select(tasks.done))
        );
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[0], tasks.status))
                .as_deref(),
            Some("Done")
        );
        ui.click(cx, cell_id(tasks.rows[1], tasks.priority));
        ui.press(cx, "cmd-a");
        ui.input(cx, "42");
        ui.press(cx, "enter");
        assert_eq!(
            value_of(&model, cx, tasks.rows[1], tasks.priority),
            Some(PropertyValue::Number(42.0))
        );
        ui.click(cx, cell_id(tasks.rows[2], tasks.title));
        ui.press(cx, "cmd-a");
        ui.input(cx, "Shipped UI");
        ui.press(cx, "enter");
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[2], tasks.title)).as_deref(),
            Some("Shipped UI")
        );
        assert_eq!(
            titles(&model, cx, tasks.view),
            ["Write spec", "Build core", "Shipped UI", "Celebrate"]
        );
    }

    #[gpui_kit::test]
    fn undo_restores_table_cells(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, cell_id(tasks.rows[1], tasks.priority));
        ui.press(cx, "cmd-a");
        ui.input(cx, "42");
        ui.press(cx, "enter");
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[1], tasks.priority))
                .as_deref(),
            Some("42")
        );
        model.update(cx, |model, cx| {
            model.undo(cx).unwrap();
        });
        ui.settle(cx);
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[1], tasks.priority))
                .as_deref(),
            Some("8")
        );
    }

    #[gpui_kit::test]
    fn the_new_row_button_adds_a_row_at_the_end(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "table-new-row");
        let rows = model.read_with(cx, |model, _| model.workspace().query(tasks.view).unwrap());
        assert_eq!(rows.len(), 5);
        assert!(ui.exists(cx, row_id(rows[4])));
        ui.click(cx, "toolbar-new");
        let rows = model.read_with(cx, |model, _| model.workspace().query(tasks.view).unwrap());
        assert_eq!(rows.len(), 6);
    }

    #[gpui_kit::test]
    fn resizing_a_column_stores_its_width(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        let header = ui
            .find(cx, ElementId::NamedInteger("col-header".into(), 1))
            .unwrap()
            .bounds();
        let edge = header.origin.x + header.size.width;
        let middle = header.origin.y + header.size.height / 2.;
        ui.with(cx, |window, cx| {
            window.drag(
                point(edge - px(1.), middle),
                point(edge + px(80.), middle),
                cx,
            );
        });
        ui.settle(cx);
        let widths = model.read_with(cx, |model, _| {
            model
                .workspace()
                .view(tasks.view)
                .unwrap()
                .column_widths
                .clone()
        });
        let stored = widths.get(&tasks.status).copied().unwrap();
        assert!(stored > 200, "{widths:?}");
        assert_eq!(widths.get(&tasks.title), None);
    }

    #[gpui_kit::test]
    fn dragging_a_header_moves_the_column(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.drag_to(cx, header_id(tasks.priority), header_id(tasks.title));
        let visible = model.read_with(cx, |model, _| {
            model.workspace().view(tasks.view).unwrap().visible.clone()
        });
        assert_eq!(visible, vec![tasks.priority, tasks.title, tasks.status]);
        let priority = ui.find(cx, header_id(tasks.priority)).unwrap().bounds();
        let title = ui.find(cx, header_id(tasks.title)).unwrap().bounds();
        assert!(priority.origin.x < title.origin.x);
    }

    #[gpui_kit::test]
    fn hiding_a_property_removes_its_column(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        assert!(ui.exists(cx, header_id(tasks.status)));
        let before = visible_columns(&model, cx, tasks.view);
        ui.click(cx, header_id(tasks.status));
        ui.pick_item(cx, "popup-menu", 2usize);
        assert!(!ui.exists(cx, header_id(tasks.status)));
        assert_eq!(visible_columns(&model, cx, tasks.view), before - 1);
        assert!(!ui.exists(cx, cell_id(tasks.rows[0], tasks.status)));
        assert!(ui.exists(cx, header_id(tasks.priority)));
    }

    #[gpui_kit::test]
    fn adding_a_property_adds_a_column(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        let before = visible_columns(&model, cx, tasks.view);
        ui.click(cx, "column-add");
        let (added, name) = model.read_with(cx, |model, _| {
            let property = model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .schema
                .properties
                .last()
                .unwrap()
                .clone();
            (property.id, property.name)
        });
        assert_eq!(name, "Property");
        assert_eq!(visible_columns(&model, cx, tasks.view), before + 1);
        assert!(ui.exists(cx, header_id(added)) || ui.exists(cx, rename_id(added)));
        assert!(ui.exists(cx, rename_id(added)));
        ui.press(cx, "cmd-a");
        ui.input(cx, "Notes");
        ui.press(cx, "enter");
        let renamed = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .schema
                .properties
                .last()
                .unwrap()
                .name
                .clone()
        });
        assert_eq!(renamed, "Notes");
        assert_eq!(ui.label(cx, header_id(added)).as_deref(), Some("Notes"));
        assert!(!ui.exists(cx, rename_id(added)));
    }

    #[gpui_kit::test]
    fn escape_cancels_a_header_rename(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, header_id(tasks.priority));
        ui.pick_item(cx, "popup-menu", 0usize);
        assert!(ui.exists(cx, rename_id(tasks.priority)));
        ui.press(cx, "cmd-a");
        ui.input(cx, "Points");
        ui.press(cx, "escape");
        assert!(!ui.exists(cx, rename_id(tasks.priority)));
        assert_eq!(
            ui.label(cx, header_id(tasks.priority)).as_deref(),
            Some("Priority")
        );
    }

    #[gpui_kit::test]
    fn the_header_menu_renames_retypes_and_deletes(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, header_id(tasks.priority));
        ui.pick_item(cx, "popup-menu", 0usize);
        ui.press(cx, "cmd-a");
        ui.input(cx, "Points");
        ui.press(cx, "enter");
        let name = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .schema
                .properties[2]
                .name
                .clone()
        });
        assert_eq!(name, "Points");

        ui.click(cx, header_id(tasks.priority));
        ui.with(cx, |window, cx| {
            window.within("popup-menu").hover(1usize, cx)
        });
        ui.settle(cx);
        ui.with(cx, |window, cx| window.within("submenu").click(1usize, cx));
        ui.settle(cx);
        let kind = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .schema
                .properties[2]
                .kind
                .clone()
        });
        assert_eq!(kind, PropertyKind::Text);
        assert_eq!(
            ui.label(cx, cell_id(tasks.rows[1], tasks.priority))
                .as_deref(),
            Some("8")
        );

        ui.click(cx, header_id(tasks.priority));
        ui.pick_item(cx, "popup-menu", 4usize);
        assert!(!ui.exists(cx, header_id(tasks.priority)));
        let count = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .schema
                .properties
                .len()
        });
        assert_eq!(count, 2);
    }

    #[gpui_kit::test]
    fn the_title_column_cannot_be_deleted_or_retyped(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, header_id(tasks.title));
        ui.with(cx, |window, _| {
            let menu = window.within("popup-menu");
            assert_eq!(menu.find(0usize).label(), Some("Rename"));
            assert_eq!(menu.find(1usize).label(), Some("Type"));
            assert_eq!(menu.find(2usize).label(), Some("Hide"));
            assert!(menu.try_find(3usize).is_none());
        });
    }
}
