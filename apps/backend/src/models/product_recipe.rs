use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Quantity is a whole number in the ingredient's own unit (pieces, grams, millilitres).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecipeIngredient {
    pub ingredient_id: Uuid,
    pub quantity: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProductOption {
    /// Omit when adding an option; keep it when editing so past sales stay linked.
    pub id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub price_delta: f64,
    /// Added to the base recipe. A negative quantity removes an ingredient.
    #[serde(default)]
    pub ingredients: Vec<RecipeIngredient>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProductOptionGroup {
    pub id: Option<Uuid>,
    pub name: String,
    /// At least one option must be chosen.
    #[serde(default)]
    pub required: bool,
    /// More than one option may be chosen.
    #[serde(default)]
    pub multiple: bool,
    pub options: Vec<ProductOption>,
}

/// With `items`, the product is made to order and a sale deducts these
/// ingredients instead of the product's own stock.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProductRecipe {
    #[serde(default)]
    pub items: Vec<RecipeIngredient>,
    #[serde(default)]
    pub option_groups: Vec<ProductOptionGroup>,
}

impl ProductRecipe {
    pub fn is_made_to_order(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.option_groups.is_empty()
    }

    pub fn has_required_options(&self) -> bool {
        self.option_groups.iter().any(|group| group.required)
    }
}

/// An option chosen on a sale line, copied so receipts survive recipe edits.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelectedOption {
    pub option_id: Uuid,
    pub group_name: String,
    pub name: String,
    pub price_delta: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProductCurrentPrice {
    pub product_id: Uuid,
    pub price: f64,
    pub has_options: bool,
    /// How many the base recipe can make at the requested location; absent when the
    /// product is sold from its own stock.
    pub made_to_order_available: Option<i32>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct CurrentPricesQuery {
    /// Sale location used for made-to-order availability.
    pub location_id: Option<Uuid>,
    pub venue_location_id: Option<Uuid>,
}
