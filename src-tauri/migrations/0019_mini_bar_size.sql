-- W6: user-chosen mini bar size. Both columns are NULL until the user resizes
-- the window, so the section-derived default size is used until then.
ALTER TABLE application_settings ADD COLUMN mini_bar_width INTEGER;
ALTER TABLE application_settings ADD COLUMN mini_bar_height INTEGER;