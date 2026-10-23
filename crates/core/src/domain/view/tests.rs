use super::{
    Condition, DateCondition, Filter, MultiSelectCondition, NumberCondition, SelectCondition, Sort,
    SortDirection, TextCondition, ViewKind,
};
use crate::domain::database::fixture::Sample;
use crate::domain::{
    BlockId, CalendarDate, DomainError, OptionId, PropertyId, PropertyValue, ViewId,
};

fn titles(sample: &Sample) -> Vec<String> {
    sample
        .workspace
        .query(sample.view)
        .unwrap()
        .into_iter()
        .map(|row| sample.workspace.block(row).unwrap().text.to_plain())
        .collect()
}

fn filter_by(property: PropertyId, condition: Condition, sample: &mut Sample) {
    sample.view_mut().filters = vec![Filter {
        property,
        condition,
    }];
}

fn sort_by(property: PropertyId, direction: SortDirection, sample: &mut Sample) {
    sample.view_mut().sorts = vec![Sort {
        property,
        direction,
    }];
}

fn date(day: u8) -> CalendarDate {
    CalendarDate::new(2026, 10, day).unwrap()
}

fn rows(sample: &mut Sample, names: &[&str]) -> Vec<BlockId> {
    names.iter().map(|name| sample.add_row(name)).collect()
}

#[test]
fn status_is_done_filter_returns_only_done_rows() {
    let mut sample = Sample::new();
    let [a, b, c] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.status, PropertyValue::Select(sample.finished));
    sample.set(b, sample.status, PropertyValue::Select(sample.todo));
    sample.set(c, sample.status, PropertyValue::Select(sample.finished));
    filter_by(
        sample.status,
        Condition::Select(SelectCondition::Is(sample.finished)),
        &mut sample,
    );
    assert_eq!(titles(&sample), ["a", "c"]);
}

#[test]
fn sorting_by_number_descending_then_title_ascending_puts_rows_without_number_last() {
    let mut sample = Sample::new();
    let [b, a, d, _, _] = rows(&mut sample, &["b", "a", "d", "c", "e"])[..] else {
        panic!("five rows");
    };
    sample.set(b, sample.amount, PropertyValue::Number(2.0));
    sample.set(a, sample.amount, PropertyValue::Number(2.0));
    sample.set(d, sample.amount, PropertyValue::Number(10.0));
    sample.view_mut().sorts = vec![
        Sort {
            property: sample.amount,
            direction: SortDirection::Descending,
        },
        Sort {
            property: sample.title,
            direction: SortDirection::Ascending,
        },
    ];
    assert_eq!(titles(&sample), ["d", "a", "b", "c", "e"]);
}

#[test]
fn rows_without_a_value_come_last_in_both_directions() {
    let mut sample = Sample::new();
    let [a, _, c] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.amount, PropertyValue::Number(1.0));
    sample.set(c, sample.amount, PropertyValue::Number(2.0));
    sort_by(sample.amount, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["a", "c", "b"]);
    sort_by(sample.amount, SortDirection::Descending, &mut sample);
    assert_eq!(titles(&sample), ["c", "a", "b"]);
}

#[test]
fn text_title_and_url_sort_case_insensitively() {
    let mut sample = Sample::new();
    let [a, b, c] = rows(&mut sample, &["banana", "Apple", "cherry"])[..] else {
        panic!("three rows");
    };
    sort_by(sample.title, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["Apple", "banana", "cherry"]);
    sort_by(sample.title, SortDirection::Descending, &mut sample);
    assert_eq!(titles(&sample), ["cherry", "banana", "Apple"]);
    sample.set(a, sample.notes, PropertyValue::Text("b".to_owned()));
    sample.set(b, sample.notes, PropertyValue::Text("C".to_owned()));
    sample.set(c, sample.notes, PropertyValue::Text("a".to_owned()));
    sort_by(sample.notes, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["cherry", "banana", "Apple"]);
    sample.set(a, sample.link, PropertyValue::Url("z".to_owned()));
    sample.set(b, sample.link, PropertyValue::Url("Y".to_owned()));
    sort_by(sample.link, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["Apple", "banana", "cherry"]);
}

