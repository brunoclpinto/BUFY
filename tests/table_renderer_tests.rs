use budget_core::cli::output::{set_preferences, OutputPreferences};
use budget_core::cli::ui::table_renderer::{
    horizontal_rule, render_cell, Alignment, Table, TableColumn,
};

fn basic_table(columns: Vec<TableColumn>, rows: Vec<Vec<String>>) -> Table {
    Table {
        columns,
        rows,
        show_headers: true,
        padding: 1,
    }
}

#[test]
fn width_calculation_respects_constraints() {
    set_preferences(OutputPreferences::default());

    let columns = vec![
        TableColumn {
            header: "Account".into(),
            min_width: 4,
            max_width: Some(8),
            alignment: Alignment::Left,
        },
        TableColumn {
            header: "Notes".into(),
            min_width: 10,
            max_width: None,
            alignment: Alignment::Left,
        },
    ];

    let rows = vec![
        vec!["AlphaBetaGamma".into(), "Short".into()],
        vec!["BB".into(), "Detailed overview entry".into()],
    ];

    let table = basic_table(columns, rows);
    let widths = table.compute_widths();

    assert_eq!(widths, vec![8, 23]);
}

#[test]
fn render_cell_respects_alignment() {
    set_preferences(OutputPreferences::default());

    let left = render_cell("AB", 4, &Alignment::Left, 1);
    assert_eq!(left, " AB   ");

    let right = render_cell("AB", 4, &Alignment::Right, 1);
    assert_eq!(right, "   AB ");

    let center = render_cell("X", 5, &Alignment::Center, 1);
    assert_eq!(center, "   X   ");
}

#[test]
fn truncation_adds_ellipsis_and_resets_styles() {
    set_preferences(OutputPreferences::default());

    let columns = vec![TableColumn {
        header: "DATA".into(),
        min_width: 3,
        max_width: Some(5),
        alignment: Alignment::Left,
    }];

    let colored = "\u{1b}[31mExtremelyLongValue\u{1b}[0m".to_string();
    let rows = vec![vec![colored]];
    let table = Table {
        columns,
        rows,
        show_headers: false,
        padding: 0,
    };

    let rendered = table.render();
    let lines: Vec<&str> = rendered.lines().collect();
    assert!(lines.iter().any(|line| line.contains('…')));
    assert!(lines
        .get(1)
        .map(|line| line.ends_with("\u{1b}[0m"))
        .unwrap_or(false));
    assert_eq!(table.compute_widths(), vec![5]);
}

#[test]
fn header_rendering_includes_rule() {
    set_preferences(OutputPreferences::default());

    let columns = vec![
        TableColumn {
            header: "ID".into(),
            min_width: 2,
            max_width: None,
            alignment: Alignment::Left,
        },
        TableColumn {
            header: "VALUE".into(),
            min_width: 5,
            max_width: None,
            alignment: Alignment::Right,
        },
    ];

    let rows = vec![vec!["1".into(), "42".into()]];
    let table = basic_table(columns, rows);
    let widths = table.compute_widths();
    let rendered = table.render();
    let lines: Vec<&str> = rendered.lines().collect();

    let header_cells: Vec<String> = table
        .columns
        .iter()
        .map(|col| col.header.to_uppercase())
        .collect();
    let expected_header = table.render_row(&header_cells, &widths);
    let rule_line = horizontal_rule(&widths, table.padding);

    assert_eq!(lines[0], rule_line);
    assert_eq!(lines[1], expected_header);
    assert_eq!(lines[2], rule_line);
    assert_eq!(lines.last().copied(), Some(rule_line.as_str()));
}

#[test]
fn renders_full_table_example() {
    set_preferences(OutputPreferences::default());

    let table = Table {
        show_headers: true,
        padding: 1,
        columns: vec![
            TableColumn {
                header: "NAME".into(),
                min_width: 6,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "BALANCE".into(),
                min_width: 10,
                max_width: None,
                alignment: Alignment::Right,
            },
        ],
        rows: vec![
            vec!["Checking".into(), "1200.00".into()],
            vec!["Savings".into(), "5000.50".into()],
        ],
    };

    let expected = concat!(
        "────────────────────────────────────────\n",
        " NAME           BALANCE\n",
        "────────────────────────────────────────\n",
        " Checking       1200.00\n",
        " Savings        5000.50\n",
        "────────────────────────────────────────"
    );
    assert_eq!(table.render(), expected);
}
