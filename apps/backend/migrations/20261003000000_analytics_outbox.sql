-- Transactional CDC: captures committed inserts, updates, and hard deletes.
-- Deliberately separate from websocket outbox and its retention policy.
CREATE TABLE analytics_outbox (
 id BIGSERIAL PRIMARY KEY,
 source_table TEXT NOT NULL,
 row_id UUID NOT NULL,
 row_data JSONB NOT NULL,
 version BIGINT CHECK (version = 0), -- NULL uses the outbox ID; zero is a backfill snapshot
 deleted BOOLEAN NOT NULL DEFAULT false,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE FUNCTION analytics_capture_change() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE payload JSONB; projected JSONB; old_payload JSONB; old_projected JSONB;
BEGIN
 payload := CASE WHEN TG_OP = 'DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 IF TG_TABLE_NAME = 'location_stock' THEN
   payload := payload || jsonb_build_object('id', md5((payload->>'locationId') || ':' || (payload->>'productId'))::uuid);
 END IF;
 SELECT jsonb_object_agg(key, value) INTO projected
 FROM jsonb_each(payload) WHERE key = ANY(string_to_array(TG_ARGV[0], ','));
 IF TG_OP = 'UPDATE' THEN
   old_payload := to_jsonb(OLD);
   IF TG_TABLE_NAME = 'location_stock' THEN
     old_payload := old_payload || jsonb_build_object('id', md5((old_payload->>'locationId') || ':' || (old_payload->>'productId'))::uuid);
   END IF;
   SELECT jsonb_object_agg(key,value) INTO old_projected FROM jsonb_each(old_payload)
     WHERE key = ANY(string_to_array(TG_ARGV[0], ','));
   IF projected = old_projected THEN RETURN NEW; END IF;
   IF old_payload->>'id' <> payload->>'id' THEN
     INSERT INTO analytics_outbox(source_table,row_id,row_data,deleted)
       VALUES(TG_TABLE_NAME,(old_payload->>'id')::uuid,old_projected,true);
   END IF;
 END IF;
 INSERT INTO analytics_outbox(source_table, row_id, row_data, deleted)
 VALUES(TG_TABLE_NAME, (payload->>'id')::uuid, projected, TG_OP = 'DELETE');
 RETURN NULL;
END $$;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON transactions
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,playerId,planId,amount,paymentMethod,paymentStatus,transactionType,cashAmount,onlineAmount,paidAmount,transactionDate');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON credit_settlements
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,playerId,amount,paymentMethod,cashAmount,onlineAmount,settledAt');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON credit_settlement_items
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,settlementId,transactionId,amountApplied');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON usage_sessions
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,deviceId,balanceId,startTime,endTime,durationMinutes');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON users
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,username,firstName,lastName,role,isActive,creditLimit');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON plans
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON player_plan_balances
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,playerId,status');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON devices
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name,status');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON cash_registers
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,shiftId,status,variance,closingBalance,expectedClosing');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON cash_deposits
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,status,amount,depositType');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON expenses
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,categoryId,amount,approvalStatus,expenseDate');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON expense_categories
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name,isActive,budgetAmount,budgetPeriod');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON products
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name,purchasePricePerBox,purchasePrice,unitsPerPurchaseUnit');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON vendors
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON inventory_locations
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON stock_receipts
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,vendorId,locationId');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON stock_receipt_lines
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,receiptId,productId,boxQuantity,piecesAdded');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON stock_waste_events
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,locationId,status,approvedAt');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON stock_waste_lines
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,wasteEventId,productId,reasonCode,quantityPieces');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON location_stock
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,locationId,productId,quantityPieces');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON inventory_reorder_rules
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,locationId,productId,isActive,minimumPieces');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON purchase_orders
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,status');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON stock_transfer_requests
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,status');
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON stock_movements
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,locationId,productId,delta,movementType,referenceId,referenceType,createdBy');
