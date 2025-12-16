-- Initial schema for VintageLightbox
-- Photos table
CREATE TABLE IF NOT EXISTS photos (
    id TEXT PRIMARY KEY NOT NULL,
    file_path TEXT NOT NULL,
    rating INTEGER,
    color_label TEXT,
    is_edited BOOLEAN NOT NULL DEFAULT 0,
    imported_at TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

-- Collections table
CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

-- Collection-Photos junction table (many-to-many)
CREATE TABLE IF NOT EXISTS collection_photos (
    collection_id TEXT NOT NULL,
    photo_id TEXT NOT NULL,
    PRIMARY KEY (collection_id, photo_id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_photos_rating ON photos(rating);
CREATE INDEX IF NOT EXISTS idx_photos_color_label ON photos(color_label);
CREATE INDEX IF NOT EXISTS idx_collection_photos_photo_id ON collection_photos(photo_id);
CREATE INDEX IF NOT EXISTS idx_collection_photos_collection_id ON collection_photos(collection_id);
