use remindr_core::domain::{
    CalendarDate, Condition, DateCondition, MultiSelectCondition, NumberCondition, OptionId,
    PropertyKind, SelectCondition, SelectOption, TextCondition,
};

use super::format::today;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Contains,
    DoesNotContain,
    Is,
    IsNot,
    IsEmpty,
    IsNotEmpty,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Before,
    After,
    Checked,
    Unchecked,
}

impl Operator {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::DoesNotContain => "does not contain",
            Self::Is => "is",
            Self::IsNot => "is not",
            Self::IsEmpty => "is empty",
            Self::IsNotEmpty => "is not empty",
            Self::Eq => "=",
            Self::Ne => "≠",
            Self::Lt => "<",
            Self::Le => "≤",
            Self::Gt => ">",
            Self::Ge => "≥",
            Self::Before => "before",
            Self::After => "after",
            Self::Checked => "is checked",
            Self::Unchecked => "is not checked",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operand<'a> {
    None,
    Text(&'a str),
    Number(f64),
    Option(OptionId),
    Date(CalendarDate),
}

#[must_use]
pub fn operators(kind: &PropertyKind) -> &'static [Operator] {
    match kind {
        PropertyKind::Title | PropertyKind::Text | PropertyKind::Url => &[
            Operator::Contains,
            Operator::DoesNotContain,
            Operator::Is,
            Operator::IsEmpty,
            Operator::IsNotEmpty,
        ],
        PropertyKind::Number { .. } => &[
            Operator::Eq,
            Operator::Ne,
            Operator::Lt,
            Operator::Le,
            Operator::Gt,
            Operator::Ge,
            Operator::IsEmpty,
            Operator::IsNotEmpty,
        ],
        PropertyKind::Select { .. } => &[
            Operator::Is,
            Operator::IsNot,
            Operator::IsEmpty,
            Operator::IsNotEmpty,
        ],
        PropertyKind::MultiSelect { .. } => &[
            Operator::Contains,
            Operator::DoesNotContain,
            Operator::IsEmpty,
            Operator::IsNotEmpty,
        ],
        PropertyKind::Date => &[
            Operator::Is,
            Operator::Before,
            Operator::After,
            Operator::IsEmpty,
            Operator::IsNotEmpty,
        ],
        PropertyKind::Checkbox => &[Operator::Checked, Operator::Unchecked],
    }
}

#[must_use]
pub fn operator_of(condition: &Condition) -> Operator {
    match condition {
        Condition::Text(TextCondition::Contains(_))
        | Condition::MultiSelect(MultiSelectCondition::Contains(_)) => Operator::Contains,
        Condition::Text(TextCondition::DoesNotContain(_))
        | Condition::MultiSelect(MultiSelectCondition::DoesNotContain(_)) => {
            Operator::DoesNotContain
        }
        Condition::Text(TextCondition::Is(_))
        | Condition::Select(SelectCondition::Is(_))
        | Condition::Date(DateCondition::Is(_)) => Operator::Is,
        Condition::Select(SelectCondition::IsNot(_)) => Operator::IsNot,
        Condition::Text(TextCondition::IsEmpty)
        | Condition::Number(NumberCondition::IsEmpty)
        | Condition::Select(SelectCondition::IsEmpty)
        | Condition::MultiSelect(MultiSelectCondition::IsEmpty)
        | Condition::Date(DateCondition::IsEmpty) => Operator::IsEmpty,
        Condition::Text(TextCondition::IsNotEmpty)
        | Condition::Number(NumberCondition::IsNotEmpty)
        | Condition::Select(SelectCondition::IsNotEmpty)
        | Condition::MultiSelect(MultiSelectCondition::IsNotEmpty)
        | Condition::Date(DateCondition::IsNotEmpty) => Operator::IsNotEmpty,
        Condition::Number(NumberCondition::Eq(_)) => Operator::Eq,
        Condition::Number(NumberCondition::Ne(_)) => Operator::Ne,
        Condition::Number(NumberCondition::Lt(_)) => Operator::Lt,
        Condition::Number(NumberCondition::Le(_)) => Operator::Le,
        Condition::Number(NumberCondition::Gt(_)) => Operator::Gt,
        Condition::Number(NumberCondition::Ge(_)) => Operator::Ge,
        Condition::Date(DateCondition::Before(_)) => Operator::Before,
        Condition::Date(DateCondition::After(_)) => Operator::After,
        Condition::Checkbox(true) => Operator::Checked,
        Condition::Checkbox(false) => Operator::Unchecked,
    }
}