#[test]
fn select_sorts_by_option_order_and_multi_select_lexicographically() {
    let mut sample = Sample::new();
    let [a, b, c] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.status, PropertyValue::Select(sample.finished));
    sample.set(b, sample.status, PropertyValue::Select(sample.todo));
    sample.set(c, sample.status, PropertyValue::Select(sample.doing));
    sort_by(sample.status, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["b", "c", "a"]);
    sample.set(
        a,
        sample.tags,
        PropertyValue::MultiSelect(vec![sample.blue]),
    );
    sample.set(
        b,
        sample.tags,
        PropertyValue::MultiSelect(vec![sample.red, sample.blue]),
    );
    sample.set(c, sample.tags, PropertyValue::MultiSelect(vec![sample.red]));
    sort_by(sample.tags, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["c", "b", "a"]);
}

#[test]
fn date_and_checkbox_sort_in_natural_order() {
    let mut sample = Sample::new();
    let [a, b, c] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.due, PropertyValue::Date(date(20)));
    sample.set(b, sample.due, PropertyValue::Date(date(3)));
    sample.set(c, sample.due, PropertyValue::Date(date(11)));
    sort_by(sample.due, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["b", "c", "a"]);
    sample.set(a, sample.done, PropertyValue::Checkbox(true));
    sample.set(b, sample.done, PropertyValue::Checkbox(false));
    sort_by(sample.done, SortDirection::Ascending, &mut sample);
    assert_eq!(titles(&sample), ["b", "c", "a"]);
    sort_by(sample.done, SortDirection::Descending, &mut sample);
    assert_eq!(titles(&sample), ["a", "b", "c"]);
}

#[test]
fn filters_and_together() {
    let mut sample = Sample::new();
    let [a, b, c] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    for (row, status, amount) in [
        (a, sample.finished, 5.0),
        (b, sample.finished, 1.0),
        (c, sample.todo, 5.0),
    ] {
        sample.set(row, sample.status, PropertyValue::Select(status));
        sample.set(row, sample.amount, PropertyValue::Number(amount));
    }
    sample.view_mut().filters = vec![
        Filter {
            property: sample.status,
            condition: Condition::Select(SelectCondition::Is(sample.finished)),
        },
        Filter {
            property: sample.amount,
            condition: Condition::Number(NumberCondition::Ge(5.0)),
        },
    ];
    assert_eq!(titles(&sample), ["a"]);
}

#[test]
fn title_filters_use_the_row_text() {
    let mut sample = Sample::new();
    rows(&mut sample, &["Buy milk", "Write code", ""]);
    filter_by(
        sample.title,
        Condition::Text(TextCondition::Contains("MILK".to_owned())),
        &mut sample,
    );
    assert_eq!(titles(&sample), ["Buy milk"]);
    filter_by(
        sample.title,
        Condition::Text(TextCondition::IsEmpty),
        &mut sample,
    );
    assert_eq!(titles(&sample), [""]);
    filter_by(
        sample.title,
        Condition::Text(TextCondition::IsNotEmpty),
        &mut sample,
    );
    assert_eq!(titles(&sample), ["Buy milk", "Write code"]);
}

#[test]
fn text_conditions_are_case_insensitive_and_treat_a_missing_value_as_empty() {
    let mut sample = Sample::new();
    let [a, b, _] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(
        a,
        sample.notes,
        PropertyValue::Text("Hello World".to_owned()),
    );
    sample.set(b, sample.notes, PropertyValue::Text(String::new()));
    let cases = [
        (TextCondition::Contains("WORLD".to_owned()), vec!["a"]),
        (
            TextCondition::DoesNotContain("world".to_owned()),
            vec!["b", "c"],
        ),
        (TextCondition::Is("hello world".to_owned()), vec!["a"]),
        (TextCondition::Is("hello".to_owned()), vec![]),
        (TextCondition::IsEmpty, vec!["b", "c"]),
        (TextCondition::IsNotEmpty, vec!["a"]),
    ];
    for (condition, expected) in cases {
        filter_by(sample.notes, Condition::Text(condition), &mut sample);
        assert_eq!(titles(&sample), expected);
    }
}

