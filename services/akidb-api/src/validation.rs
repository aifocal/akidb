//! Input validation utilities

use crate::handlers::ApiError;

/// Validate collection name
pub fn validate_collection_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() {
        return Err(ApiError::Validation(
            "Collection name cannot be empty".to_string(),
        ));
    }

    if name.len() > 255 {
        return Err(ApiError::Validation(format!(
            "Collection name too long (max 255 characters, got {})",
            name.len()
        )));
    }

    // Check for valid characters (alphanumeric, underscore, hyphen)
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ApiError::Validation(
            "Collection name can only contain alphanumeric characters, underscores, and hyphens"
                .to_string(),
        ));
    }

    Ok(())
}

/// Validate vector dimension
pub fn validate_vector_dim(dim: u16) -> Result<(), ApiError> {
    if dim == 0 {
        return Err(ApiError::Validation(
            "Vector dimension must be greater than 0".to_string(),
        ));
    }

    if dim > 4096 {
        return Err(ApiError::Validation(format!(
            "Vector dimension too large (max 4096, got {})",
            dim
        )));
    }

    Ok(())
}

/// Validate top_k parameter
pub fn validate_top_k(top_k: u16) -> Result<(), ApiError> {
    if top_k == 0 {
        return Err(ApiError::Validation(
            "top_k must be greater than 0".to_string(),
        ));
    }

    if top_k > 1000 {
        return Err(ApiError::Validation(format!(
            "top_k too large (max 1000, got {})",
            top_k
        )));
    }

    Ok(())
}

/// Validate vector components
pub fn validate_vector(vector: &[f32], expected_dim: usize) -> Result<(), ApiError> {
    if vector.len() != expected_dim {
        return Err(ApiError::Validation(format!(
            "Vector dimension mismatch: expected {}, got {}",
            expected_dim,
            vector.len()
        )));
    }

    // Check for NaN or Inf
    for (idx, &value) in vector.iter().enumerate() {
        if !value.is_finite() {
            return Err(ApiError::Validation(format!(
                "Vector component at index {} is not finite (NaN or Inf)",
                idx
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_collection_name() {
        // Valid names
        assert!(validate_collection_name("test").is_ok());
        assert!(validate_collection_name("test_collection").is_ok());
        assert!(validate_collection_name("test-collection-123").is_ok());

        // Invalid names
        assert!(validate_collection_name("").is_err());
        assert!(validate_collection_name(&"a".repeat(256)).is_err());
        assert!(validate_collection_name("test collection").is_err()); // spaces
        assert!(validate_collection_name("test@collection").is_err()); // special chars
    }

    #[test]
    fn test_validate_vector_dim() {
        assert!(validate_vector_dim(128).is_ok());
        assert!(validate_vector_dim(384).is_ok());

        assert!(validate_vector_dim(0).is_err());
        assert!(validate_vector_dim(5000).is_err());
    }

    #[test]
    fn test_validate_top_k() {
        assert!(validate_top_k(10).is_ok());
        assert!(validate_top_k(100).is_ok());

        assert!(validate_top_k(0).is_err());
        assert!(validate_top_k(2000).is_err());
    }

    #[test]
    fn test_validate_vector() {
        let valid = vec![1.0, 2.0, 3.0];
        assert!(validate_vector(&valid, 3).is_ok());

        // Wrong dimension
        assert!(validate_vector(&valid, 4).is_err());

        // NaN
        let invalid = vec![1.0, f32::NAN, 3.0];
        assert!(validate_vector(&invalid, 3).is_err());

        // Inf
        let invalid = vec![1.0, f32::INFINITY, 3.0];
        assert!(validate_vector(&invalid, 3).is_err());
    }
}
