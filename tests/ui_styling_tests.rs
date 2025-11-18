use std::sync::Once;

use budget_core::cli::output::{set_preferences, OutputPreferences};
use budget_core::cli::ui::{
    detail_actions::{DetailAction, DetailActionsMenu},
    detail_view::DetailView,
    table_renderer::{Alignment, Table, TableColumn},
};

fn init_style() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_var("NO_COLOR", "1");
        set_preferences(OutputPreferences::default());
    });
}

fn sample_table() -> Table {
    Table {
        show_headers: true,
        padding: 1,
        columns: vec![
            TableColumn {
                header: "NAME".into(),
                min_width: 4,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "VALUE".into(),
                min_width: 6,
                max_width: None,
                alignment: Alignment::Right,
            },
        ],
        rows: vec![
            vec!["Alpha".into(), "10".into()],
            vec!["Beta".into(), "25".into()],
        ],
    }
}

#[test]
fn table_renderer_outputs_borders() {
    init_style();
    let rendered = sample_table().render();
    let lines: Vec<&str> = rendered.lines().collect();
    assert!(lines.first().unwrap().chars().all(|ch| ch == '─'));
    assert!(lines
        .iter()
        .any(|line| line.trim_start().starts_with("NAME")));
    assert!(lines.last().unwrap().chars().all(|ch| ch == '─'));
}

#[test]
fn detail_view_header_and_alignment_are_consistent() {
    init_style();
    let view = DetailView::new("Test Detail")
        .with_field("short", "\"A\"")
        .with_field("long_field", "\"B\"");
    let output = view.render();
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines[0].starts_with("⮞ Test Detail"));
    assert_eq!(lines[2], "{");
    let colon_positions: Vec<_> = lines[3..5]
        .iter()
        .map(|line| line.find(':').unwrap())
        .collect();
    assert!(colon_positions.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn detail_actions_render_contains_highlight_marker() {
    init_style();
    let menu = DetailActionsMenu::new(
        "Actions",
        vec![
            DetailAction::new("edit", "EDIT", "Edit item"),
            DetailAction::new("delete", "DELETE", "Delete item"),
        ],
    );
    let snapshot = menu.render_snapshot(0);
    assert!(snapshot.lines().any(|line| line.starts_with("  > ")));
    assert!(snapshot.contains("Actions"));
    assert!(snapshot
        .lines()
        .any(|line| line.chars().all(|ch| ch == '─')));
}

#[test]
fn styled_outputs_do_not_emit_escape_sequences_in_plain_mode() {
    init_style();
    let table = sample_table().render();
    let detail = DetailView::new("Plain Check")
        .with_field("name", "\"Value\"")
        .render();
    let menu = DetailActionsMenu::new(
        "Plain Actions",
        vec![DetailAction::new("edit", "EDIT", "Edit item")],
    );
    let snapshot = menu.render_snapshot(0);
    let combined = format!("{table}\n{detail}\n{snapshot}");
    assert!(!combined.contains("\u{1b}["));
}
