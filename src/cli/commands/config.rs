use chrono::{NaiveDate, Weekday};

use super::CommandDefinition;
use crate::cli::core::{CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::currency::{
    CurrencyCode, DateFormatStyle, LocaleConfig, NegativeStyle, ValuationPolicy,
};

pub(crate) fn definitions() -> Vec<CommandDefinition> {
    vec![CommandDefinition::new(
        "config",
        "Configure currencies, locale, and valuation",
        "config [show|base-currency|locale|negative-style|screen-reader|high-contrast|valuation|backup|backups|restore]",
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
        context.show_config()?;
        return Ok(());
    }

    match args[0].to_lowercase().as_str() {
        "base-currency" => {
            let code = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config base-currency <ISO>".into())
            })?;
            let ledger = context.current_ledger_mut()?;
            ledger.base_currency = CurrencyCode::new(*code);
            io::print_success(format!(
                "Base currency set to {}.",
                ledger.base_currency.as_str()
            ));
            Ok(())
        }
        "locale" => {
            let tag = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config locale <tag>".into())
            })?;
            let ledger = context.current_ledger_mut()?;
            ledger.locale = locale_template(tag);
            io::print_success(format!("Locale set to {}.", ledger.locale.language_tag));
            Ok(())
        }
        "negative-style" => {
            let style = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments(
                    "usage: config negative-style <sign|parentheses>".into(),
                )
            })?;
            let ledger = context.current_ledger_mut()?;
            ledger.format.negative_style = match style.to_lowercase().as_str() {
                "sign" => NegativeStyle::Sign,
                "parentheses" => NegativeStyle::Parentheses,
                other => {
                    return Err(CommandError::InvalidArguments(format!(
                        "unknown negative style `{}`",
                        other
                    )))
                }
            };
            io::print_success("Negative style updated.");
            Ok(())
        }
        "screen-reader" => {
            let mode = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config screen-reader <on|off>".into())
            })?;
            let ledger = context.current_ledger_mut()?;
            ledger.format.screen_reader_mode =
                matches!(mode.to_lowercase().as_str(), "on" | "true" | "yes");
            io::print_success("Screen reader mode updated.");
            Ok(())
        }
        "high-contrast" => {
            let mode = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments("usage: config high-contrast <on|off>".into())
            })?;
            let ledger = context.current_ledger_mut()?;
            ledger.format.high_contrast_mode =
                matches!(mode.to_lowercase().as_str(), "on" | "true" | "yes");
            io::print_success("Contrast preference updated.");
            Ok(())
        }
        "valuation" => {
            let policy = args.get(1).ok_or_else(|| {
                CommandError::InvalidArguments(
                    "usage: config valuation <transaction|report|custom> [YYYY-MM-DD]".into(),
                )
            })?;
            let ledger = context.current_ledger_mut()?;
            ledger.valuation_policy = match policy.to_lowercase().as_str() {
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
            io::print_success("Valuation policy updated.");
            Ok(())
        }
        "backup" => {
            let mut note: Option<String> = None;
            let mut iter = args.iter().skip(1);
            while let Some(arg) = iter.next() {
                if arg.eq_ignore_ascii_case("--note") {
                    let value = iter.next().ok_or_else(|| {
                        CommandError::InvalidArguments(
                            "usage: config backup [--note <text>]".into(),
                        )
                    })?;
                    note = Some((*value).to_string());
                } else {
                    return Err(CommandError::InvalidArguments(format!(
                        "unknown option `{}`",
                        arg
                    )));
                }
            }
            context.create_config_backup(note)
        }
        "backups" => {
            if args.len() > 1 {
                return Err(CommandError::InvalidArguments(
                    "usage: config backups".into(),
                ));
            }
            context.list_config_backups()
        }
        "restore" => match args.len() {
            1 => {
                if !context.can_prompt() {
                    return Err(CommandError::InvalidArguments(
                        "usage: config restore <backup_reference>".into(),
                    ));
                }
                match context
                    .select_config_backup("Select a configuration backup to restore:")?
                {
                    Some(path) => context.restore_config_from_path(path),
                    None => {
                        io::print_info("Operation cancelled.");
                        Ok(())
                    }
                }
            }
            2 => context.restore_config_by_reference(args[1]),
            _ => Err(CommandError::InvalidArguments(
                "usage: config restore [backup_reference]".into(),
            )),
        },
        _ => Err(CommandError::InvalidArguments(
            "usage: config [show|base-currency|locale|negative-style|screen-reader|high-contrast|valuation|backup|backups|restore]".into(),
        )),
    }
}
