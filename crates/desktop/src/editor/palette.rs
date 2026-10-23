use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, Hsla};
use remindr_core::domain::TextColor;

#[derive(Clone, Copy)]
pub struct Palette {
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub selection: Hsla,
    pub caret: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    red: Hsla,
    green: Hsla,
    blue: Hsla,
    yellow: Hsla,
    magenta: Hsla,
}

impl Palette {
    pub fn of(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            muted: theme.muted,
            border: theme.border,
            selection: theme.selection,
            caret: theme.caret,
            primary: theme.primary,
            primary_foreground: theme.primary_foreground,
            red: theme.red,
            green: theme.green,
            blue: theme.blue,
            yellow: theme.yellow,
            magenta: theme.magenta,
        }
    }

    #[cfg(test)]
    pub fn uniform(color: Hsla) -> Self {
        Self {
            foreground: color,
            muted_foreground: color,
            muted: color,
            border: color,
            selection: color,
            caret: color,
            primary: color,
            primary_foreground: color,
            red: color,
            green: color,
            blue: color,
            yellow: color,
            magenta: color,
        }
    }

    pub fn highlight(&self, color: TextColor) -> Option<Hsla> {
        let base = match color {
            TextColor::GrayBackground => self.muted_foreground,
            TextColor::BrownBackground
            | TextColor::OrangeBackground
            | TextColor::YellowBackground => self.yellow,
            TextColor::GreenBackground => self.green,
            TextColor::BlueBackground => self.blue,
            TextColor::PurpleBackground | TextColor::PinkBackground => self.magenta,
            TextColor::RedBackground => self.red,
            TextColor::Gray
            | TextColor::Brown
            | TextColor::Orange
            | TextColor::Yellow
            | TextColor::Green
            | TextColor::Blue
            | TextColor::Purple
            | TextColor::Pink
            | TextColor::Red => return None,
        };
        Some(base.opacity(0.3))
    }
}
