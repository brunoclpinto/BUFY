use crate::core::services::{CategoryService, ServiceError};
use bufy_domain::category::{Category, CategoryKind};
use crate::ledger::{BudgetPeriod, Ledger};

#[test]
fn add_category_succeeds() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    let category = Category::new("Food", CategoryKind::Expense);

    CategoryService::add(&mut ledger, category).unwrap();

    assert_eq!(ledger.categories.len(), 1);
}

#[test]
fn duplicate_category_name_rejected() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    CategoryService::add(&mut ledger, Category::new("Rent", CategoryKind::Expense)).unwrap();

    let err = CategoryService::add(&mut ledger, Category::new("rent", CategoryKind::Expense))
        .unwrap_err();
    assert!(matches!(err, ServiceError::Invalid(_)));
}

#[test]
fn edit_category_updates_fields() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    let category = Category::new("Health", CategoryKind::Expense);
    let id = category.id;
    CategoryService::add(&mut ledger, category).unwrap();

    let mut changes = Category::new("Wellness", CategoryKind::Expense);
    changes.id = id;
    changes.is_custom = false;
    CategoryService::edit(&mut ledger, id, changes).unwrap();

    let updated = ledger.category(id).unwrap();
    assert_eq!(updated.name, "Wellness");
    assert!(!updated.is_custom);
}
