-- Expand status_pages table with Search Engine Visibility (SEO & Indexing) fields
ALTER TABLE status_pages ADD COLUMN allow_indexing BOOLEAN NOT NULL DEFAULT 1;
ALTER TABLE status_pages ADD COLUMN meta_title TEXT;
ALTER TABLE status_pages ADD COLUMN meta_description TEXT;
ALTER TABLE status_pages ADD COLUMN og_image_url TEXT;
ALTER TABLE status_pages ADD COLUMN canonical_url TEXT;