#[must_use]
pub fn operand_of(condition: &Condition) -> Operand<'_> {
    match condition {
        Condition::Text(
            TextCondition::Contains(text)
            | TextCondition::DoesNotContain(text)
            | TextCondition::Is(text),
        ) => Operand::Text(text),
        Condition::Number(
            NumberCondition::Eq(number)
            | NumberCondition::Ne(number)
            | NumberCondition::Lt(number)
            | NumberCondition::Le(number)
            | NumberCondition::Gt(number)
            | NumberCondition::Ge(number),
        ) => Operand::Number(*number),
        Condition::Select(SelectCondition::Is(option) | SelectCondition::IsNot(option))
        | Condition::MultiSelect(
            MultiSelectCondition::Contains(option) | MultiSelectCondition::DoesNotContain(option),
        ) => Operand::Option(*option),
        Condition::Date(
            DateCondition::Is(date) | DateCondition::Before(date) | DateCondition::After(date),
        ) => Operand::Date(*date),
        _ => Operand::None,
    }
}

#[must_use]
pub fn default_condition(kind: &PropertyKind) -> Condition {
    match kind {
        PropertyKind::Title | PropertyKind::Text | PropertyKind::Url => {
            Condition::Text(TextCondition::Contains(String::new()))
        }
        PropertyKind::Number { .. } => Condition::Number(NumberCondition::IsNotEmpty),
        PropertyKind::Select { .. } => Condition::Select(SelectCondition::IsNotEmpty),
        PropertyKind::MultiSelect { .. } => {
            Condition::MultiSelect(MultiSelectCondition::IsNotEmpty)
        }
        PropertyKind::Date => Condition::Date(DateCondition::IsNotEmpty),
        PropertyKind::Checkbox => Condition::Checkbox(true),
    }
}

fn kind_options(kind: &PropertyKind) -> &[SelectOption] {
    match kind {
        PropertyKind::Select { options } | PropertyKind::MultiSelect { options } => options,
        _ => &[],
    }
}

#[must_use]
pub fn with_operator(
    kind: &PropertyKind,
    current: &Condition,
    operator: Operator,
) -> Option<Condition> {
    let carried = operand_of(current);
    let text = match carried {
        Operand::Text(text) => text.to_owned(),
        _ => String::new(),
    };
    let number = match carried {
        Operand::Number(number) => number,
        _ => 0.0,
    };
    let option = match carried {
        Operand::Option(option) if kind_options(kind).iter().any(|o| o.id == option) => {
            Some(option)
        }
        _ => kind_options(kind).first().map(|option| option.id),
    };
    let date = match carried {
        Operand::Date(date) => Some(date),
        _ => today(),
    };
    Some(match (kind, operator) {
        (_, Operator::Checked) => Condition::Checkbox(true),
        (_, Operator::Unchecked) => Condition::Checkbox(false),
        (PropertyKind::Title | PropertyKind::Text | PropertyKind::Url, op) => {
            Condition::Text(match op {
                Operator::Contains => TextCondition::Contains(text),
                Operator::DoesNotContain => TextCondition::DoesNotContain(text),
                Operator::Is => TextCondition::Is(text),
                Operator::IsEmpty => TextCondition::IsEmpty,
                Operator::IsNotEmpty => TextCondition::IsNotEmpty,
                _ => return None,
            })
        }
        (PropertyKind::Number { .. }, op) => Condition::Number(match op {
            Operator::Eq => NumberCondition::Eq(number),
            Operator::Ne => NumberCondition::Ne(number),
            Operator::Lt => NumberCondition::Lt(number),
            Operator::Le => NumberCondition::Le(number),
            Operator::Gt => NumberCondition::Gt(number),
            Operator::Ge => NumberCondition::Ge(number),
            Operator::IsEmpty => NumberCondition::IsEmpty,
            Operator::IsNotEmpty => NumberCondition::IsNotEmpty,
            _ => return None,
        }),
        (PropertyKind::Select { .. }, op) => Condition::Select(match op {
            Operator::Is => SelectCondition::Is(option?),
            Operator::IsNot => SelectCondition::IsNot(option?),
            Operator::IsEmpty => SelectCondition::IsEmpty,
            Operator::IsNotEmpty => SelectCondition::IsNotEmpty,
            _ => return None,
        }),
        (PropertyKind::MultiSelect { .. }, op) => Condition::MultiSelect(match op {
            Operator::Contains => MultiSelectCondition::Contains(option?),
            Operator::DoesNotContain => MultiSelectCondition::DoesNotContain(option?),
            Operator::IsEmpty => MultiSelectCondition::IsEmpty,
            Operator::IsNotEmpty => MultiSelectCondition::IsNotEmpty,
            _ => return None,
        }),
        (PropertyKind::Date, op) => Condition::Date(match op {
            Operator::Is => DateCondition::Is(date?),
            Operator::Before => DateCondition::Before(date?),
            Operator::After => DateCondition::After(date?),
            Operator::IsEmpty => DateCondition::IsEmpty,
            Operator::IsNotEmpty => DateCondition::IsNotEmpty,
            _ => return None,
        }),
        (PropertyKind::Checkbox, _) => return None,
    })
}

