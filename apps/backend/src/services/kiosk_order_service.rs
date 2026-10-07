use crate::error::AppError;
use crate::models::{
    kiosk_order_status, ConvertKioskOrderDto, CreateKioskOrderDto, CreateLineItemDto,
    CreateTransactionDto, KioskMenuProduct, KioskOrderFilterDto, KioskOrderWithItems,
};
use crate::repositories::TenantKioskOrderRepository;
use crate::services::{ConfigService, TransactionService};
use crate::tenancy::TenantDb;
use std::sync::Arc;
use uuid::Uuid;

pub struct KioskOrderService {
    settings: Arc<ConfigService>,
}
impl KioskOrderService {
    pub fn new(settings: Arc<ConfigService>) -> Self {
        Self { settings }
    }
    pub async fn list_menu_tenant(
        &self,
        db: Arc<TenantDb>,
        device_id: Uuid,
        products: &crate::services::ProductService,
    ) -> Result<Vec<KioskMenuProduct>, AppError> {
        let device = crate::repositories::TenantDeviceRepository::new(db.clone())
            .find_by_id(device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Device not found".into()))?;
        let prices = products
            .current_prices_tenant(db.clone(), None, device.location_id, &self.settings)
            .await?;
        let rows: Vec<(Uuid,String,Option<String>,String,i32)> = sqlx::query_as("SELECT unhex(replace(p.id,'-','')),p.name,p.description,p.category,COALESCE(ls.quantity_pieces,0) FROM products p LEFT JOIN product_locations pl ON pl.product_id=p.id AND pl.location_id=? LEFT JOIN location_stock ls ON ls.product_id=p.id AND ls.inventory_location_id=(SELECT id FROM inventory_locations WHERE venue_location_id=? AND kind='store' AND is_active=1 AND deleted_at IS NULL ORDER BY id LIMIT 1) WHERE p.deleted_at IS NULL AND p.is_active=1 AND p.is_raw_material=0 AND (p.availability_scope='ALL' OR pl.product_id IS NOT NULL) AND NOT EXISTS(SELECT 1 FROM product_option_groups g WHERE g.product_id=p.id AND g.required=1) ORDER BY p.category,p.name,p.id")
            .bind(device.location_id.to_string()).bind(device.location_id.to_string()).fetch_all(&db.read_pool()?).await?;
        let prices = prices
            .into_iter()
            .map(|price| (price.product_id, price))
            .collect::<std::collections::HashMap<_, _>>();
        rows.into_iter()
            .map(|(id, name, description, category, stock)| {
                let price = prices.get(&id).ok_or_else(|| {
                    AppError::Conflict("Menu changed while loading prices".into())
                })?;
                let available = price.made_to_order_available.unwrap_or(stock);
                Ok(KioskMenuProduct {
                    id,
                    name,
                    description,
                    category,
                    price: price.price,
                    stock_available: available,
                    in_stock: available > 0,
                })
            })
            .collect()
    }

    pub async fn place_order_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
        device_id: Uuid,
        dto: CreateKioskOrderDto,
    ) -> Result<KioskOrderWithItems, AppError> {
        let timezone = db.timezone().await?;
        TenantKioskOrderRepository::new(db)
            .with_timezone(timezone)
            .place(player_id, device_id, dto)
            .await
    }

    pub async fn current_order_for_player_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
        device_id: Uuid,
    ) -> Result<Option<KioskOrderWithItems>, AppError> {
        TenantKioskOrderRepository::new(db)
            .current_for_player(player_id, device_id)
            .await
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: KioskOrderFilterDto,
    ) -> Result<crate::dto::PaginationResult<KioskOrderWithItems>, AppError> {
        TenantKioskOrderRepository::new(db).list(&filters).await
    }

    pub async fn get_by_id_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
    ) -> Result<KioskOrderWithItems, AppError> {
        TenantKioskOrderRepository::new(db).get(id).await
    }

    pub async fn update_status_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        status: &str,
    ) -> Result<KioskOrderWithItems, AppError> {
        TenantKioskOrderRepository::new(db)
            .update_status(id, status)
            .await
    }

    pub fn build_transaction_dto(
        order: &KioskOrderWithItems,
        convert: &ConvertKioskOrderDto,
    ) -> CreateTransactionDto {
        CreateTransactionDto {
            player_id: order.player_id,
            transaction_type: "product_purchase".to_string(),
            plan_id: None,
            shift_id: None,
            amount: None,
            payment_method: convert.payment_method.clone(),
            payment_status: convert.payment_status.clone(),
            notes: match (&order.player_note, &convert.notes) {
                (Some(player), Some(counter)) => {
                    Some(format!("Customer: {player}\nCounter: {counter}"))
                }
                (Some(player), None) => Some(format!("Customer: {player}")),
                (None, notes) => notes.clone(),
            },
            online_payment_ref_last4: convert.online_payment_ref_last4.clone(),
            transaction_date: None,
            cash_amount: convert.cash_amount,
            online_amount: convert.online_amount,
            line_items: Some(
                order
                    .line_items
                    .iter()
                    .map(|i| CreateLineItemDto {
                        product_id: i.product_id,
                        quantity: i.quantity,
                        unit_price: Some(i.unit_price),
                        option_ids: Vec::new(),
                    })
                    .collect(),
            ),
            sale_location_id: convert.sale_location_id,
            venue_location_id: None,
            kiosk_order_id: Some(order.id),
        }
    }

    pub async fn convert_to_sale_tenant(
        &self,
        db: Arc<TenantDb>,
        order_id: Uuid,
        convert: ConvertKioskOrderDto,
        shift_id: Uuid,
        actor_id: Uuid,
        transactions: &TransactionService,
    ) -> Result<crate::models::Transaction, AppError> {
        let repo = TenantKioskOrderRepository::new(db.clone());
        let order = repo.get(order_id).await?;
        if !kiosk_order_status::OPEN.contains(&order.status.as_str()) {
            return Err(AppError::Conflict(
                "Order is not open for conversion".to_string(),
            ));
        }
        let mut dto = Self::build_transaction_dto(&order, &convert);
        dto.venue_location_id = Some(repo.venue_for_order(order_id).await?);
        dto.shift_id = Some(shift_id);
        transactions.create_tenant(db, dto, Some(actor_id)).await
    }
}
