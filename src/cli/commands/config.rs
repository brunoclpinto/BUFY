use chrono::{NaiveDate, Weekday};

use crate::cli::core::{CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::cli::registry::CommandEntry;
use crate::currency::{
    CurrencyCode, DateFormatStyle, LocaleConfig, NegativeStyle, ValuationPolicy,
};

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![CommandEntry::new(
        "config",
        "View and manage global CLI preferences",
        "config [show|set <key> <value>|backup [note]|backups|restore [name]]",
        cmd_config,
    )]
}

fn locale_template(tag: &str) -> LocaleConfig {
    match tag {
        "fr-FR" => LocaleConfig {
            language_tag: "fr-FR".into(),
            decimal_separator: ',',
            grouping_separator: ' ',
            date_format: DateFormatStyle::Long,
            first_weekday: Weekday::Mon,
        },
        "en-GB" => LocaleConfig {
            language_tag: "en-GB".into(),
            decimal_separator: '.',
            grouping_separator: ',',
            date_format: DateFormatStyle::Long,
            first_weekday: Weekday::Mon,
        },
        _ => LocaleConfig::default(),
    }
}

fn cmd_config(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if args.is_empty() || args[0].eq_ignore_ascii_case("show") {
        return context.show_config();
    }

    match args[0].to_lowercase().as_str() {
        "set" => {
            if args.len() < 3 {
                return Err(CommandError::InvalidArguments(
                    "usage: config set <locale|currency|theme|last_opened_ledger> <value>".into(),
                ));
            }
            let key = args[1];
            let value = args[2..].join(" ");
            context.set_config_value(key, value.trim())
        }
        "backup" => {
            let note = if args.len() > 1 {
                Some(args[1..].join(" "))
            } else {
                None
            };
            context.backup_app_config(note)
        }
        "backups" => context.list_config_backups(),
        "restore" => {
            if args.len() > 1 {
                context.restore_config_by_reference(args[1])
            } else {
                if !context.can_prompt() {
                    return Err(CommandError::InvalidArguments(
                        "usage: config restore <name>".into(),
                    ));
                }
                match context.select_config_backup("Select configuration backup:")? {
                    Some(name) => context.restore_config_from_name(name),
                    None => {
                        io::print_info("Operation cancelled.");
                        Ok(())
                    }
                }
            }
        }
        "base-currency" => {
            let code = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config base-currency <ISO>".into())
            })?;
            context.with_ledger_mut(|ledger| {
                ledger.base_currency = CurrencyCode::new(*code);
                Ok(())
            })?;
            io::print_success(format!(
                "Base currency set to {}.",
                CurrencyCode::new(*code).as_str()
            ));
            Ok(())
        }
        "locale" => {
            let tag = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config locale <tag>".into())
            })?;
            let locale = locale_template(tag);
            context.with_ledger_mut(|ledger| {
                ledger.locale = locale.clone();
                Ok(())
            })?;
            io::print_success(format!("Locale set to {}.", locale.language_tag));
            Ok(())
        }
        "negative-style" => {
            let style = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments(
                    "usage: config negative-style <sign|parentheses>".into(),
                )
            })?;
            let negative = match style.to_lowercase().as_str() {
                "sign" => NegativeStyle::Sign,
                "parentheses" => NegativeStyle::Parentheses,
                other => {
                    return Err(CommandError::InvalidArguments(format!(
                        "unknown negative style `{}`",
                        other
                    )))
                }
            };
            context.with_ledger_mut(|ledger| {
                ledger.format.negative_style = negative;
                Ok(())
            })?;
            io::print_success("Negative style updated.");
            Ok(())
        }
        "screen-reader" => {
            let mode = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config screen-reader <on|off>".into())
            })?;
            let enabled = matches!(mode.to_lowercase().as_str(), "on" | "true" | "yes");
            context.with_ledger_mut(|ledger| {
                ledger.format.screen_reader_mode = enabled;
                Ok(())
            })?;
            io::print_success("Screen reader mode updated.");
            Ok(())
        }
        "high-contrast" => {
            let mode = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config high-contrast <on|off>".into())
            })?;
            let enabled = matches!(mode.to_lowercase().as_str(), "on" | "true" | "yes");
            context.with_ledger_mut(|ledger| {
                ledger.format.high_contrast_mode = enabled;
                Ok(())
            })?;
            io::print_success("Contrast preference updated.");
            Ok(())
        }
        "valuation" => {
            let policy = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments(
                    "usage: config valuation <transaction|report|custom> [YYYY-MM-DD]".into(),
                )
            })?;
            let valuation = match policy.to_lowercase().as_str() {
                "transaction" => ValuationPolicy::TransactionDate,
                "report" => ValuationPolicy::ReportDate,
                "custom" => {
                    let date_arg = args.get(2).ok_or_else(|| {
                        CommandError::InvalidArguments(
                            "usage: config valuation custom <YYYY-MM-DD>".into(),
                        )
                    })?;
                    let date = NaiveDate::parse_from_str(date_arg, "%Y-%m-%d").map_err(|_| {
                        CommandError::InvalidArguments(
                            "invalid date (use YYYY-MM-DD)".into(),
                        )
                    })?;
                    ValuationPolicy::CustomDate(date)
                }
                other => {
                    return Err(CommandError::InvalidArguments(format!(
                        "unknown valuation policy `{}`",
                        other
                    )))
                }
            };
            context.with_ledger_mut(|ledger| {
                ledger.valuation_policy = valuation;
                Ok(())
            })?;
            io::print_success("Valuation policy updated.");
            Ok(())
        }
        _ => Err(CommandError::InvalidArguments(
            "usage: config [show|set <key> <value>|backup [note]|backups|restore [name]|base-currency <ISO>|locale <tag>|negative-style <sign|parentheses>|screen-reader <on|off>|high-contrast <on|off>|valuation <transaction|report|custom> [date]]".into(),
        )),
    }
}