#[test]
fn url_filters_behave_like_text_filters() {
    let mut sample = Sample::new();
    let [a, _] = rows(&mut sample, &["a", "b"])[..] else {
        panic!("two rows");
    };
    sample.set(
        a,
        sample.link,
        PropertyValue::Url("https://Example.dev".to_owned()),
    );
    filter_by(
        sample.link,
        Condition::Text(TextCondition::Contains("example".to_owned())),
        &mut sample,
    );
    assert_eq!(titles(&sample), ["a"]);
    filter_by(
        sample.link,
        Condition::Text(TextCondition::IsEmpty),
        &mut sample,
    );
    assert_eq!(titles(&sample), ["b"]);
}

#[test]
fn number_comparisons_never_match_a_missing_value() {
    let mut sample = Sample::new();
    let [a, b, _] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.amount, PropertyValue::Number(1.0));
    sample.set(b, sample.amount, PropertyValue::Number(5.0));
    let cases = [
        (NumberCondition::Eq(5.0), vec!["b"]),
        (NumberCondition::Ne(5.0), vec!["a"]),
        (NumberCondition::Lt(5.0), vec!["a"]),
        (NumberCondition::Le(5.0), vec!["a", "b"]),
        (NumberCondition::Gt(1.0), vec!["b"]),
        (NumberCondition::Ge(1.0), vec!["a", "b"]),
        (NumberCondition::IsEmpty, vec!["c"]),
        (NumberCondition::IsNotEmpty, vec!["a", "b"]),
    ];
    for (condition, expected) in cases {
        filter_by(sample.amount, Condition::Number(condition), &mut sample);
        assert_eq!(titles(&sample), expected);
    }
}

#[test]
fn select_is_not_and_is_empty_match_a_missing_value() {
    let mut sample = Sample::new();
    let [a, b, _] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.status, PropertyValue::Select(sample.todo));
    sample.set(b, sample.status, PropertyValue::Select(sample.doing));
    let cases = [
        (SelectCondition::Is(sample.todo), vec!["a"]),
        (SelectCondition::IsNot(sample.todo), vec!["b", "c"]),
        (SelectCondition::IsEmpty, vec!["c"]),
        (SelectCondition::IsNotEmpty, vec!["a", "b"]),
    ];
    for (condition, expected) in cases {
        filter_by(sample.status, Condition::Select(condition), &mut sample);
        assert_eq!(titles(&sample), expected);
    }
}

#[test]
fn multi_select_does_not_contain_matches_a_missing_or_empty_value() {
    let mut sample = Sample::new();
    let [a, b, _] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(
        a,
        sample.tags,
        PropertyValue::MultiSelect(vec![sample.red, sample.blue]),
    );
    sample.set(b, sample.tags, PropertyValue::MultiSelect(Vec::new()));
    let cases = [
        (MultiSelectCondition::Contains(sample.red), vec!["a"]),
        (
            MultiSelectCondition::DoesNotContain(sample.red),
            vec!["b", "c"],
        ),
        (MultiSelectCondition::IsEmpty, vec!["b", "c"]),
        (MultiSelectCondition::IsNotEmpty, vec!["a"]),
    ];
    for (condition, expected) in cases {
        filter_by(sample.tags, Condition::MultiSelect(condition), &mut sample);
        assert_eq!(titles(&sample), expected);
    }
}

#[test]
fn date_comparisons_never_match_a_missing_value() {
    let mut sample = Sample::new();
    let [a, b, _] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.due, PropertyValue::Date(date(5)));
    sample.set(b, sample.due, PropertyValue::Date(date(20)));
    let cases = [
        (DateCondition::Is(date(5)), vec!["a"]),
        (DateCondition::Before(date(20)), vec!["a"]),
        (DateCondition::After(date(5)), vec!["b"]),
        (DateCondition::IsEmpty, vec!["c"]),
        (DateCondition::IsNotEmpty, vec!["a", "b"]),
    ];
    for (condition, expected) in cases {
        filter_by(sample.due, Condition::Date(condition), &mut sample);
        assert_eq!(titles(&sample), expected);
    }
}

