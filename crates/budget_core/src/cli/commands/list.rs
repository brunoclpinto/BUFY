use std::collections::HashMap;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::formatters::CliFormatters;
use crate::cli::io as cli_io;
use crate::cli::menus::{list_menu, menu_error_to_command_error};
use crate::cli::registry::CommandEntry;
use crate::cli::ui::{Table, TableColumn, TableRenderer};
use crate::core::errors::CliError;
use crate::core::services::BudgetService;
use crate::ledger::{Ledger, TimeInterval, Transaction};
use bufy_core::{CurrencyFormatter, DateFormatter};

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![CommandEntry::new(
        "list",
        "List accounts, categories, transactions, simulations, ledgers...",
        "list <accounts|categories|transactions|simulations|ledgers|backups|recurring>",
        cmd_list,
    )]
}

fn cmd_list(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        let selection = list_menu::show(context).map_err(menu_error_to_command_error)?;
        if let Some(action) = selection {
            handle_list_command(context, &[action.as_str()]).map_err(CommandError::from)
        } else {
            Ok(())
        }
    } else {
        handle_list_command(context, args).map_err(CommandError::from)
    }
}

pub fn handle_list_command(context: &ShellContext, args: &[&str]) -> Result<(), CliError> {
    let target = args
        .first()
        .copied()
        .unwrap_or("ledgers")
        .to_ascii_lowercase();
    match target.as_str() {
        "ledgers" => list_ledgers(context),
        "accounts" => list_accounts(context),
        "categories" => list_categories(context),
        "transactions" => list_transactions(context),
        "simulations" => list_simulations(context),
        "backups" => list_backups(context),
        "recurring" => list_recurring(context),
        other => Err(CliError::Input(format!("unknown list target `{}`", other))),
    }
}

fn list_ledgers(context: &ShellContext) -> Result<(), CliError> {
    let metadata = context.list_ledger_metadata().map_err(CliError::from)?;
    if metadata.is_empty() {
        cli_io::print_warning("No ledgers found.");
        return Ok(());
    }

    let mut table = Table::new(
        Some("Ledgers"),
        vec![
            TableColumn::new("NAME", 20),
            TableColumn::new("UPDATED", 24),
            TableColumn::new("BUDGETED", 14),
            TableColumn::new("AVAILABLE", 14),
        ],
    );
    let formatters = context.formatters.clone();
    let style = context.ui_style.clone();
    for entry in metadata {
        let updated = formatters.format_date(entry.updated_at.date_naive());
        let budgeted = formatters.format_amount(entry.total_budgeted, "");
        let available = formatters.format_amount(entry.total_available, "");
        table.add_row(vec![entry.name, updated, budgeted, available]);
    }
    TableRenderer::render(&table, &style);
    Ok(())
}

fn list_accounts(context: &ShellContext) -> Result<(), CliError> {
    let style = context.ui_style.clone();
    let formatters = context.formatters.clone();
    context
        .with_ledger(|ledger| {
            if ledger.accounts.is_empty() {
                cli_io::print_warning("No accounts in this ledger.");
                return Ok(());
            }
            let summary = BudgetService::summarize_current_period(ledger, context.clock.as_ref());
            let totals: HashMap<_, _> = summary
                .per_account
                .iter()
                .map(|entry| (entry.account_id, entry.totals.clone()))
                .collect();

            let mut table = Table::new(
                Some("Accounts"),
                vec![
                    TableColumn::new("NAME", 18),
                    TableColumn::new("TYPE", 18),
                    TableColumn::new("CATEGORY", 18),
                    TableColumn::new("BUDGETED", 12),
                    TableColumn::new("ACTUAL", 12),
                ],
            );

            for account in &ledger.accounts {
                let category = account
                    .category_id
                    .and_then(|id| ledger.category(id))
                    .map(|cat| cat.name.clone())
                    .unwrap_or_else(|| "—".into());
                let totals = totals
                    .get(&account.id)
                    .map(|entry| (entry.budgeted, entry.real))
                    .unwrap_or((0.0, 0.0));
                table.add_row(vec![
                    account.name.clone(),
                    account.kind.to_string(),
                    category,
                    formatters.format_amount(totals.0, ""),
                    formatters.format_amount(totals.1, ""),
                ]);
            }

            TableRenderer::render(&table, &style);
            Ok(())
        })
        .map_err(CliError::from)
}

