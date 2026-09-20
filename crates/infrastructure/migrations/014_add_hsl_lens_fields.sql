-- Add HSL Hue fields
ALTER TABLE photos ADD COLUMN edit_hsl_red_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_orange_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_yellow_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_green_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_aqua_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_blue_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_purple_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_magenta_hue REAL DEFAULT 0.0;

-- Add HSL Luminance fields
ALTER TABLE photos ADD COLUMN edit_hsl_red_lum REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_orange_lum REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_yellow_lum REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_green_lum REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_aqua_lum REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_blue_lum REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_purple_lum REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_hsl_magenta_lum REAL DEFAULT 0.0;

-- Add Lens Correction fields
ALTER TABLE photos ADD COLUMN edit_lens_distortion REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_lens_vignette_amount REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_lens_vignette_midpoint REAL DEFAULT 50.0;
