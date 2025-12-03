use bufy_domain::{
    category::{Category, CategoryBudgetDefinition, CategoryKind},
    BudgetPeriod,
};
use chrono::NaiveDate;

#[test]
fn category_budget_defaults_to_none() {
    let category = Category::new("Travel", CategoryKind::Expense);
    assert!(!category.has_budget());
    assert!(category.budget().is_none());
}

#[test]
fn category_budget_can_be_assigned_and_cleared() {
    let mut category = Category::new("Housing", CategoryKind::Expense);
    let reference = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    category.set_budget(1250.0, BudgetPeriod::Monthly, Some(reference));

    let budget = category.budget().expect("budget assigned");
    assert!((budget.amount - 1250.0).abs() < f64::EPSILON);
    assert_eq!(budget.period, BudgetPeriod::Monthly);
    assert_eq!(budget.reference_date, Some(reference));
    assert!(category.has_budget());

    category.clear_budget();
    assert!(!category.has_budget());
}

#[test]
fn category_budget_survives_serialization_roundtrip() {
    let mut category = Category::new("Subscriptions", CategoryKind::Expense);
    let definition = CategoryBudgetDefinition::new(89.99, BudgetPeriod::Monthly)
        .with_reference_date(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap());
    category.set_budget_definition(definition.clone());

    let json = serde_json::to_string(&category).expect("serialize");
    assert!(json.contains("budget"));

    let restored: Category = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.budget(), Some(&definition));
}
