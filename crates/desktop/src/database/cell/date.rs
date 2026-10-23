use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::calendar::{Calendar, CalendarEvent, CalendarState};
use gpui_kit::component::{Sizable as _, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, Window,
};
use remindr_core::domain::{BlockId, PropertyId, PropertyValue};

use super::{Done, write_value};
use crate::app::WorkspaceModel;
use crate::database::format::{from_naive, to_naive};

pub struct DatePanel {
    model: Entity<WorkspaceModel>,
    row: BlockId,
    property: PropertyId,
    calendar: Entity<CalendarState>,
    _subscription: Subscription,
}

impl EventEmitter<Done> for DatePanel {}

impl DatePanel {
    pub fn new(
        model: Entity<WorkspaceModel>,
        row: BlockId,
        property: PropertyId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = match model.read(cx).workspace().value(row, property) {
            Some(PropertyValue::Date(date)) => to_naive(*date),
            _ => None,
        };
        let calendar = cx.new(|cx| {
            let mut state = CalendarState::new(window, cx);
            if let Some(date) = current {
                state.set_date(date, window, cx);
            }
            state
        });
        let subscription = cx.subscribe_in(&calendar, window, Self::on_calendar_event);
        Self {
            model,
            row,
            property,
            calendar,
            _subscription: subscription,
        }
    }

    fn on_calendar_event(
        &mut self,
        _: &Entity<CalendarState>,
        event: &CalendarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let CalendarEvent::Selected(date) = event;
        let chosen = date.start().and_then(from_naive);
        if let Some(chosen) = chosen {
            write_value(
                &self.model,
                self.row,
                self.property,
                Some(PropertyValue::Date(chosen)),
                window,
                cx,
            );
            cx.emit(Done);
        }
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        write_value(&self.model, self.row, self.property, None, window, cx);
        cx.emit(Done);
    }
}

impl Render for DatePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_2().child(Calendar::new(&self.calendar)).child(
            Button::new("date-clear")
                .ghost()
                .small()
                .label("Clear")
                .on_click(cx.listener(|this, _, window, cx| this.clear(window, cx))),
        )
    }
}
