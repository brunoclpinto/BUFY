pub mod account_menu;
pub mod category_menu;
pub mod ledger_menu;
pub mod list_menu;
pub mod main_menu;
pub mod simulation_menu;
mod state;
pub mod transaction_menu;

use crate::cli::core::CommandError;
pub use crate::cli::ui::menu_renderer::MenuRenderError as MenuError;

pub fn menu_error_to_command_error(err: MenuError) -> CommandError {
    match err {
        MenuError::Interrupted | MenuError::EndOfInput => CommandError::ExitRequested,
        MenuError::Io(io_err) => CommandError::Io(io_err),
    }
}
