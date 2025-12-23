-- Create presets table
CREATE TABLE IF NOT EXISTS presets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    is_system BOOLEAN NOT NULL DEFAULT 0,
    exposure REAL,
    contrast REAL,
    temperature REAL,
    tint REAL,
    highlights REAL,
    shadows REAL,
    whites REAL,
    blacks REAL,
    clarity REAL,
    vibrance REAL,
    saturation REAL,
    tone_curve_shadows REAL,
    tone_curve_darks REAL,
    tone_curve_lights REAL,
    tone_curve_highlights REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Index for faster lookup by name? Not strictly necessary for MVP but good practice.
CREATE INDEX IF NOT EXISTS idx_presets_name ON presets(name);
