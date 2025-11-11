-- Migration: Fix tier state trigger timestamp format
-- Created: 2025-11-09
-- Phase 10 Week 2 Bug Fix
--
-- The auto-update trigger was using CURRENT_TIMESTAMP which produces
-- SQLite default format (YYYY-MM-DD HH:MM:SS) instead of RFC3339 format.
-- Since we manually set updated_at in the code anyway, we can drop the trigger.

DROP TRIGGER IF EXISTS update_tier_state_timestamp;
