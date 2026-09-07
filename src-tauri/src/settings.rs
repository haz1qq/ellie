use serde::{Deserialize, Serialize};

/// Only non-sensitive, implemented presentation preferences cross IPC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub close_to_tray: bool,
    pub show_mascot: bool,
    pub friendly_messages: bool,
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    #[serde(default = "default_notification_thresholds")]
    pub notification_thresholds: [f64; 3],
    #[serde(default)]
    pub hidden_provider_ids: Vec<String>,
}

fn default_notifications_enabled() -> bool {
    true
}

pub const DEFAULT_NOTIFICATION_THRESHOLDS: [f64; 3] = [75.0, 90.0, 95.0];

fn default_notification_thresholds() -> [f64; 3] {
    DEFAULT_NOTIFICATION_THRESHOLDS
}

impl Settings {
    pub fn validate(&self) -> Result<(), crate::error::AppError> {
        if !self
            .notification_thresholds
            .iter()
            .all(|value| *value > 0.0 && *value <= 100.0 && value.is_finite())
            || self
                .notification_thresholds
                .windows(2)
                .any(|values| values[0] >= values[1])
        {
            return Err(crate::error::AppError::Storage);
        }
        let mut seen = std::collections::BTreeSet::new();
        if self.hidden_provider_ids.len() > 64
            || self.hidden_provider_ids.iter().any(|id| {
                id.is_empty()
                    || id.len() > 64
                    || !id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
                    || !seen.insert(id)
            })
        {
            return Err(crate::error::AppError::Storage);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn visibility_round_trips_and_older_settings_default_to_visible() {
        let mut settings: Settings = serde_json::from_str(
            r#"{"closeToTray":true,"showMascot":true,"friendlyMessages":true}"#,
        )
        .expect("old preferences");
        assert!(settings.notifications_enabled);
        assert_eq!(
            settings.notification_thresholds,
            super::DEFAULT_NOTIFICATION_THRESHOLDS
        );
        assert!(settings.hidden_provider_ids.is_empty());
        settings.hidden_provider_ids = vec!["ellie-demo".into()];
        assert!(settings.validate().is_ok());
        let json = serde_json::to_value(&settings).expect("serialize");
        assert_eq!(json["hiddenProviderIds"], serde_json::json!(["ellie-demo"]));
        assert_eq!(
            serde_json::from_value::<Settings>(json).expect("deserialize"),
            settings
        );
        for invalid in ["null", "false", "[1]", "\"ellie-demo\""] {
            let json = format!(
                r#"{{"closeToTray":true,"showMascot":true,"friendlyMessages":true,"hiddenProviderIds":{invalid}}}"#
            );
            assert!(serde_json::from_str::<Settings>(&json).is_err());
        }
    }

    #[test]
    fn rejects_invalid_notification_thresholds() {
        for thresholds in [[0.0, 90.0, 95.0], [75.0, 75.0, 95.0], [75.0, 90.0, 101.0]] {
            let settings = Settings {
                close_to_tray: true,
                show_mascot: true,
                friendly_messages: true,
                notifications_enabled: true,
                notification_thresholds: thresholds,
                hidden_provider_ids: vec![],
            };
            assert!(settings.validate().is_err());
        }
    }

    #[test]
    fn rejects_unknown_fields_and_invalid_boolean_types() {
        assert!(serde_json::from_str::<Settings>(
            r#"{"closeToTray":true,"showMascot":true,"friendlyMessages":true,"apiKey":"fixture"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<Settings>(
            r#"{"closeToTray":1,"showMascot":true,"friendlyMessages":true}"#
        )
        .is_err());
    }
}