#[test]
fn checkbox_filter_counts_a_missing_value_as_false() {
    let mut sample = Sample::new();
    let [a, b, _] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    sample.set(a, sample.done, PropertyValue::Checkbox(true));
    sample.set(b, sample.done, PropertyValue::Checkbox(false));
    filter_by(sample.done, Condition::Checkbox(true), &mut sample);
    assert_eq!(titles(&sample), ["a"]);
    filter_by(sample.done, Condition::Checkbox(false), &mut sample);
    assert_eq!(titles(&sample), ["b", "c"]);
}

#[test]
fn rows_are_listed_by_row_key_without_filters_sorts_or_manual_order() {
    let mut sample = Sample::new();
    rows(&mut sample, &["z", "y", "x"]);
    assert_eq!(titles(&sample), ["z", "y", "x"]);
}

#[test]
fn manual_order_lists_rows_first_then_the_rest_by_row_key() {
    let mut sample = Sample::new();
    let [_, _, c, d] = rows(&mut sample, &["a", "b", "c", "d"])[..] else {
        panic!("four rows");
    };
    sample.view_mut().manual_order = vec![d, c];
    assert_eq!(titles(&sample), ["d", "c", "a", "b"]);
}

#[test]
fn manual_order_breaks_ties_after_sorts() {
    let mut sample = Sample::new();
    let [a, b, c, d] = rows(&mut sample, &["a", "b", "c", "d"])[..] else {
        panic!("four rows");
    };
    sample.set(a, sample.amount, PropertyValue::Number(1.0));
    sample.set(b, sample.amount, PropertyValue::Number(1.0));
    sample.set(c, sample.amount, PropertyValue::Number(2.0));
    sample.set(d, sample.amount, PropertyValue::Number(1.0));
    sort_by(sample.amount, SortDirection::Ascending, &mut sample);
    sample.view_mut().manual_order = vec![d, b, c];
    assert_eq!(titles(&sample), ["d", "b", "a", "c"]);
}

#[test]
fn unknown_ids_in_manual_order_are_ignored() {
    let mut sample = Sample::new();
    let [a, b] = rows(&mut sample, &["a", "b"])[..] else {
        panic!("two rows");
    };
    sample.view_mut().manual_order = vec![BlockId::new(), b, BlockId::new(), a];
    assert_eq!(titles(&sample), ["b", "a"]);
}

#[test]
fn two_views_keep_separate_manual_orders() {
    let mut sample = Sample::new();
    let [a, b] = rows(&mut sample, &["a", "b"])[..] else {
        panic!("two rows");
    };
    let mut other = sample.workspace.view(sample.view).unwrap().clone();
    other.id = ViewId::new();
    other.manual_order = vec![b, a];
    let other_id = other.id;
    sample.workspace.views.insert(other.id, other);
    assert_eq!(titles(&sample), ["a", "b"]);
    let reversed: Vec<BlockId> = sample.workspace.query(other_id).unwrap();
    assert_eq!(reversed, [b, a]);
}

#[test]
fn deleted_rows_are_not_returned() {
    let mut sample = Sample::new();
    let [a, _] = rows(&mut sample, &["a", "b"])[..] else {
        panic!("two rows");
    };
    sample.workspace.blocks.get_mut(&a).unwrap().alive = false;
    sample.view_mut().manual_order = vec![a];
    assert_eq!(titles(&sample), ["b"]);
}

#[test]
fn unknown_view_is_rejected() {
    let sample = Sample::new();
    let unknown = ViewId::new();
    assert_eq!(
        sample.workspace.query(unknown),
        Err(DomainError::UnknownView(unknown))
    );
    assert_eq!(
        sample.workspace.board_groups(unknown),
        Err(DomainError::UnknownView(unknown))
    );
}

#[test]
fn a_dead_database_has_no_rows() {
    let mut sample = Sample::new();
    rows(&mut sample, &["a"]);
    sample
        .workspace
        .databases
        .get_mut(&sample.database)
        .unwrap()
        .alive = false;
    assert_eq!(sample.workspace.query(sample.view), Ok(Vec::new()));
}

