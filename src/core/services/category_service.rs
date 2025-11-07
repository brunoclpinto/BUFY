use uuid::Uuid;

use crate::domain::category::Category;
use crate::ledger::Ledger;

use super::{ServiceError, ServiceResult};

pub struct CategoryService;

impl CategoryService {
    pub fn add(ledger: &mut Ledger, category: Category) -> ServiceResult<()> {
        let _ = ledger;
        let _ = category;
        Err(ServiceError::Invalid(
            "CategoryService::add not yet implemented".into(),
        ))
    }

    pub fn edit(ledger: &mut Ledger, id: Uuid, changes: Category) -> ServiceResult<()> {
        let _ = ledger;
        let _ = id;
        let _ = changes;
        Err(ServiceError::Invalid(
            "CategoryService::edit not yet implemented".into(),
        ))
    }

    pub fn remove(ledger: &mut Ledger, id: Uuid) -> ServiceResult<()> {
        let _ = ledger;
        let _ = id;
        Err(ServiceError::Invalid(
            "CategoryService::remove not yet implemented".into(),
        ))
    }

    pub fn list<'a>(ledger: &'a Ledger) -> Vec<&'a Category> {
        let _ = ledger;
        Vec::new()
    }
}
