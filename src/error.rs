/// Errors produced by this crate.
///
/// `#[non_exhaustive]`: new variants may be added in minor releases —
/// match with a wildcard arm.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IconError {
    #[error("failed to extract icon: {0}")]
    Extract(String),
    #[error("encoding failed: {0}")]
    Encode(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache error: {0}")]
    Cache(String),
    #[error("resize failed: {0}")]
    Resize(String),
}