#[must_use]
pub fn with_text(condition: &Condition, text: &str) -> Option<Condition> {
    let text = text.to_owned();
    match condition {
        Condition::Text(TextCondition::Contains(_)) => {
            Some(Condition::Text(TextCondition::Contains(text)))
        }
        Condition::Text(TextCondition::DoesNotContain(_)) => {
            Some(Condition::Text(TextCondition::DoesNotContain(text)))
        }
        Condition::Text(TextCondition::Is(_)) => Some(Condition::Text(TextCondition::Is(text))),
        _ => None,
    }
}

#[must_use]
pub fn with_number(condition: &Condition, number: f64) -> Option<Condition> {
    Some(Condition::Number(match condition {
        Condition::Number(NumberCondition::Eq(_)) => NumberCondition::Eq(number),
        Condition::Number(NumberCondition::Ne(_)) => NumberCondition::Ne(number),
        Condition::Number(NumberCondition::Lt(_)) => NumberCondition::Lt(number),
        Condition::Number(NumberCondition::Le(_)) => NumberCondition::Le(number),
        Condition::Number(NumberCondition::Gt(_)) => NumberCondition::Gt(number),
        Condition::Number(NumberCondition::Ge(_)) => NumberCondition::Ge(number),
        _ => return None,
    }))
}

#[must_use]
pub fn with_option(condition: &Condition, option: OptionId) -> Option<Condition> {
    Some(match condition {
        Condition::Select(SelectCondition::Is(_)) => Condition::Select(SelectCondition::Is(option)),
        Condition::Select(SelectCondition::IsNot(_)) => {
            Condition::Select(SelectCondition::IsNot(option))
        }
        Condition::MultiSelect(MultiSelectCondition::Contains(_)) => {
            Condition::MultiSelect(MultiSelectCondition::Contains(option))
        }
        Condition::MultiSelect(MultiSelectCondition::DoesNotContain(_)) => {
            Condition::MultiSelect(MultiSelectCondition::DoesNotContain(option))
        }
        _ => return None,
    })
}

#[must_use]
pub fn with_date(condition: &Condition, date: CalendarDate) -> Option<Condition> {
    Some(Condition::Date(match condition {
        Condition::Date(DateCondition::Is(_)) => DateCondition::Is(date),
        Condition::Date(DateCondition::Before(_)) => DateCondition::Before(date),
        Condition::Date(DateCondition::After(_)) => DateCondition::After(date),
        _ => return None,
    }))
}

#[cfg(test)]
mod tests {
    use remindr_core::domain::{
        CalendarDate, Condition, DateCondition, NumberCondition, NumberFormat, OptionColor,
        OptionId, PropertyKind, SelectCondition, SelectOption, TextCondition,
    };

