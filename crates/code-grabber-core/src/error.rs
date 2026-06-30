use thiserror::Error;

pub type Result<T> = std::result::Result<T, CodeGrabberError>;

#[derive(Debug, Error)]
pub enum CodeGrabberError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("glob error: {0}")]
    Glob(#[from] globset::Error),

    #[error("walk error: {0}")]
    Walk(#[from] ignore::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("TOML parse error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML write error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("tokenizer error: {0}")]
    Tokenizer(String),
}
