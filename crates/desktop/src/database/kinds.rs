use gpui_kit::component::IconName;
use remindr_core::domain::{NumberFormat, PropertyKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindTag {
    Title,
    Text,
    Number,
    Select,
    MultiSelect,
    Date,
    Checkbox,
    Url,
}

impl KindTag {
    #[must_use]
    pub fn of(kind: &PropertyKind) -> Self {
        match kind {
            PropertyKind::Title => Self::Title,
            PropertyKind::Text => Self::Text,
            PropertyKind::Number { .. } => Self::Number,
            PropertyKind::Select { .. } => Self::Select,
            PropertyKind::MultiSelect { .. } => Self::MultiSelect,
            PropertyKind::Date => Self::Date,
            PropertyKind::Checkbox => Self::Checkbox,
            PropertyKind::Url => Self::Url,
        }
    }

    #[must_use]
    pub fn icon(self) -> IconName {
        match self {
            Self::Title => IconName::ALargeSmall,
            Self::Text => IconName::FileText,
            Self::Number => IconName::Asterisk,
            Self::Select => IconName::ChevronsUpDown,
            Self::MultiSelect => IconName::Menu,
            Self::Date => IconName::Calendar,
            Self::Checkbox => IconName::CircleCheck,
            Self::Url => IconName::Globe,
        }
    }
}

#[must_use]
pub fn kind_icon(kind: &PropertyKind) -> IconName {
    KindTag::of(kind).icon()
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
