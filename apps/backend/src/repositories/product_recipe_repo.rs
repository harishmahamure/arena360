use std::collections::HashMap;

use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{ProductOption, ProductOptionGroup, ProductRecipe, RecipeIngredient};

/// An ingredient candidate as seen when validating a recipe.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IngredientInfo {
    pub id: Uuid,
    pub name: String,
    pub is_raw_material: bool,
    pub has_recipe: bool,
}

#[derive(Clone)]
pub struct ProductRecipeRepository {
    pool: PgPool,
}

impl ProductRecipeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, product_id: Uuid) -> Result<ProductRecipe, AppError> {
        let mut conn = self.pool.acquire().await?;
        Self::load(&mut conn, product_id).await
    }

    pub async fn load(
        conn: &mut PgConnection,
        product_id: Uuid,
    ) -> Result<ProductRecipe, AppError> {
        let items: Vec<(Uuid, i32)> = sqlx::query_as(
            r#"SELECT "ingredientId", quantity FROM product_recipe_items
               WHERE "productId" = $1 ORDER BY "ingredientId""#,
        )
        .bind(product_id)
        .fetch_all(&mut *conn)
        .await?;

        let groups: Vec<(Uuid, String, bool, bool)> = sqlx::query_as(
            r#"SELECT id, name, required, multiple FROM product_option_groups
               WHERE "productId" = $1 ORDER BY "sortOrder", id"#,
        )
        .bind(product_id)
        .fetch_all(&mut *conn)
        .await?;

        let options: Vec<(Uuid, Uuid, String, f64)> = sqlx::query_as(
            r#"SELECT o.id, o."groupId", o.name, o."priceDelta"::float8
               FROM product_options o
               JOIN product_option_groups g ON g.id = o."groupId"
               WHERE g."productId" = $1
               ORDER BY o."sortOrder", o.id"#,
        )
        .bind(product_id)
        .fetch_all(&mut *conn)
        .await?;

        let option_ingredients: Vec<(Uuid, Uuid, i32)> = sqlx::query_as(
            r#"SELECT oi."optionId", oi."ingredientId", oi.quantity
               FROM product_option_ingredients oi
               JOIN product_options o ON o.id = oi."optionId"
               JOIN product_option_groups g ON g.id = o."groupId"
               WHERE g."productId" = $1
               ORDER BY oi."ingredientId""#,
        )
        .bind(product_id)
        .fetch_all(&mut *conn)
        .await?;

        let mut ingredients_by_option: HashMap<Uuid, Vec<RecipeIngredient>> = HashMap::new();
        for (option_id, ingredient_id, quantity) in option_ingredients {
            ingredients_by_option
                .entry(option_id)
                .or_default()
                .push(RecipeIngredient {
                    ingredient_id,
                    quantity,
                });
        }
        let mut options_by_group: HashMap<Uuid, Vec<ProductOption>> = HashMap::new();
        for (id, group_id, name, price_delta) in options {
            options_by_group
                .entry(group_id)
                .or_default()
                .push(ProductOption {
                    id: Some(id),
                    name,
                    price_delta,
                    ingredients: ingredients_by_option.remove(&id).unwrap_or_default(),
                });
        }

        Ok(ProductRecipe {
            items: items
                .into_iter()
                .map(|(ingredient_id, quantity)| RecipeIngredient {
                    ingredient_id,
                    quantity,
                })
                .collect(),
            option_groups: groups
                .into_iter()
                .map(|(id, name, required, multiple)| ProductOptionGroup {
                    id: Some(id),
                    name,
                    required,
                    multiple,
                    options: options_by_group.remove(&id).unwrap_or_default(),
                })
                .collect(),
        })
    }

    /// Products that may be used as ingredients: existing, not deleted, without their own recipe.
    pub async fn ingredient_info(&self, ids: &[Uuid]) -> Result<Vec<IngredientInfo>, AppError> {
        Ok(sqlx::query_as::<_, IngredientInfo>(
            r#"SELECT p.id, p.name, p."isRawMaterial" AS is_raw_material,
                      EXISTS(SELECT 1 FROM product_recipe_items r WHERE r."productId" = p.id)
                        AS has_recipe
               FROM products p
               WHERE p.id = ANY($1) AND p."deletedAt" IS NULL"#,
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn is_used_as_ingredient(&self, product_id: Uuid) -> Result<bool, AppError> {
        let (used,): (bool,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM product_recipe_items WHERE "ingredientId" = $1)
                   OR EXISTS(SELECT 1 FROM product_option_ingredients WHERE "ingredientId" = $1)"#,
        )
        .bind(product_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(used)
    }

    /// Option and group IDs currently owned by the product, so edits can keep them.
    pub async fn owned_ids(&self, product_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar(
            r#"SELECT g.id FROM product_option_groups g WHERE g."productId" = $1
               UNION ALL
               SELECT o.id FROM product_options o
               JOIN product_option_groups g ON g.id = o."groupId"
               WHERE g."productId" = $1"#,
        )
        .bind(product_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Replaces the whole recipe. Each group and option must already carry its final ID.
    pub async fn replace(&self, product_id: Uuid, recipe: &ProductRecipe) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(r#"DELETE FROM product_recipe_items WHERE "productId" = $1"#)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(r#"DELETE FROM product_option_groups WHERE "productId" = $1"#)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
        for item in &recipe.items {
            sqlx::query(
                r#"INSERT INTO product_recipe_items ("productId", "ingredientId", quantity)
                   VALUES ($1, $2, $3)"#,
            )
            .bind(product_id)
            .bind(item.ingredient_id)
            .bind(item.quantity)
            .execute(&mut *tx)
            .await?;
        }
        for (group_order, group) in recipe.option_groups.iter().enumerate() {
            Self::insert_group(&mut tx, product_id, group, group_order as i32).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn insert_group(
        tx: &mut Transaction<'_, Postgres>,
        product_id: Uuid,
        group: &ProductOptionGroup,
        sort_order: i32,
    ) -> Result<(), AppError> {
        let group_id = group
            .id
            .ok_or_else(|| AppError::Internal("Option group ID was not assigned".to_string()))?;
        sqlx::query(
            r#"INSERT INTO product_option_groups
                 (id, "productId", name, required, multiple, "sortOrder")
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(group_id)
        .bind(product_id)
        .bind(group.name.trim())
        .bind(group.required)
        .bind(group.multiple)
        .bind(sort_order)
        .execute(&mut **tx)
        .await?;
        for (option_order, option) in group.options.iter().enumerate() {
            let option_id = option
                .id
                .ok_or_else(|| AppError::Internal("Option ID was not assigned".to_string()))?;
            sqlx::query(
                r#"INSERT INTO product_options (id, "groupId", name, "priceDelta", "sortOrder")
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(option_id)
            .bind(group_id)
            .bind(option.name.trim())
            .bind(option.price_delta)
            .bind(option_order as i32)
            .execute(&mut **tx)
            .await?;
            for ingredient in &option.ingredients {
                sqlx::query(
                    r#"INSERT INTO product_option_ingredients ("optionId", "ingredientId", quantity)
                       VALUES ($1, $2, $3)"#,
                )
                .bind(option_id)
                .bind(ingredient.ingredient_id)
                .bind(ingredient.quantity)
                .execute(&mut **tx)
                .await?;
            }
        }
        Ok(())
    }

    /// Locks and returns stock with names for the given ingredients at a location.
    pub async fn lock_stock(
        tx: &mut Transaction<'_, Postgres>,
        location_id: Uuid,
        ingredient_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, (String, i32)>, AppError> {
        let rows: Vec<(Uuid, String, i32)> = sqlx::query_as(
            r#"SELECT p.id, p.name, COALESCE(ls."quantityPieces", 0)
               FROM products p
               LEFT JOIN location_stock ls
                 ON ls."productId" = p.id AND ls."locationId" = $1
               WHERE p.id = ANY($2)
               ORDER BY p.id
               FOR UPDATE OF p"#,
        )
        .bind(location_id)
        .bind(ingredient_ids)
        .fetch_all(&mut **tx)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, stock)| (id, (name, stock)))
            .collect())
    }

    /// Base-recipe ingredients and stock for every made-to-order product, used to show
    /// how many can be made right now.
    pub async fn made_to_order_stock(
        &self,
        location_id: Option<Uuid>,
    ) -> Result<Vec<(Uuid, i32, i32)>, AppError> {
        Ok(sqlx::query_as(
            r#"SELECT r."productId", r.quantity, COALESCE(ls."quantityPieces", 0)
               FROM product_recipe_items r
               LEFT JOIN location_stock ls
                 ON ls."productId" = r."ingredientId" AND ls."locationId" = $1"#,
        )
        .bind(location_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn with_option_groups(&self) -> Result<Vec<Uuid>, AppError> {
        Ok(
            sqlx::query_scalar(r#"SELECT DISTINCT "productId" FROM product_option_groups"#)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// Products that have a required option group.
    pub async fn with_required_options(&self) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar(
            r#"SELECT DISTINCT "productId" FROM product_option_groups WHERE required"#,
        )
        .fetch_all(&self.pool)
        .await?)
    }
}
