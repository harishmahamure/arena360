use std::collections::{BTreeMap, HashMap, HashSet};

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{ProductRecipe, SelectedOption};
use crate::repositories::ProductRecipeRepository;

const MAX_NAME_LENGTH: usize = 120;
const MAX_PRICE_DELTA: f64 = 1_000_000.0;

/// What one unit of a sale line costs in extra price and ingredients.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionSelection {
    pub price_delta: f64,
    pub options: Vec<SelectedOption>,
    /// Ingredient quantities used by one unit, sorted by ingredient ID.
    pub ingredients: Vec<(Uuid, i32)>,
}

#[derive(Clone)]
pub struct ProductRecipeService {
    repo: ProductRecipeRepository,
}

impl ProductRecipeService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ProductRecipeRepository::new(pool),
        }
    }

    pub async fn get(&self, product_id: Uuid) -> Result<ProductRecipe, AppError> {
        self.find_product(product_id).await?;
        self.repo.get(product_id).await
    }

    pub async fn save(
        &self,
        product_id: Uuid,
        mut recipe: ProductRecipe,
    ) -> Result<ProductRecipe, AppError> {
        let product = self.find_product(product_id).await?;
        if product.is_raw_material && !recipe.is_empty() {
            return Err(AppError::BadRequest(
                "Raw materials cannot have a recipe or options".to_string(),
            ));
        }
        if recipe.is_made_to_order() && self.repo.is_used_as_ingredient(product_id).await? {
            return Err(AppError::BadRequest(format!(
                "{} is an ingredient of another product, so it cannot have its own recipe",
                product.name
            )));
        }
        Self::validate_shape(&recipe)?;
        self.validate_ingredients(product_id, &recipe).await?;
        self.assign_ids(product_id, &mut recipe).await?;
        self.repo.replace(product_id, &recipe).await?;
        self.repo.get(product_id).await
    }

    async fn find_product(
        &self,
        product_id: Uuid,
    ) -> Result<crate::repositories::product_recipe_repo::IngredientInfo, AppError> {
        self.repo
            .ingredient_info(&[product_id])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound(format!("Product with ID {product_id} not found")))
    }

    fn validate_shape(recipe: &ProductRecipe) -> Result<(), AppError> {
        let mut seen = HashSet::new();
        for item in &recipe.items {
            if item.quantity <= 0 {
                return Err(AppError::BadRequest(
                    "Recipe quantities must be positive whole numbers".to_string(),
                ));
            }
            if !seen.insert(item.ingredient_id) {
                return Err(AppError::BadRequest(
                    "Each ingredient may appear only once in the recipe".to_string(),
                ));
            }
        }
        for group in &recipe.option_groups {
            require_name(&group.name, "Option group")?;
            if group.options.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "Option group '{}' needs at least one option",
                    group.name.trim()
                )));
            }
            for option in &group.options {
                require_name(&option.name, "Option")?;
                if !option.price_delta.is_finite() || option.price_delta.abs() > MAX_PRICE_DELTA {
                    return Err(AppError::BadRequest(format!(
                        "Option '{}' has an invalid price change",
                        option.name.trim()
                    )));
                }
                let mut seen = HashSet::new();
                for ingredient in &option.ingredients {
                    if ingredient.quantity == 0 || !seen.insert(ingredient.ingredient_id) {
                        return Err(AppError::BadRequest(format!(
                            "Option '{}' needs non-zero quantities and each ingredient once",
                            option.name.trim()
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    async fn validate_ingredients(
        &self,
        product_id: Uuid,
        recipe: &ProductRecipe,
    ) -> Result<(), AppError> {
        let ids: Vec<Uuid> = recipe
            .items
            .iter()
            .chain(
                recipe
                    .option_groups
                    .iter()
                    .flat_map(|group| &group.options)
                    .flat_map(|option| &option.ingredients),
            )
            .map(|item| item.ingredient_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if ids.contains(&product_id) {
            return Err(AppError::BadRequest(
                "A product cannot be an ingredient of itself".to_string(),
            ));
        }
        let found = self.repo.ingredient_info(&ids).await?;
        if found.len() != ids.len() {
            return Err(AppError::BadRequest(
                "One or more ingredients do not exist".to_string(),
            ));
        }
        if let Some(nested) = found.iter().find(|info| info.has_recipe) {
            return Err(AppError::BadRequest(format!(
                "{} has its own recipe and cannot be used as an ingredient",
                nested.name
            )));
        }
        Ok(())
    }

    /// Keeps IDs the product already owns so past sales stay linked; assigns new ones otherwise.
    async fn assign_ids(
        &self,
        product_id: Uuid,
        recipe: &mut ProductRecipe,
    ) -> Result<(), AppError> {
        let owned: HashSet<Uuid> = self.repo.owned_ids(product_id).await?.into_iter().collect();
        let mut used = HashSet::new();
        let mut keep = |id: Option<Uuid>| match id {
            Some(id) if owned.contains(&id) && used.insert(id) => Some(id),
            _ => Some(Uuid::new_v4()),
        };
        for group in &mut recipe.option_groups {
            group.id = keep(group.id);
            for option in &mut group.options {
                option.id = keep(option.id);
            }
        }
        Ok(())
    }

    /// Validates the chosen options against the recipe and totals their effect on one unit.
    pub fn select_options(
        product_name: &str,
        recipe: &ProductRecipe,
        option_ids: &[Uuid],
    ) -> Result<OptionSelection, AppError> {
        let chosen: HashSet<Uuid> = option_ids.iter().copied().collect();
        if chosen.len() != option_ids.len() {
            return Err(AppError::BadRequest(format!(
                "{product_name}: each option can be chosen once"
            )));
        }
        let mut ingredients: BTreeMap<Uuid, i32> = recipe
            .items
            .iter()
            .map(|item| (item.ingredient_id, item.quantity))
            .collect();
        let mut options = Vec::new();
        let mut price_delta = 0.0;
        for group in &recipe.option_groups {
            let picked: Vec<_> = group
                .options
                .iter()
                .filter(|option| option.id.is_some_and(|id| chosen.contains(&id)))
                .collect();
            if group.required && picked.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "{product_name}: choose an option for {}",
                    group.name
                )));
            }
            if !group.multiple && picked.len() > 1 {
                return Err(AppError::BadRequest(format!(
                    "{product_name}: choose only one option for {}",
                    group.name
                )));
            }
            for option in picked {
                price_delta += option.price_delta;
                for ingredient in &option.ingredients {
                    *ingredients.entry(ingredient.ingredient_id).or_default() +=
                        ingredient.quantity;
                }
                options.push(SelectedOption {
                    option_id: option.id.unwrap_or_default(),
                    group_name: group.name.clone(),
                    name: option.name.clone(),
                    price_delta: option.price_delta,
                });
            }
        }
        if options.len() != chosen.len() {
            return Err(AppError::BadRequest(format!(
                "{product_name}: one or more options are not available"
            )));
        }
        Ok(OptionSelection {
            price_delta,
            options,
            ingredients: ingredients
                .into_iter()
                .filter(|(_, quantity)| *quantity > 0)
                .collect(),
        })
    }
}

/// How many of each made-to-order product the base recipe allows, from
/// `(product, quantity per unit, ingredient stock)` rows.
pub fn made_to_order_capacity(rows: Vec<(Uuid, i32, i32)>) -> HashMap<Uuid, i32> {
    let mut capacity: HashMap<Uuid, i32> = HashMap::new();
    for (product_id, per_unit, stock) in rows {
        let makeable = stock.max(0) / per_unit.max(1);
        capacity
            .entry(product_id)
            .and_modify(|current| *current = (*current).min(makeable))
            .or_insert(makeable);
    }
    capacity
}

fn require_name(name: &str, label: &str) -> Result<(), AppError> {
    let length = name.trim().chars().count();
    if length == 0 || length > MAX_NAME_LENGTH {
        return Err(AppError::BadRequest(format!(
            "{label} names must be 1 to {MAX_NAME_LENGTH} characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ProductOption, ProductOptionGroup, RecipeIngredient};

    fn burger(bun: Uuid, patty: Uuid, cheese: Uuid) -> ProductRecipe {
        let ingredient = |ingredient_id, quantity| RecipeIngredient {
            ingredient_id,
            quantity,
        };
        let option = |name: &str, price_delta, ingredients| ProductOption {
            id: Some(Uuid::new_v4()),
            name: name.to_string(),
            price_delta,
            ingredients,
        };
        ProductRecipe {
            items: vec![
                ingredient(bun, 1),
                ingredient(patty, 1),
                ingredient(cheese, 1),
            ],
            option_groups: vec![
                ProductOptionGroup {
                    id: Some(Uuid::new_v4()),
                    name: "Size".to_string(),
                    required: true,
                    multiple: false,
                    options: vec![
                        option("Single", 0.0, vec![]),
                        option("Double", 60.0, vec![ingredient(patty, 1)]),
                    ],
                },
                ProductOptionGroup {
                    id: Some(Uuid::new_v4()),
                    name: "Extras".to_string(),
                    required: false,
                    multiple: true,
                    options: vec![option("No cheese", -10.0, vec![ingredient(cheese, -1)])],
                },
            ],
        }
    }

    fn option_id(recipe: &ProductRecipe, group: usize, option: usize) -> Uuid {
        recipe.option_groups[group].options[option].id.unwrap()
    }

    #[test]
    fn options_change_price_and_ingredients() {
        let (bun, patty, cheese) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let recipe = burger(bun, patty, cheese);
        let selection = ProductRecipeService::select_options(
            "Burger",
            &recipe,
            &[option_id(&recipe, 0, 1), option_id(&recipe, 1, 0)],
        )
        .unwrap();
        assert_eq!(selection.price_delta, 50.0);
        let mut expected = vec![(bun, 1), (patty, 2)];
        expected.sort();
        assert_eq!(selection.ingredients, expected);
        assert_eq!(selection.options.len(), 2);
    }

    #[test]
    fn enforces_required_single_and_known_options() {
        let recipe = burger(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let select = |ids: &[Uuid]| ProductRecipeService::select_options("Burger", &recipe, ids);
        assert!(select(&[]).is_err());
        assert!(select(&[option_id(&recipe, 0, 0), option_id(&recipe, 0, 1)]).is_err());
        assert!(select(&[option_id(&recipe, 0, 0), Uuid::new_v4()]).is_err());
        assert!(select(&[option_id(&recipe, 0, 0), option_id(&recipe, 0, 0)]).is_err());
        assert!(select(&[option_id(&recipe, 0, 0)]).is_ok());
    }

    #[test]
    fn capacity_is_limited_by_the_scarcest_ingredient() {
        let burger = Uuid::new_v4();
        let capacity = made_to_order_capacity(vec![(burger, 1, 12), (burger, 30, 95)]);
        assert_eq!(capacity[&burger], 3);
    }

    #[test]
    fn products_without_recipe_only_add_option_ingredients() {
        let cheese = Uuid::new_v4();
        let mut recipe = burger(Uuid::new_v4(), Uuid::new_v4(), cheese);
        recipe.items.clear();
        recipe.option_groups.remove(0);
        recipe.option_groups[0].options[0].ingredients[0].quantity = 2;
        let selection =
            ProductRecipeService::select_options("Fries", &recipe, &[option_id(&recipe, 0, 0)])
                .unwrap();
        assert_eq!(selection.ingredients, vec![(cheese, 2)]);
        assert!(!recipe.is_made_to_order());
    }
}
