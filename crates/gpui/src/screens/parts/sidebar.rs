use std::vec;

use gpui::*;
use gpui_component::{
    Collapsible, Icon, IconName,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
};

pub struct MenuSidebar;

impl Render for MenuSidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let links = vec![
            SidebarMenuItem::new("Chercher").icon(IconName::Search),
            SidebarMenuItem::new("Accueil").icon(Icon::default().path("icons/house.svg")),
            SidebarMenuItem::new("Boite de réception").icon(IconName::Inbox),
        ];

        let documents = vec![
            SidebarMenuItem::new("Document 1")
                .icon(IconName::File)
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
