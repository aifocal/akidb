//! Tier management API handlers (Phase 10 Week 3)
//!
//! Provides endpoints for monitoring and controlling collection tiering:
//! - GET /collections/{id}/tier - Get tier status
//! - POST /collections/{id}/tier - Manual tier control (admin)
//! - GET /metrics/tiers - Tier distribution stats

use akidb_core::CollectionId;
use akidb_service::CollectionService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

/// Tier status response
#[derive(Serialize)]
pub struct TierStatusResponse {
    pub collection_id: String,
    pub tier: String,
    pub last_accessed_at: String,
    pub access_count: u32,
    pub pinned: bool,
    pub snapshot_id: Option<String>,
    pub warm_file_path: Option<String>,
}

/// Tier action for manual control
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierAction {
    PromoteToHot,
    DemoteToWarm,
    DemoteToRold,
    Pin,
    Unpin,
}

/// Update tier request
#[derive(Deserialize)]
pub struct UpdateTierRequest {
    pub action: TierAction,
}

/// Tier metrics response
#[derive(Serialize)]
pub struct TierMetrics {
    pub hot_count: usize,
    pub warm_count: usize,
    pub cold_count: usize,
    pub total_collections: usize,
}

/// Get collection tier status
///
/// Returns current tier, access stats, and pinning status
#[tracing::instrument(skip(service), fields(collection_id = %collection_id))]
pub async fn get_collection_tier(
    Path(collection_id): Path<String>,
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<TierStatusResponse>, (StatusCode, String)> {
    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    // Check if tiering is enabled
    let tiering_manager = service.tiering_manager().ok_or_else(|| {
        (
            StatusCode::NOT_IMPLEMENTED,
            "Tiering not enabled".to_string(),
        )
    })?;

    // Get tier state
    let tier_state = tiering_manager
        .get_tier_state(collection_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                (StatusCode::NOT_FOUND, e.to_string())
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?;

    Ok(Json(TierStatusResponse {
        collection_id: collection_id.to_string(),
        tier: tier_state.tier.to_string(),
        last_accessed_at: tier_state.last_accessed_at.to_rfc3339(),
        access_count: tier_state.access_count,
        pinned: tier_state.pinned,
        snapshot_id: tier_state.snapshot_id.map(|id| id.to_string()),
        warm_file_path: tier_state.warm_file_path,
    }))
}

/// Update collection tier (manual control)
///
/// Allows administrators to manually promote/demote collections or pin/unpin them.
/// Pinned collections will not be automatically demoted.
#[tracing::instrument(skip(service, req), fields(collection_id = %collection_id, action = ?req.action))]
pub async fn update_collection_tier(
    Path(collection_id): Path<String>,
    State(service): State<Arc<CollectionService>>,
    Json(req): Json<UpdateTierRequest>,
) -> Result<Json<TierStatusResponse>, (StatusCode, String)> {
    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    // Check if tiering is enabled
    let tiering_manager = service.tiering_manager().ok_or_else(|| {
        (
            StatusCode::NOT_IMPLEMENTED,
            "Tiering not enabled".to_string(),
        )
    })?;

    // Execute action
    match req.action {
        TierAction::PromoteToHot => {
            tiering_manager
                .promote_from_warm(collection_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        TierAction::DemoteToWarm => {
            // Note: demote_to_warm is private in TieringManager
            // This would need to be exposed or we use a different approach
            return Err((
                StatusCode::NOT_IMPLEMENTED,
                "Manual demotion to warm not yet implemented".to_string(),
            ));
        }
        TierAction::DemoteToRold => {
            // Note: demote_to_cold is private in TieringManager
            return Err((
                StatusCode::NOT_IMPLEMENTED,
                "Manual demotion to cold not yet implemented".to_string(),
            ));
        }
        TierAction::Pin => {
            // Note: pin_collection needs to be exposed in TieringManager
            return Err((
                StatusCode::NOT_IMPLEMENTED,
                "Pin/unpin not yet implemented in TieringManager API".to_string(),
            ));
        }
        TierAction::Unpin => {
            return Err((
                StatusCode::NOT_IMPLEMENTED,
                "Pin/unpin not yet implemented in TieringManager API".to_string(),
            ));
        }
    }

    // Return updated status
    get_collection_tier(Path(collection_id.to_string()), State(service)).await
}

/// Get tier distribution metrics
///
/// Returns counts of collections in each tier.
/// Note: This is a simplified implementation. A full implementation would
/// query all collections from the tier state repository.
pub async fn get_tier_metrics(
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<TierMetrics>, (StatusCode, String)> {
    // Check if tiering is enabled
    let _tiering_manager = service.tiering_manager().ok_or_else(|| {
        (
            StatusCode::NOT_IMPLEMENTED,
            "Tiering not enabled".to_string(),
        )
    })?;

    // TODO: Implement actual tier counting by querying TierStateRepository
    // For now, return placeholder values
    Ok(Json(TierMetrics {
        hot_count: 0,
        warm_count: 0,
        cold_count: 0,
        total_collections: 0,
    }))
}
