-- Add template configuration to webhooks table
ALTER TABLE webhooks ADD COLUMN use_default_template INTEGER NOT NULL DEFAULT 1;
ALTER TABLE webhooks ADD COLUMN message_template TEXT;
