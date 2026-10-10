-- Preserve the expense venue even when a receipt's inventory location moves.
ALTER TABLE expenses ADD COLUMN location_id TEXT REFERENCES venue_locations(id);
UPDATE expenses SET location_id=COALESCE(
    (SELECT o.location_id FROM outbox_events o
     WHERE o.aggregate_type='expense' AND o.aggregate_id=expenses.id
       AND o.event_type IN ('expense.created','expense.updated','expense.status_changed','expense.deleted')
     ORDER BY o.sequence DESC LIMIT 1),
    (SELECT s.location_id FROM shifts s WHERE s.id=expenses.shift_id));
CREATE INDEX expenses_location_date ON expenses(location_id,expense_date DESC,id DESC);
