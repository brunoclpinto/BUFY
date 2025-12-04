//! Business logic helpers for category management.

use chrono::NaiveDate;
use uuid::Uuid;

use bufy_domain::{category::Category, BudgetPeriod, Ledger};

use crate::CoreError;

/// Provides validated operations for [`Category`] entities.
pub struct CategoryService;

impl CategoryService {
    /// Adds a new category and ensures its name and parent are valid.
    pub fn add(ledger: &mut Ledger, category: Category) -> Result<(), CoreError> {
        Self::validate_name(ledger, None, &category.name)?;
        if let Some(parent_id) = category.parent_id {
            Self::validate_parent(ledger, parent_id, None)?;
        }
        ledger.add_category(category);
        Ok(())
    }

    /// Applies updates to a category, respecting parentage rules.
    pub fn edit(ledger: &mut Ledger, id: Uuid, changes: Category) -> Result<(), CoreError> {
        Self::validate_name(ledger, Some(id), &changes.name)?;
        if let Some(parent_id) = changes.parent_id {
            Self::validate_parent(ledger, parent_id, Some(id))?;
        }
        let category = ledger
            .category_mut(id)
            .ok_or_else(|| CoreError::CategoryNotFound(id.to_string()))?;
        category.name = changes.name;
        category.kind = changes.kind;
        category.parent_id = changes.parent_id;
        category.is_custom = changes.is_custom;
        category.notes = changes.notes;
        ledger.touch();
        Ok(())
    }

    /// Removes a category after verifying it has no children or transactions.
    pub fn remove(ledger: &mut Ledger, id: Uuid) -> Result<(), CoreError> {
        if ledger
            .categories
            .iter()
            .any(|cat| cat.parent_id == Some(id))
        {
            return Err(CoreError::InvalidOperation(
                "category has child categories".into(),
            ));
        }
        if ledger
            .transactions
            .iter()
            .any(|txn| txn.category_id == Some(id))
        {
            return Err(CoreError::InvalidOperation(
                "category has linked transactions".into(),
            ));
        }
        let before = ledger.categories.len();
        ledger.categories.retain(|category| category.id != id);
        if ledger.categories.len() == before {
            return Err(CoreError::CategoryNotFound(id.to_string()));
        }
        ledger.touch();
        Ok(())
    }

    /// Assigns a budget definition to the given category.
    pub fn set_budget(
        ledger: &mut Ledger,
        id: Uuid,
        amount: f64,
        period: BudgetPeriod,
        reference_date: Option<NaiveDate>,
    ) -> Result<(), CoreError> {
        let category = ledger
            .category_mut(id)
            .ok_or_else(|| CoreError::CategoryNotFound(id.to_string()))?;
        category.set_budget(amount, period, reference_date);
        ledger.touch();
        Ok(())
    }

    /// Clears the budget assigned to a category, returning whether it existed.
    pub fn clear_budget(ledger: &mut Ledger, id: Uuid) -> Result<bool, CoreError> {
        let category = ledger
            .category_mut(id)
            .ok_or_else(|| CoreError::CategoryNotFound(id.to_string()))?;
        let had_budget = category.has_budget();
        category.clear_budget();
        if had_budget {
            ledger.touch();
        }
        Ok(had_budget)
    }

    /// Returns a snapshot of all categories.
    pub fn list(ledger: &Ledger) -> Vec<&Category> {
        ledger.categories.iter().collect()
    }

    fn validate_name(
        ledger: &Ledger,
        exclude: Option<Uuid>,
        candidate: &str,
    ) -> Result<(), CoreError> {
        let normalized = candidate.trim().to_ascii_lowercase();
        let duplicate = ledger.categories.iter().any(|category| {
            let name = category.name.trim().to_ascii_lowercase();
            name == normalized && (exclude != Some(category.id))
        });
        if duplicate {
            Err(CoreError::Validation(format!(
                "category `{}` already exists",
                candidate
            )))
        } else {
            Ok(())
        }
    }

    fn validate_parent(
        ledger: &Ledger,
        parent_id: Uuid,
        current: Option<Uuid>,
    ) -> Result<(), CoreError> {
        if Some(parent_id) == current {
            return Err(CoreError::InvalidOperation(
                "category cannot be its own parent".into(),
            ));
        }
        if ledger.category(parent_id).is_none() {
            return Err(CoreError::CategoryNotFound(parent_id.to_string()));
        }
        Ok(())
    }
}
