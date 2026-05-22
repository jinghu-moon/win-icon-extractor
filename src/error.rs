#[derive(Debug, thiserror::Error)]
pub enum IconError {
    #[error("failed to extract icon: {0}")]
    Extract(String),
    #[error("encoding failed: {0}")]
    Encode(String),
    #[error("decoding failed: {0}")]
    Decode(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache error: {0}")]
    Cache(String),
}
