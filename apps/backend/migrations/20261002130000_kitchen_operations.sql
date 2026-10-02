-- Kitchen fulfillment is independent of payment status. Only opted-in products create tickets.
CREATE TABLE kitchen_menu_settings (
  product_id uuid PRIMARY KEY REFERENCES products(id),
  enabled boolean NOT NULL DEFAULT false,
  station text NOT NULL CHECK (length(station) BETWEEN 1 AND 60),
  prep_minutes integer NOT NULL CHECK (prep_minutes BETWEEN 1 AND 240),
  revision integer NOT NULL DEFAULT 1,
  updated_by uuid REFERENCES users(id),
  updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE kitchen_tickets (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  transaction_id uuid NOT NULL UNIQUE REFERENCES transactions(id),
  status text NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','preparing','ready','served','cancelled')),
  revision integer NOT NULL DEFAULT 1,
  items jsonb NOT NULL,
  customer text NOT NULL,
  notes text,
  created_at timestamptz NOT NULL DEFAULT now(),
  due_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX kitchen_tickets_queue ON kitchen_tickets(status, created_at);
CREATE TABLE kitchen_ticket_events (
  id bigserial PRIMARY KEY,
  ticket_id uuid NOT NULL REFERENCES kitchen_tickets(id),
  status text NOT NULL,
  actor_id uuid REFERENCES users(id),
  reason text,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX kitchen_ticket_events_ticket ON kitchen_ticket_events(ticket_id, id);
