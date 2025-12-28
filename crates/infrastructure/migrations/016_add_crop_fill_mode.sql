-- Add fill mode for rotation empty areas
ALTER TABLE photos ADD COLUMN edit_crop_fill_mode INTEGER DEFAULT 0;
