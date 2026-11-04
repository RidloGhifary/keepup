-- Expand notification_channels table with Telegram-specific configuration fields
ALTER TABLE notification_channels ADD COLUMN telegram_bot_token TEXT;
ALTER TABLE notification_channels ADD COLUMN telegram_chat_id TEXT;
ALTER TABLE notification_channels ADD COLUMN telegram_thread_id TEXT;
