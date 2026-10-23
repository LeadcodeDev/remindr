use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ColorName, Sizable as _};
use gpui_kit::{ParentElement as _, SharedString};
use remindr_core::domain::{OptionColor, SelectOption};

#[must_use]
pub fn tag_color(color: OptionColor) -> ColorName {
    match color {
        OptionColor::Default => ColorName::Neutral,
        OptionColor::Gray => ColorName::Gray,
        OptionColor::Brown => ColorName::Amber,
        OptionColor::Orange => ColorName::Orange,
        OptionColor::Yellow => ColorName::Yellow,
        OptionColor::Green => ColorName::Green,
        OptionColor::Blue => ColorName::Blue,
        OptionColor::Purple => ColorName::Purple,
        OptionColor::Pink => ColorName::Pink,
        OptionColor::Red => ColorName::Red,
    }
}

pub fn option_tag(option: &SelectOption) -> Tag {
    Tag::color(tag_color(option.color))
        .small()
        .child(SharedString::from(option.name.clone()))
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::ColorName;
    use remindr_core::domain::OptionColor;

    use super::tag_color;

    #[test]
    fn option_colors_map_to_tag_colors() {
        assert_eq!(tag_color(OptionColor::Default), ColorName::Neutral);
        assert_eq!(tag_color(OptionColor::Green), ColorName::Green);
        assert_eq!(tag_color(OptionColor::Brown), ColorName::Amber);
    }
}
