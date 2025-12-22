-- Migration to add flag column to photos table
-- Stores: 1 (Pick), -1 (Reject), NULL (Unflagged)

ALTER TABLE photos ADD COLUMN flag INTEGER;
