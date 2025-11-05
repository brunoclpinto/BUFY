use crate::cli::output::{info, warning};
use crate::cli::selectors::{SelectionItem, SelectionOutcome, SelectionProvider};
use dialoguer::{theme::ColorfulTheme, Select};

#[derive(Debug)]
pub enum SelectionError<E> {
    Provider(E),
    Interaction(dialoguer::Error),
}

pub struct SelectionManager<P: SelectionProvider> {
    provider: P,
}

impl<P> SelectionManager<P>
where
    P: SelectionProvider,
    P::Id: Clone,
{
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn choose_with<F>(
        mut self,
        prompt: &str,
        empty_message: &str,
        mut selector: F,
    ) -> Result<SelectionOutcome<P::Id>, SelectionError<P::Error>>
    where
        F: FnMut(&str, &[String]) -> Result<Option<usize>, dialoguer::Error>,
    {
        let items = self.provider.items().map_err(SelectionError::Provider)?;
        if items.is_empty() {
            warning(empty_message);
            return Ok(SelectionOutcome::Cancelled);
        }

        info(prompt);
        let labels: Vec<String> = items.iter().map(render_label).collect();
        let display_rows: Vec<String> = labels
            .iter()
            .enumerate()
            .map(|(index, label)| format!("  {:>2}. {}", index + 1, label))
            .collect();

        for row in &display_rows {
            info(row);
        }
        info("  Type cancel or press Esc to abort.");

        let selection = selector(prompt, &display_rows).map_err(SelectionError::Interaction)?;

        if let Some(index) = selection {
            Ok(SelectionOutcome::Selected(items[index].id.clone()))
        } else {
            Ok(SelectionOutcome::Cancelled)
        }
    }

    pub fn choose_with_dialoguer(
        self,
        prompt: &str,
        empty_message: &str,
        theme: &ColorfulTheme,
    ) -> Result<SelectionOutcome<P::Id>, SelectionError<P::Error>> {
        self.choose_with(prompt, empty_message, |prompt, labels| {
            Select::with_theme(theme)
                .with_prompt(prompt)
                .items(labels)
                .default(0)
                .interact_opt()
        })
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
