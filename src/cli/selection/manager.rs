use crate::cli::output::{info, warning};
use crate::cli::selectors::{SelectionItem, SelectionOutcome, SelectionProvider};
use dialoguer::{theme::ColorfulTheme, Select};

#[derive(Debug)]
pub enum SelectionError<E> {
    Provider(E),
    Interaction(dialoguer::Error),
}

pub struct SelectionManager<'a, P: SelectionProvider> {
    provider: P,
    theme: &'a ColorfulTheme,
}

impl<'a, P> SelectionManager<'a, P>
where
    P: SelectionProvider,
    P::Id: Clone,
{
    pub fn new(provider: P, theme: &'a ColorfulTheme) -> Self {
        Self { provider, theme }
    }

    pub fn choose(
        mut self,
        prompt: &str,
    ) -> Result<SelectionOutcome<P::Id>, SelectionError<P::Error>> {
        let items = self.provider.items().map_err(SelectionError::Provider)?;
        if items.is_empty() {
            warning("No items available.");
            return Ok(SelectionOutcome::Cancelled);
        }

        info(prompt);
        let labels: Vec<String> = items.iter().map(render_label).collect();

        let selection = Select::with_theme(self.theme)
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(SelectionError::Interaction)?;

        if let Some(index) = selection {
            Ok(SelectionOutcome::Selected(items[index].id.clone()))
        } else {
            Ok(SelectionOutcome::Cancelled)
        }
    }
}

fn render_label<ID>(item: &SelectionItem<ID>) -> String {
    match (&item.subtitle, &item.category) {
        (Some(sub), Some(cat)) => format!("{} — {} ({})", item.label, sub, cat),
        (Some(sub), None) => format!("{} — {}", item.label, sub),
        (None, Some(cat)) => format!("{} ({})", item.label, cat),
        (None, None) => item.label.clone(),
    }
}
