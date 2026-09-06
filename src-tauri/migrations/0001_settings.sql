CREATE TABLE application_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    close_to_tray INTEGER NOT NULL CHECK (close_to_tray IN (0, 1)),
    show_mascot INTEGER NOT NULL CHECK (show_mascot IN (0, 1)),
    friendly_messages INTEGER NOT NULL CHECK (friendly_messages IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO application_settings (id, close_to_tray, show_mascot, friendly_messages)
VALUES (1, 1, 1, 1);
