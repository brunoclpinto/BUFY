use crate::cli::core::{CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::Table;
use crate::cli::ui::test_mode;

/// Shared interactive list flow used by list_* commands.
///
/// Each command provides callbacks for gathering entries, building a table,
/// rendering detail views, supplying actions, and executing the selected action.
pub fn run_selectable_table<T, GatherFn, TableFn, DetailFn, ActionsFn, HandleFn>(
    context: &mut ShellContext,
    selector_label: &'static str,
    action_label: &'static str,
    empty_message: Option<&'static str>,
    mut gather_entries: GatherFn,
    build_table: TableFn,
    build_detail: DetailFn,
    build_actions: ActionsFn,
    mut handle_action: HandleFn,
) -> CommandResult
where
    GatherFn: FnMut(&mut ShellContext) -> Result<Vec<T>, CommandError>,
    TableFn: Fn(&[T]) -> Table,
    DetailFn: Fn(&T) -> DetailView,
    ActionsFn: Fn(&T) -> Vec<DetailAction>,
    HandleFn: FnMut(&mut ShellContext, &T, &DetailAction) -> CommandResult,
{
    loop {
        let entries = gather_entries(context)?;
        if entries.is_empty() {
            if let Some(message) = empty_message {
                cli_io::print_warning(message);
            }
            return Ok(());
        }

        let table = build_table(&entries);
        match select_row(selector_label, &table) {
            RowSelection::Exit => return Ok(()),
            RowSelection::Index(index) => {
                let entry = &entries[index];
                let _ = cli_io::println_text("");
                let detail = build_detail(entry).render();
                let _ = cli_io::println_text(&detail);

                let actions = build_actions(entry);
                if actions.is_empty() {
                    let _ = cli_io::println_text("");
                    continue;
                }

                match choose_action(action_label, &actions) {
                    DetailActionResult::Selected(action) => {
                        handle_action(context, entry, &action)?;
                    }
                    DetailActionResult::Escaped | DetailActionResult::Empty => {}
                }
                let _ = cli_io::println_text("");
            }
        }
    }
}

enum RowSelection {
    Index(usize),
    Exit,
}

fn select_row(label: &str, table: &Table) -> RowSelection {
    if let Some(keys) = test_mode::next_selector_events(label) {
        return match ListSelector::new(table).run_simulated(&keys) {
            ListSelectionResult::Selected(index) => RowSelection::Index(index),
            ListSelectionResult::Escaped | ListSelectionResult::Empty => RowSelection::Exit,
        };
    }

    match ListSelector::new(table).run() {
        ListSelectionResult::Selected(index) => RowSelection::Index(index),
        ListSelectionResult::Escaped | ListSelectionResult::Empty => RowSelection::Exit,
    }
}

fn choose_action(label: &str, actions: &[DetailAction]) -> DetailActionResult {
    if let Some(keys) = test_mode::next_action_events(label) {
        return DetailActionsMenu::new("Actions", actions.to_vec()).run_simulated(&keys);
    }

    DetailActionsMenu::new("Actions", actions.to_vec()).run()
}