fn list_categories(context: &ShellContext) -> Result<(), CliError> {
    let style = context.ui_style.clone();
    let formatters = context.formatters.clone();
    context
        .with_ledger(|ledger| {
            if ledger.categories.is_empty() {
                cli_io::print_warning("No categories in this ledger.");
                return Ok(());
            }

            let mut table = Table::new(
                Some("Categories"),
                vec![
                    TableColumn::new("NAME", 20),
                    TableColumn::new("TYPE", 16),
                    TableColumn::new("BUDGET", 12),
                    TableColumn::new("SPENT", 12),
                ],
            );

            let summary = BudgetService::summarize_current_period(ledger, context.clock.as_ref());
            let totals: HashMap<_, _> = summary
                .per_category
                .iter()
                .filter_map(|entry| entry.category_id.map(|id| (id, entry.totals.clone())))
                .collect();

            for category in &ledger.categories {
                let budget = category
                    .budget
                    .as_ref()
                    .map(|budget| formatters.format_amount(budget.amount, ""))
                    .unwrap_or_else(|| "—".into());
                let spent = totals
                    .get(&category.id)
                    .map(|entry| formatters.format_amount(entry.real, ""))
                    .unwrap_or_else(|| formatters.format_amount(0.0, ""));
                table.add_row(vec![
                    category.name.clone(),
                    category.kind.to_string(),
                    budget,
                    spent,
                ]);
            }

            TableRenderer::render(&table, &style);
            Ok(())
        })
        .map_err(CliError::from)
}

fn list_transactions(context: &ShellContext) -> Result<(), CliError> {
    let style = context.ui_style.clone();
    let formatters = context.formatters.clone();
    context
        .with_ledger(|ledger| {
            if ledger.transactions.is_empty() {
                cli_io::print_warning("No transactions recorded.");
                return Ok(());
            }
            let account_names: HashMap<_, _> = ledger
                .accounts
                .iter()
                .map(|acct| (acct.id, acct.name.clone()))
                .collect();

            let mut table = Table::new(
                Some("Transactions"),
                vec![
                    TableColumn::new("DATE", 12),
                    TableColumn::new("FROM", 16),
                    TableColumn::new("TO", 16),
                    TableColumn::new("CATEGORY", 18),
                    TableColumn::new("BUDGETED", 12),
                    TableColumn::new("STATUS", 10),
                ],
            );
            for txn in &ledger.transactions {
                table.add_row(transaction_row(txn, ledger, &account_names, &formatters));
            }
            TableRenderer::render(&table, &style);
            Ok(())
        })
        .map_err(CliError::from)
}

fn transaction_row(
    txn: &Transaction,
    ledger: &Ledger,
    account_names: &HashMap<uuid::Uuid, String>,
    formatters: &CliFormatters,
) -> Vec<String> {
    vec![
        formatters.format_date(txn.scheduled_date),
        account_names
            .get(&txn.from_account)
            .cloned()
            .unwrap_or_else(|| "Unknown".into()),
        account_names
            .get(&txn.to_account)
            .cloned()
            .unwrap_or_else(|| "Unknown".into()),
        txn.category_id
            .and_then(|id| ledger.category(id))
            .map(|cat| cat.name.clone())
            .unwrap_or_else(|| "—".into()),
        formatters.format_amount(txn.budgeted_amount, ""),
        txn.status.to_string(),
    ]
}

