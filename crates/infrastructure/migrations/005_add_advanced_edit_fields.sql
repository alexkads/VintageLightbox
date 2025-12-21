-- Add advanced edit fields to photos table
ALTER TABLE photos ADD COLUMN edit_temperature REAL;
ALTER TABLE photos ADD COLUMN edit_tint REAL;
ALTER TABLE photos ADD COLUMN edit_highlights REAL;
ALTER TABLE photos ADD COLUMN edit_shadows REAL;
ALTER TABLE photos ADD COLUMN edit_whites REAL;
ALTER TABLE photos ADD COLUMN edit_blacks REAL;
ALTER TABLE photos ADD COLUMN edit_clarity REAL;
ALTER TABLE photos ADD COLUMN edit_vibrance REAL;
ALTER TABLE photos ADD COLUMN edit_saturation REAL;
