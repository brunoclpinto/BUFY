use uuid::Uuid;

use crate::domain::category::Category;
use crate::ledger::Ledger;

use super::{ServiceError, ServiceResult};

pub struct CategoryService;

impl CategoryService {
    pub fn add(ledger: &mut Ledger, category: Category) -> ServiceResult<()> {
        Self::validate_name(ledger, None, &category.name)?;
        if let Some(parent_id) = category.parent_id {
            Self::validate_parent(ledger, parent_id, None)?;
        }
        ledger.add_category(category);
        Ok(())
    }

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

    pub fn list<'a>(ledger: &'a Ledger) -> Vec<&'a Category> {
        ledger.categories.iter().collect()
    }

    fn validate_name(ledger: &Ledger, exclude: Option<Uuid>, candidate: &str) -> ServiceResult<()> {
        let normalized = candidate.trim().to_ascii_lowercase();
        let duplicate = ledger.categories.iter().any(|category| {
            let name = category.name.trim().to_ascii_lowercase();
            name == normalized && exclude.map_or(true, |id| category.id != id)
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
