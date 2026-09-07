ALTER TABLE application_settings ADD COLUMN notification_thresholds TEXT NOT NULL DEFAULT '[75,90,95]'
    CHECK (
        json_valid(notification_thresholds)
        AND json_type(notification_thresholds) = 'array'
    );