fn list_simulations(context: &ShellContext) -> Result<(), CliError> {
    let style = context.ui_style.clone();
    let formatters = context.formatters.clone();
    context
        .with_ledger(|ledger| {
            if ledger.simulations().is_empty() {
                cli_io::print_warning("No simulations recorded.");
                return Ok(());
            }
            let mut table = Table::new(
                Some("Simulations"),
                vec![
                    TableColumn::new("NAME", 20),
                    TableColumn::new("STATUS", 10),
                    TableColumn::new("CHANGES", 8),
                    TableColumn::new("UPDATED", 24),
                ],
            );
            for sim in ledger.simulations() {
                table.add_row(vec![
                    sim.name.clone(),
                    sim.status.to_string(),
                    sim.changes.len().to_string(),
                    formatters.format_date(sim.updated_at.date_naive()),
                ]);
            }
            TableRenderer::render(&table, &style);
            Ok(())
        })
        .map_err(CliError::from)
}

fn list_backups(context: &ShellContext) -> Result<(), CliError> {
    let ledger_name = context.require_named_ledger().map_err(CliError::from)?;
    let backups = context
        .storage
        .list_backup_metadata(&ledger_name)
        .map_err(|err| CliError::Command(err.to_string()))?;
    if backups.is_empty() {
        cli_io::print_warning("No backups found.");
        return Ok(());
    }

    let mut table = Table::new(
        Some(format!("Backups for {}", ledger_name)),
        vec![
            TableColumn::new("NAME", 32),
            TableColumn::new("CREATED", 24),
            TableColumn::new("SIZE", 10),
        ],
    );
    let style = context.ui_style.clone();
    let formatters = context.formatters.clone();
    for backup in backups {
        let created = backup
            .created_at
            .map(|ts| formatters.format_date(ts.date_naive()))
            .unwrap_or_else(|| "—".into());
        table.add_row(vec![
            backup.name.clone(),
            created,
            format_size(backup.size_bytes),
        ]);
    }
    TableRenderer::render(&table, &style);
    Ok(())
}

fn list_recurring(context: &ShellContext) -> Result<(), CliError> {
    let style = context.ui_style.clone();
    let formatters = context.formatters.clone();
    context
        .with_ledger(|ledger| {
            let recurring: Vec<_> = ledger
                .transactions
                .iter()
                .filter(|txn| txn.recurrence.is_some())
                .collect();
            if recurring.is_empty() {
                cli_io::print_warning("No recurring transactions configured.");
                return Ok(());
            }

            let account_names: HashMap<_, _> = ledger
                .accounts
                .iter()
                .map(|acct| (acct.id, acct.name.clone()))
                .collect();
            let mut table = Table::new(
                Some("Recurring schedules"),
                vec![
                    TableColumn::new("TRANSACTION", 24),
                    TableColumn::new("FROM", 16),
                    TableColumn::new("TO", 16),
                    TableColumn::new("INTERVAL", 16),
                    TableColumn::new("NEXT", 12),
                ],
            );
            for txn in recurring {
                let recurrence = txn.recurrence.as_ref().expect("filtered above");
                table.add_row(vec![
                    txn.category_id
                        .and_then(|id| ledger.category(id))
                        .map(|cat| cat.name.clone())
                        .unwrap_or_else(|| format!("Txn {}", txn.id)),
                    account_names
                        .get(&txn.from_account)
                        .cloned()
                        .unwrap_or_else(|| "Unknown".into()),
                    account_names
                        .get(&txn.to_account)
                        .cloned()
                        .unwrap_or_else(|| "Unknown".into()),
                    format_interval(&recurrence.interval),
                    recurrence
                        .next_scheduled
                        .map(|date| formatters.format_date(date))
                        .unwrap_or_else(|| "—".into()),
                ]);
            }
            TableRenderer::render(&table, &style);
            Ok(())
        })
        .map_err(CliError::from)
}

fn format_interval(interval: &TimeInterval) -> String {
    format!("Every {} {:?}", interval.every, interval.unit)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
