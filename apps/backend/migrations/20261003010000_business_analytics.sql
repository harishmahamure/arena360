-- Extend analytics projections and refresh existing rows without editing source records.
-- Run with the upgraded worker stopped; restart it after this migration commits.
LOCK TABLE transactions, usage_sessions, devices, plans, player_plan_balances, transaction_products, shifts, games IN SHARE ROW EXCLUSIVE MODE;
DROP TRIGGER IF EXISTS analytics_capture ON transactions;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON transactions
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,playerId,planId,amount,paymentMethod,paymentStatus,transactionType,cashAmount,onlineAmount,paidAmount,transactionDate,shiftId,createdBy');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'transactions', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,createdAt,updatedAt,deletedAt,playerId,planId,amount,paymentMethod,paymentStatus,transactionType,cashAmount,onlineAmount,paidAmount,transactionDate,shiftId,createdBy',','))) FROM transactions t;
DROP TRIGGER IF EXISTS analytics_capture ON usage_sessions;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON usage_sessions
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,deviceId,balanceId,startTime,endTime,durationMinutes,shiftId,createdBy,sourcePlanIdAtStart');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'usage_sessions', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,createdAt,updatedAt,deletedAt,deviceId,balanceId,startTime,endTime,durationMinutes,shiftId,createdBy,sourcePlanIdAtStart',','))) FROM usage_sessions t;
DROP TRIGGER IF EXISTS analytics_capture ON devices;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON devices
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name,status,location,deviceType');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'devices', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,createdAt,updatedAt,deletedAt,name,status,location,deviceType',','))) FROM devices t;
DROP TRIGGER IF EXISTS analytics_capture ON plans;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON plans
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name,price,timeCredits,planType,validityDays');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'plans', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,createdAt,updatedAt,deletedAt,name,price,timeCredits,planType,validityDays',','))) FROM plans t;
DROP TRIGGER IF EXISTS analytics_capture ON player_plan_balances;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON player_plan_balances
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,playerId,status,kind,remainingMinutes,expiryDate,sourcePlanId');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'player_plan_balances', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,createdAt,updatedAt,deletedAt,playerId,status,kind,remainingMinutes,expiryDate,sourcePlanId',','))) FROM player_plan_balances t;
DROP TRIGGER IF EXISTS analytics_capture ON transaction_products;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON transaction_products
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,transactionId,productId,quantity,unitPrice,createdAt,updatedAt');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'transaction_products', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,transactionId,productId,quantity,unitPrice,createdAt,updatedAt',','))) FROM transaction_products t;
DROP TRIGGER IF EXISTS analytics_capture ON shifts;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON shifts
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,userId,clockIn,clockOut,status,createdAt,updatedAt');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'shifts', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,userId,clockIn,clockOut,status,createdAt,updatedAt',','))) FROM shifts t;
DROP TRIGGER IF EXISTS analytics_capture ON games;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON games
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,name,isActive,createdAt,updatedAt,deletedAt');
INSERT INTO analytics_outbox(source_table,row_id,row_data)
 SELECT 'games', t.id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t)) WHERE key=ANY(string_to_array('id,name,isActive,createdAt,updatedAt,deletedAt',','))) FROM games t;
INSERT INTO analytics_outbox(source_table,row_id,row_data) VALUES('__ready_business','00000000-0000-0000-0000-000000000002','{}');
