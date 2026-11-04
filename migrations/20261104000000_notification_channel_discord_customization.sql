-- Expand notification_channels table with bot_name and avatar_url for webhook profile customization
ALTER TABLE notification_channels ADD COLUMN bot_name TEXT;
ALTER TABLE notification_channels ADD COLUMN avatar_url TEXT;
