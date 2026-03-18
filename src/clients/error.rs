use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub struct HttpWithCode(reqwest::Error);

impl fmt::Display for HttpWithCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Note: status() is Some(...) only for HTTP responses (e.g. after error_for_status())
        write!(f, "HTTP error: {} code: {:?}", self.0, self.0.status())
    }
}

impl StdError for HttpWithCode {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.0)
    }
}

impl From<reqwest::Error> for HttpWithCode {
    fn from(e: reqwest::Error) -> Self {
        Self(e)
    }
}
