use std::sync::Once;

use budget_core::cli::output::{set_preferences, OutputPreferences};
use budget_core::cli::ui::table_renderer::{Alignment, Table, TableColumn};

fn init_style() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_var("NO_COLOR", "1");
        set_preferences(OutputPreferences::default());
    });
}

fn sample_columns() -> Vec<TableColumn> {
    vec![
        TableColumn {
            header: "NAME".into(),
            min_width: 4,
            max_width: None,
            alignment: Alignment::Left,
        },
        TableColumn {
            header: "VALUE".into(),
            min_width: 6,
            max_width: Some(10),
            alignment: Alignment::Right,
        },
    ]
}

#[test]
fn zero_rows_render_with_header_rules() {
    init_style();
    let table = Table {
        columns: sample_columns(),
        rows: Vec::new(),
        show_headers: true,
        padding: 1,
    };
    let rendered = table.render();
    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), 3, "unexpected layout:\n{rendered}");
    assert!(lines[0].chars().all(|ch| ch == '─'));
    assert!(lines[1].contains("NAME"));
    assert!(lines[2].chars().all(|ch| ch == '─'));
}

#[test]
fn truncation_applies_to_long_cells() {
    init_style();
    let table = Table {
        columns: sample_columns(),
        rows: vec![vec!["ExtremelyLongCellValue".into(), "1234567890".into()]],
        show_headers: false,
        padding: 0,
    };
    let rendered = table.render();
    assert!(rendered.contains('…'), "expected ellipsis in `{rendered}`");
}

#[test]
fn padding_and_alignment_preserved() {
    init_style();
    let table = Table {
        columns: sample_columns(),
        rows: vec![vec!["Alpha".into(), "7".into()]],
        show_headers: true,
        padding: 2,
    };
    let rendered = table.render();
    let value_line = rendered
        .lines()
        .find(|line| line.contains("Alpha"))
        .expect("value line");
    let name_idx = value_line.find("Alpha").unwrap();
    let value_idx = value_line.rfind("7").unwrap();
    assert!(
        value_idx > name_idx,
        "value should appear after padded name: {value_line}"
    );
}