#[test]
fn a_view_of_a_missing_database_has_no_rows() {
    let mut sample = Sample::new();
    rows(&mut sample, &["a"]);
    sample.workspace.databases.clear();
    assert_eq!(sample.workspace.query(sample.view), Ok(Vec::new()));
}

fn board(sample: &mut Sample) {
    sample.view_mut().kind = ViewKind::Board {
        group_by: sample.status,
    };
}

#[test]
fn board_groups_list_the_empty_group_first_then_one_per_option() {
    let mut sample = Sample::new();
    let [a, b, c, d] = rows(&mut sample, &["a", "b", "c", "d"])[..] else {
        panic!("four rows");
    };
    sample.set(a, sample.status, PropertyValue::Select(sample.finished));
    sample.set(b, sample.status, PropertyValue::Select(sample.todo));
    sample.set(c, sample.status, PropertyValue::Select(sample.finished));
    board(&mut sample);
    let groups = sample.workspace.board_groups(sample.view).unwrap();
    let options: Vec<Option<OptionId>> = groups.iter().map(|group| group.option).collect();
    assert_eq!(
        options,
        [
            None,
            Some(sample.todo),
            Some(sample.doing),
            Some(sample.finished)
        ]
    );
    assert_eq!(groups[0].rows, [d]);
    assert_eq!(groups[1].rows, [b]);
    assert!(groups[2].rows.is_empty());
    assert_eq!(groups[3].rows, [a, c]);
}

#[test]
fn a_card_moved_to_done_shows_in_the_done_group() {
    let mut sample = Sample::new();
    let [a, b] = rows(&mut sample, &["a", "b"])[..] else {
        panic!("two rows");
    };
    board(&mut sample);
    let before = sample.workspace.board_groups(sample.view).unwrap();
    assert_eq!(before[0].rows, [a, b]);
    sample.set(a, sample.status, PropertyValue::Select(sample.finished));
    let after = sample.workspace.board_groups(sample.view).unwrap();
    assert_eq!(after[0].rows, [b]);
    assert_eq!(after[3].option, Some(sample.finished));
    assert_eq!(after[3].rows, [a]);
}

#[test]
fn board_groups_follow_filters_sorts_and_manual_order() {
    let mut sample = Sample::new();
    let [a, b, c] = rows(&mut sample, &["a", "b", "c"])[..] else {
        panic!("three rows");
    };
    for row in [a, b, c] {
        sample.set(row, sample.status, PropertyValue::Select(sample.todo));
    }
    board(&mut sample);
    sample.view_mut().manual_order = vec![c, a];
    let groups = sample.workspace.board_groups(sample.view).unwrap();
    assert_eq!(groups[1].rows, [c, a, b]);
    filter_by(
        sample.title,
        Condition::Text(TextCondition::DoesNotContain("b".to_owned())),
        &mut sample,
    );
    let groups = sample.workspace.board_groups(sample.view).unwrap();
    assert_eq!(groups[1].rows, [c, a]);
}

#[test]
fn a_row_whose_option_was_removed_goes_to_the_empty_group() {
    let mut sample = Sample::new();
    let [a] = rows(&mut sample, &["a"])[..] else {
        panic!("one row");
    };
    sample.set(a, sample.status, PropertyValue::Select(sample.doing));
    board(&mut sample);
    let database = sample
        .workspace
        .databases
        .get_mut(&sample.database)
        .unwrap();
    for property in &mut database.schema.properties {
        if let crate::domain::PropertyKind::Select { options } = &mut property.kind {
            options.retain(|option| option.id != sample.doing);
        }
    }
    let groups = sample.workspace.board_groups(sample.view).unwrap();
    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0].option, None);
    assert_eq!(groups[0].rows, [a]);
}

#[test]
fn board_groups_require_a_board_view() {
    let sample = Sample::new();
    assert_eq!(
        sample.workspace.board_groups(sample.view),
        Err(DomainError::ValueKindMismatch)
    );
}
