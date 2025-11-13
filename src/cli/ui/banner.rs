use crate::cli::core::ShellContext;
use crate::cli::output::current_preferences;
use crate::cli::ui::formatting::Formatter;

pub struct Banner;

impl Banner {
    pub fn render(context: &ShellContext) {
        let formatter = Formatter::new();
        formatter.print_detail(Self::text(context));
    }

    pub fn text(context: &ShellContext) -> String {
        let ledger_segment = {
            let manager = context.manager();
            manager
                .current_name()
                .map(|name| format!("ledger: {}", name))
                .unwrap_or_else(|| "no-ledger".to_string())
        };

        let simulation_segment = context
            .active_simulation_name()
            .map(|name| format!(" (simulation: {})", name))
            .unwrap_or_default();

        let arrow = if current_preferences().plain_mode {
            ">"
        } else {
            "⮞"
        };

        format!("{ledger_segment}{simulation_segment} {arrow}")
    }
}
