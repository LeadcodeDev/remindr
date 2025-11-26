use std::{fs::read_to_string, vec};

use gpui::*;
use gpui_component::{
    Collapsible, Icon, IconName,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
};
use gpui_router::use_navigate;
use serde_json::{Value, from_str};

use crate::states::document_state::DocumentState;

#[derive(IntoElement)]
pub struct AppSidebar;

impl RenderOnce for AppSidebar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let links = vec![
            SidebarMenuItem::new("Chercher").icon(IconName::Search),
            SidebarMenuItem::new("Accueil")
                .icon(Icon::default().path("icons/house.svg"))
                .on_click(|_, _, cx| {
                    let mut navigate = use_navigate(cx);
                    navigate(SharedString::new("/"));
                }),
            SidebarMenuItem::new("Boite de réception").icon(IconName::Inbox),
        ];

        let documents = vec![
            SidebarMenuItem::new("Document 1")
                .icon(IconName::File)
                .on_click(|_, window, cx| {
                    cx.update_global::<DocumentState, _>(|state, cx| {
                        let path = "./artifacts/demo.json";
                        let content = read_to_string(path).unwrap();
                        let nodes = from_str::<Vec<Value>>(&content).unwrap();

                        state.add_document(path, nodes, window, cx);
                    });

                    let mut navigate = use_navigate(cx);
                    navigate(SharedString::new("/documents"));
                })
                .active(true),
            SidebarMenuItem::new("Document 2").icon(IconName::File),
            SidebarMenuItem::new("Document 3").icon(IconName::File),
        ];

        Sidebar::left()
            .width(Pixels::from(240.0))
            .header(SidebarHeader::new())
            .child(
                SidebarGroup::new("Label")
                    .child(SidebarMenu::new().children(links))
                    .collapsed(true),
            )
            .child(
                SidebarGroup::new("Documents")
                    .child(SidebarMenu::new().children(documents).collapsed(false)),
            )
            .footer(SidebarFooter::new().child("Footer"))
    }
}
