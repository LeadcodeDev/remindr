use gpui_kit::component::IconName;
use remindr_core::domain::{NumberFormat, PropertyKind};

#[must_use]
pub fn kind_icon(kind: &PropertyKind) -> IconName {
    match kind {
        PropertyKind::Title => IconName::ALargeSmall,
        PropertyKind::Text => IconName::FileText,
        PropertyKind::Number { .. } => IconName::Asterisk,
        PropertyKind::Select { .. } => IconName::ChevronsUpDown,
        PropertyKind::MultiSelect { .. } => IconName::Menu,
        PropertyKind::Date => IconName::Calendar,
        PropertyKind::Checkbox => IconName::CircleCheck,
        PropertyKind::Url => IconName::Globe,
    }
}

pub fn kind_choices() -> [(&'static str, PropertyKind); 8] {
    [
        ("Title", PropertyKind::Title),
        ("Text", PropertyKind::Text),
        (
            "Number",
            PropertyKind::Number {
                format: NumberFormat::Plain,
            },
        ),
        (
            "Select",
            PropertyKind::Select {
                options: Vec::new(),
            },
        ),
        (
            "Multi-select",
            PropertyKind::MultiSelect {
                options: Vec::new(),
            },
        ),
        ("Date", PropertyKind::Date),
        ("Checkbox", PropertyKind::Checkbox),
        ("URL", PropertyKind::Url),
    ]
}