    use super::{
        Operand, Operator, default_condition, operand_of, operator_of, operators, with_number,
        with_operator, with_text,
    };

    fn labels(kind: &PropertyKind) -> Vec<&'static str> {
        operators(kind).iter().map(|op| op.label()).collect()
    }

    fn select(names: &[&str]) -> PropertyKind {
        PropertyKind::Select {
            options: names
                .iter()
                .map(|name| SelectOption {
                    id: OptionId::new(),
                    name: (*name).to_owned(),
                    color: OptionColor::Default,
                })
                .collect(),
        }
    }

    #[test]
    fn filter_conditions_per_kind() {
        assert_eq!(
            labels(&PropertyKind::Text),
            [
                "contains",
                "does not contain",
                "is",
                "is empty",
                "is not empty"
            ]
        );
        assert_eq!(labels(&PropertyKind::Title), labels(&PropertyKind::Url));
        assert_eq!(
            labels(&PropertyKind::Number {
                format: NumberFormat::Plain
            }),
            ["=", "≠", "<", "≤", ">", "≥", "is empty", "is not empty"]
        );
        assert_eq!(
            labels(&select(&["a"])),
            ["is", "is not", "is empty", "is not empty"]
        );
        assert_eq!(
            labels(&PropertyKind::MultiSelect {
                options: Vec::new()
            }),
            ["contains", "does not contain", "is empty", "is not empty"]
        );
        assert_eq!(
            labels(&PropertyKind::Date),
            ["is", "before", "after", "is empty", "is not empty"]
        );
        assert_eq!(
            labels(&PropertyKind::Checkbox),
            ["is checked", "is not checked"]
        );
    }

    #[test]
    fn every_listed_operator_builds_a_condition_that_fits_its_kind() {
        let kinds = [
            PropertyKind::Title,
            PropertyKind::Number {
                format: NumberFormat::Plain,
            },
            select(&["a", "b"]),
            PropertyKind::Date,
            PropertyKind::Checkbox,
        ];
        for kind in &kinds {
            let current = default_condition(kind);
            for operator in operators(kind) {
                let built = with_operator(kind, &current, *operator).unwrap();
                assert_eq!(operator_of(&built), *operator);
            }
        }
    }

    #[test]
    fn switching_operators_keeps_the_typed_value() {
        let kind = PropertyKind::Text;
        let current = Condition::Text(TextCondition::Contains("abc".to_owned()));
        let switched = with_operator(&kind, &current, Operator::Is).unwrap();
        assert_eq!(
            switched,
            Condition::Text(TextCondition::Is("abc".to_owned()))
        );
        let emptied = with_operator(&kind, &switched, Operator::IsEmpty).unwrap();
        assert_eq!(operand_of(&emptied), Operand::None);
        assert_eq!(
            with_text(&current, "x"),
            Some(Condition::Text(TextCondition::Contains("x".to_owned())))
        );
        let number = Condition::Number(NumberCondition::Gt(4.0));
        assert_eq!(
            with_number(&number, 9.0),
            Some(Condition::Number(NumberCondition::Gt(9.0)))
        );
    }

    #[test]
    fn select_conditions_need_an_option_and_dates_start_today() {
        let empty = select(&[]);
        let current = default_condition(&empty);
        assert_eq!(with_operator(&empty, &current, Operator::Is), None);
        let kind = select(&["a"]);
        let built = with_operator(&kind, &default_condition(&kind), Operator::IsNot).unwrap();
        assert!(matches!(
            built,
            Condition::Select(SelectCondition::IsNot(_))
        ));
        let date = with_operator(
            &PropertyKind::Date,
            &default_condition(&PropertyKind::Date),
            Operator::Before,
        )
        .unwrap();
        assert!(matches!(date, Condition::Date(DateCondition::Before(_))));
        let fixed = Condition::Date(DateCondition::After(CalendarDate::new(2026, 1, 2).unwrap()));
        let moved = with_operator(&PropertyKind::Date, &fixed, Operator::Is).unwrap();
        assert_eq!(
            operand_of(&moved),
            Operand::Date(CalendarDate::new(2026, 1, 2).unwrap())
        );
    }
}
