use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct AppError(String);

impl AppError {
    pub fn new<T: Into<String>>(message: T) -> Self {
        Self(message.into())
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for AppError {}
impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
