-- Stop the worker before rollback. Keep replicated facts for audit/recovery.
DROP TRIGGER IF EXISTS analytics_capture ON transactions;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON transactions
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,playerId,planId,amount,paymentMethod,paymentStatus,transactionType,cashAmount,onlineAmount,paidAmount,transactionDate');
DROP TRIGGER IF EXISTS analytics_capture ON usage_sessions;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON usage_sessions
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,deviceId,balanceId,startTime,endTime,durationMinutes');
DROP TRIGGER IF EXISTS analytics_capture ON devices;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON devices
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name,status');
DROP TRIGGER IF EXISTS analytics_capture ON plans;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON plans
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,name');
DROP TRIGGER IF EXISTS analytics_capture ON player_plan_balances;
CREATE TRIGGER analytics_capture AFTER INSERT OR UPDATE OR DELETE ON player_plan_balances
 FOR EACH ROW EXECUTE FUNCTION analytics_capture_change('id,createdAt,updatedAt,deletedAt,playerId,status');
DROP TRIGGER IF EXISTS analytics_capture ON transaction_products;
DROP TRIGGER IF EXISTS analytics_capture ON shifts;
DROP TRIGGER IF EXISTS analytics_capture ON games;
