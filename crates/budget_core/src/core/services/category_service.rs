//! Business logic helpers for category management.

use uuid::Uuid;

use crate::core::services::{ServiceError, ServiceResult};
use crate::domain::category::Category;
use crate::ledger::Ledger;

/// Provides validated operations for [`Category`] entities.
///
/// See also: [`crate::core::services::AccountService`] when linking accounts to categories.
pub struct CategoryService;

impl CategoryService {
    /// Adds a new category and ensures its name and parent are valid.
    pub fn add(ledger: &mut Ledger, category: Category) -> ServiceResult<()> {
        Self::validate_name(ledger, None, &category.name)?;
        if let Some(parent_id) = category.parent_id {
            Self::validate_parent(ledger, parent_id, None)?;
        }
        ledger.add_category(category);
        Ok(())
    }

    /// Applies updates to a category, respecting parentage rules.
    pub fn edit(ledger: &mut Ledger, id: Uuid, changes: Category) -> ServiceResult<()> {
        Self::validate_name(ledger, Some(id), &changes.name)?;
        if let Some(parent_id) = changes.parent_id {
            Self::validate_parent(ledger, parent_id, Some(id))?;
        }
        let category = ledger
            .category_mut(id)
            .ok_or_else(|| ServiceError::Invalid("Category not found".into()))?;
        category.name = changes.name;
        category.kind = changes.kind;
        category.parent_id = changes.parent_id;
        category.is_custom = changes.is_custom;
        category.notes = changes.notes;
        ledger.touch();
        Ok(())
    }

    /// Removes a category after verifying it has no children or transactions.
    pub fn remove(ledger: &mut Ledger, id: Uuid) -> ServiceResult<()> {
        if ledger
            .categories
            .iter()
            .any(|cat| cat.parent_id == Some(id))
        {
            return Err(ServiceError::Invalid(
                "Category has child categories".into(),
            ));
        }
        if ledger
            .transactions
            .iter()
            .any(|txn| txn.category_id == Some(id))
        {
            return Err(ServiceError::Invalid(
                "Category has linked transactions".into(),
            ));
        }
        let before = ledger.categories.len();
        ledger.categories.retain(|category| category.id != id);
        if ledger.categories.len() == before {
            return Err(ServiceError::Invalid("Category not found".into()));
        }
        ledger.touch();
        Ok(())
    }

    /// Returns a snapshot of all categories.
    pub fn list(ledger: &Ledger) -> Vec<&Category> {
        ledger.categories.iter().collect()
    }

    fn validate_name(ledger: &Ledger, exclude: Option<Uuid>, candidate: &str) -> ServiceResult<()> {
        let normalized = candidate.trim().to_ascii_lowercase();
        let duplicate = ledger.categories.iter().any(|category| {
            let name = category.name.trim().to_ascii_lowercase();
            name == normalized && (exclude != Some(category.id))
        });
        if duplicate {
            Err(ServiceError::Invalid(format!(
                "Category `{}` already exists",
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
    ) -> ServiceResult<()> {
        if Some(parent_id) == current {
            return Err(ServiceError::Invalid(
                "Category cannot be its own parent".into(),
            ));
        }
        if ledger.category(parent_id).is_none() {
            return Err(ServiceError::Invalid("Parent category not found".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::category::{Category, CategoryKind};
    use crate::ledger::{BudgetPeriod, Ledger};

    fn sample_ledger() -> Ledger {
        Ledger::new("Categories", BudgetPeriod::monthly())
    }

    #[test]
    fn add_rejects_duplicates() {
        let mut ledger = sample_ledger();
        let category = Category::new("Groceries", CategoryKind::Expense);
        CategoryService::add(&mut ledger, category.clone()).expect("first add succeeds");

        let err = CategoryService::add(&mut ledger, category).expect_err("duplicate fails");
        assert!(
            matches!(err, ServiceError::Invalid(ref message) if message.contains("already exists")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn remove_blocks_parent_with_children() {
        let mut ledger = sample_ledger();
        let parent = Category::new("Parent", CategoryKind::Expense);
        let parent_id = parent.id;
        CategoryService::add(&mut ledger, parent).unwrap();
        let mut child = Category::new("Child", CategoryKind::Expense);
        child.parent_id = Some(parent_id);
        CategoryService::add(&mut ledger, child).unwrap();

        let err = CategoryService::remove(&mut ledger, parent_id).expect_err("has child");
        assert!(
            matches!(err, ServiceError::Invalid(ref message) if message.contains("child")),
            "unexpected error: {err:?}"
        );
    }
}
