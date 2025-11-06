use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use akidb_core::{TenantDescriptor, TenantId, TenantQuota, TenantStatus};
use akidb_metadata::{create_sqlite_pool, run_migrations, SqliteTenantCatalog};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Input options for migrating legacy tenant data into the SQLite metadata store.
pub struct MigrationOptions {
    /// Directory containing legacy tenant JSON manifests.
    pub source_dir: PathBuf,
    /// SQLite database URL (e.g. `sqlite:///path/to/metadata.db`).
    pub database_url: String,
}

/// Migrates tenants stored in the legacy JSON format into the SQLite metadata store.
pub async fn migrate_v1_tenants(options: MigrationOptions) -> Result<()> {
    if !options.source_dir.exists() {
        return Err(anyhow!(
            "source directory {} does not exist",
            options.source_dir.display()
        ));
    }

    let mut tenant_files = collect_manifest_files(&options.source_dir)?;
    tenant_files.sort();
    if tenant_files.is_empty() {
        return Err(anyhow!(
            "no tenant manifests found under {}",
            options.source_dir.display()
        ));
    }

    let pool = create_sqlite_pool(&options.database_url)
        .await
        .context("failed to create SQLite pool")?;
    run_migrations(&pool)
        .await
        .context("failed to apply metadata migrations")?;

    let mut tx = pool.begin().await.context("failed to open transaction")?;

    let mut used_slugs = HashSet::new();
    let mut inserted = 0_u64;

    for manifest in tenant_files {
        let legacy = read_legacy_tenant(&manifest)?;
        let mut tenant = convert_tenant(&legacy)?;
        tenant.slug =
            ensure_unique_slug(tenant.slug, &mut used_slugs, &tenant.tenant_id.to_string());

        SqliteTenantCatalog::create_with_executor(tx.as_mut(), &tenant)
            .await
            .map_err(|err| anyhow!("failed to insert tenant {}: {}", tenant.tenant_id, err))?;
        inserted += 1;
    }

    let db_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) as count
          FROM tenants
        "#,
    )
    .fetch_one(tx.as_mut())
    .await
    .context("failed to verify tenant count")?;

    if db_count != inserted as i64 {
        return Err(anyhow!(
            "migrated tenant count ({inserted}) does not match database count ({db_count})"
        ));
    }

    tx.commit()
        .await
        .context("migration transaction commit failed")?;
    Ok(())
}

fn collect_manifest_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    fn recurse(acc: &mut Vec<PathBuf>, dir: &Path) -> Result<()> {
        for entry in fs::read_dir(dir)
            .with_context(|| format!("failed to read directory {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                recurse(acc, &path)?;
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
            {
                acc.push(path);
            }
        }
        Ok(())
    }
    recurse(&mut files, root)?;
    Ok(files)
}

fn read_legacy_tenant(path: &Path) -> Result<LegacyTenantDescriptor> {
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read tenant manifest {}", path.display()))?;
    serde_json::from_str(&payload)
        .with_context(|| format!("failed to parse legacy tenant manifest {}", path.display()))
}

fn convert_tenant(legacy: &LegacyTenantDescriptor) -> Result<TenantDescriptor> {
    let uuid = parse_legacy_uuid(&legacy.tenant_id)
        .with_context(|| format!("invalid legacy tenant_id {}", legacy.tenant_id))?;
    let tenant_id = TenantId::from_uuid(uuid);

    let status = match legacy.status {
        LegacyTenantStatus::Active => TenantStatus::Active,
        LegacyTenantStatus::Suspended => TenantStatus::Suspended,
        LegacyTenantStatus::Deleted => TenantStatus::Decommissioned,
    };

    let quotas = TenantQuota {
        memory_quota_bytes: TenantQuota::DEFAULT_MEMORY_BYTES,
        storage_quota_bytes: legacy.quotas.max_storage_bytes,
        qps_quota: legacy.quotas.api_rate_limit_per_second,
    };

    let mut metadata = serde_json::to_value(&legacy.metadata)
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
    if let Value::Object(ref mut obj) = metadata {
        obj.insert(
            "legacy_tenant_id".to_string(),
            Value::String(legacy.tenant_id.clone()),
        );
    }

    let slug = slugify(&legacy.name);

    Ok(TenantDescriptor {
        tenant_id,
        name: legacy.name.clone(),
        slug,
        status,
        quotas,
        metadata,
        created_at: legacy.created_at,
        updated_at: legacy.updated_at,
    })
}

fn ensure_unique_slug(base_slug: String, used: &mut HashSet<String>, fallback: &str) -> String {
    let base = if base_slug.is_empty() {
        fallback_slug(fallback)
    } else {
        base_slug
    };
    let mut candidate = base.clone();
    let mut index = 1_u32;
    while !used.insert(candidate.clone()) {
        candidate = format!("{}-{}", base, index);
        index += 1;
    }
    candidate
}

fn fallback_slug(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect();
    if sanitized.is_empty() {
        "tenant".to_string()
    } else {
        format!("tenant-{}", sanitized.to_lowercase())
    }
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if matches!(ch, ' ' | '-' | '_' | '.') && !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn parse_legacy_uuid(input: &str) -> Result<Uuid> {
    let trimmed = input.strip_prefix("tenant_").unwrap_or(input).trim();
    if trimmed.len() == 32 {
        let formatted = format!(
            "{:8}-{:4}-{:4}-{:4}-{:12}",
            &trimmed[0..8],
            &trimmed[8..12],
            &trimmed[12..16],
            &trimmed[16..20],
            &trimmed[20..32]
        );
        Uuid::parse_str(&formatted).context("failed to parse compact UUID")
    } else {
        Uuid::parse_str(trimmed).context("failed to parse UUID")
    }
}

#[derive(Debug, Deserialize)]
struct LegacyTenantDescriptor {
    tenant_id: String,
    name: String,
    #[serde(default)]
    status: LegacyTenantStatus,
    #[serde(default)]
    quotas: LegacyTenantQuota,
    #[serde(default)]
    metadata: LegacyTenantMetadata,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LegacyTenantStatus {
    Active,
    Suspended,
    Deleted,
}

impl Default for LegacyTenantStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Deserialize)]
struct LegacyTenantQuota {
    max_storage_bytes: u64,
    #[allow(dead_code)]
    max_collections: u32,
    #[allow(dead_code)]
    max_vectors_per_collection: u64,
    api_rate_limit_per_second: u32,
    #[allow(dead_code)]
    max_concurrent_searches: u32,
}

impl Default for LegacyTenantQuota {
    fn default() -> Self {
        Self {
            max_storage_bytes: TenantQuota::DEFAULT_STORAGE_BYTES,
            max_collections: 0,
            max_vectors_per_collection: 0,
            api_rate_limit_per_second: TenantQuota::DEFAULT_QPS,
            max_concurrent_searches: 0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct LegacyTenantMetadata {
    #[serde(default)]
    contact_email: Option<String>,
    #[serde(default)]
    billing_plan: Option<String>,
    #[serde(default)]
    organization: Option<String>,
    #[serde(default)]
    custom: serde_json::Map<String, Value>,
}

impl Default for LegacyTenantMetadata {
    fn default() -> Self {
        Self {
            contact_email: None,
            billing_plan: None,
            organization: None,
            custom: serde_json::Map::new(),
        }
    }
}
