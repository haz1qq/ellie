use serde::{Deserialize, Serialize};

/// Only non-sensitive, implemented presentation preferences cross IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub close_to_tray: bool,
    pub show_mascot: bool,
    pub friendly_messages: bool,
}

#[cfg(test)]
mod tests {
    use super::Settings;

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
