-- Add tone curve fields to photos table
ALTER TABLE photos ADD COLUMN edit_tone_curve_shadows REAL;
ALTER TABLE photos ADD COLUMN edit_tone_curve_darks REAL;
ALTER TABLE photos ADD COLUMN edit_tone_curve_lights REAL;
ALTER TABLE photos ADD COLUMN edit_tone_curve_highlights REAL;
