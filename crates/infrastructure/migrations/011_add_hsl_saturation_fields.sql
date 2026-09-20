-- Migration 011: Add HSL saturation fields to photos table
-- 8 color channels for selective saturation adjustment

ALTER TABLE photos ADD COLUMN edit_hsl_red_sat REAL;
ALTER TABLE photos ADD COLUMN edit_hsl_orange_sat REAL;
ALTER TABLE photos ADD COLUMN edit_hsl_yellow_sat REAL;
ALTER TABLE photos ADD COLUMN edit_hsl_green_sat REAL;
ALTER TABLE photos ADD COLUMN edit_hsl_aqua_sat REAL;
ALTER TABLE photos ADD COLUMN edit_hsl_blue_sat REAL;
ALTER TABLE photos ADD COLUMN edit_hsl_purple_sat REAL;
ALTER TABLE photos ADD COLUMN edit_hsl_magenta_sat REAL;
