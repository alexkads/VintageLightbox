-- Add crop fields to photos table
ALTER TABLE photos ADD COLUMN edit_crop_x REAL;
ALTER TABLE photos ADD COLUMN edit_crop_y REAL;
ALTER TABLE photos ADD COLUMN edit_crop_width REAL;
ALTER TABLE photos ADD COLUMN edit_crop_height REAL;
ALTER TABLE photos ADD COLUMN edit_crop_rotation INTEGER; -- rotation in 90 degree steps (0, 1, 2, 3)
ALTER TABLE photos ADD COLUMN edit_crop_angle REAL; -- fine rotation angle (straighten)
ALTER TABLE photos ADD COLUMN edit_crop_flip_h BOOLEAN;
ALTER TABLE photos ADD COLUMN edit_crop_flip_v BOOLEAN;
