-- Add content_hash field to photos table
-- This field stores a SHA-256 hash of the file content for duplicate detection

ALTER TABLE photos ADD COLUMN content_hash TEXT;

-- Create index for fast duplicate lookup
CREATE INDEX IF NOT EXISTS idx_photos_content_hash ON photos(content_hash);
