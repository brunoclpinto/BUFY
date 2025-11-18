use budget_core::cli::ui::detail_view::DetailView;

#[test]
fn renders_basic_detail_view() {
    let view = DetailView::new("Account: Groceries")
        .with_field("name", "\"Groceries\"")
        .with_field("budgeted_total", "450.00");

    let output = view.render();
    let lines: Vec<&str> = output.lines().collect();

    assert!(lines[0].starts_with("⮞ Account: Groceries"));
    assert!(lines[1].chars().all(|ch| ch == '─'));
    assert_eq!(lines[2], "{");
    assert!(lines.iter().any(|line| line.contains("\"name")));
    assert!(lines.iter().any(|line| line.contains("\"budgeted_total")));
    assert_eq!(lines[lines.len() - 2], "}");
    assert_eq!(lines[1], lines[lines.len() - 1]);
}

#[test]
fn aligns_values_based_on_longest_key() {
    let view = DetailView::new("Alignment Test")
        .with_field("a", "value-a")
        .with_field("longer_key", "value-long")
        .with_field("mid", "value-mid");

    let output = view.render();
    let lines: Vec<&str> = output.lines().collect();

    let mut value_positions = Vec::new();
    for needle in ["value-a", "value-long", "value-mid"] {
        let line = lines
            .iter()
            .find(|line| line.contains(needle))
            .expect("missing field line");
        let pos = line.find(needle).expect("value missing in line");
        value_positions.push(pos);
    }

    assert!(value_positions
        .windows(2)
        .all(|window| window[0] == window[1]));
}

#[test]
fn empty_fields_render_braces() {
    let view = DetailView::new("Empty Detail");
    let output = view.render();
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 5);
    assert_eq!(lines[2], "{");
    assert_eq!(lines[3], "}");
    assert!(lines[1].chars().all(|ch| ch == '─'));
    assert_eq!(lines[1], lines[4]);
}
