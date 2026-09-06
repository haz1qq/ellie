use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppError {
    #[error("local storage is unavailable")]
    Storage,
    #[error("the database was created by a newer Ellie version")]
    NewerDatabase,
    #[error("a background operation failed")]
    Background,
    #[error("the desktop window operation failed")]
    Window,
    #[error("desktop initialization failed")]
    Startup,
}

impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
