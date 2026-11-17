ALTER TABLE monitors ADD COLUMN http_method TEXT NOT NULL DEFAULT 'GET';
ALTER TABLE monitors ADD COLUMN http_headers_json TEXT;
ALTER TABLE monitors ADD COLUMN http_body TEXT;
ALTER TABLE monitors ADD COLUMN expected_status_codes TEXT NOT NULL DEFAULT '200-299';
ALTER TABLE monitors ADD COLUMN response_keyword TEXT;
ALTER TABLE monitors ADD COLUMN keyword_match_mode TEXT NOT NULL DEFAULT 'contains';
ALTER TABLE monitors ADD COLUMN follow_redirects INTEGER NOT NULL DEFAULT 1;
ALTER TABLE monitors ADD COLUMN ignore_tls_errors INTEGER NOT NULL DEFAULT 0;
ALTER TABLE monitors ADD COLUMN degraded_threshold_ms INTEGER;
