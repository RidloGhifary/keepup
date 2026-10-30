-- Track monitor status state machine and down timestamps
ALTER TABLE monitors ADD COLUMN last_status TEXT NOT NULL DEFAULT 'UP';
ALTER TABLE monitors ADD COLUMN down_since DATETIME;

-- Add notify_on_resolve setting to webhooks table
ALTER TABLE webhooks ADD COLUMN notify_on_resolve INTEGER NOT NULL DEFAULT 1;
