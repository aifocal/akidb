use std::path::PathBuf;

use akidb_core::{
    generate_api_key, hash_api_key, Action, ApiKeyDescriptor, ApiKeyRepository, AuditLogEntry,
    AuditLogRepository, AuditResult, CollectionDescriptor, CollectionRepository, CoreError,
    DatabaseDescriptor, DatabaseRepository, DatabaseState, DistanceMetric, Role, TenantCatalog,
    TenantDescriptor, TenantStatus, UserDescriptor, UserRepository, UserStatus,
};
use akidb_metadata::{
    create_sqlite_pool, password, run_migrations, SqliteApiKeyRepository, SqliteAuditLogRepository,
    SqliteCollectionRepository, SqliteDatabaseRepository, SqliteTenantCatalog,
    SqliteUserRepository,
};
use uuid::Uuid;

struct TestContext {
    catalog: SqliteTenantCatalog,
    databases: SqliteDatabaseRepository,
    collections: SqliteCollectionRepository,
    users: SqliteUserRepository,
    audit_logs: SqliteAuditLogRepository,
    api_keys: SqliteApiKeyRepository,
}

async fn setup_context() -> TestContext {
    let db_path = temp_db_path();
    let database_url = format!("sqlite://{}", db_path.display());
    let pool = create_sqlite_pool(&database_url)
        .await
        .expect("failed to create pool");
    run_migrations(&pool).await.expect("failed migrations");

    TestContext {
        catalog: SqliteTenantCatalog::new(pool.clone()),
        databases: SqliteDatabaseRepository::new(pool.clone()),
        collections: SqliteCollectionRepository::new(pool.clone()),
        users: SqliteUserRepository::new(pool.clone()),
        audit_logs: SqliteAuditLogRepository::new(pool.clone()),
        api_keys: SqliteApiKeyRepository::new(pool),
    }
}

fn temp_db_path() -> PathBuf {
    let filename = format!("akidb-metadata-test-{}.db", Uuid::now_v7());
    std::env::temp_dir().join(filename)
}

#[tokio::test]
async fn create_tenant_successfully() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Aki Labs", "aki-labs");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let fetched = ctx
        .catalog
        .get(tenant.tenant_id)
        .await
        .expect("get tenant")
        .expect("tenant present");
    assert_eq!(fetched.name, "Aki Labs");
    assert_eq!(fetched.slug, "aki-labs");
}

#[tokio::test]
async fn get_tenant_by_id() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Delta Corp", "delta");
    ctx.catalog.create(&tenant).await.expect("insert tenant");

    let maybe = ctx.catalog.get(tenant.tenant_id).await.expect("fetch");
    assert!(maybe.is_some());
}

#[tokio::test]
async fn list_all_tenants() {
    let ctx = setup_context().await;
    let t1 = TenantDescriptor::new("Tenant One", "tenant-one");
    let t2 = TenantDescriptor::new("Tenant Two", "tenant-two");
    ctx.catalog.create(&t1).await.expect("insert tenant 1");
    ctx.catalog.create(&t2).await.expect("insert tenant 2");

    let tenants = ctx.catalog.list().await.expect("list tenants");
    assert_eq!(tenants.len(), 2);
}

#[tokio::test]
async fn update_tenant_status() {
    let ctx = setup_context().await;
    let mut tenant = TenantDescriptor::new("Status Co", "status-co");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    tenant.transition_to(TenantStatus::Suspended);
    ctx.catalog.update(&tenant).await.expect("update tenant");

    let updated = ctx
        .catalog
        .get(tenant.tenant_id)
        .await
        .expect("fetch tenant")
        .expect("tenant exists");
    assert_eq!(updated.status, TenantStatus::Suspended);
}

#[tokio::test]
async fn enforce_unique_slug_constraint() {
    let ctx = setup_context().await;
    let t1 = TenantDescriptor::new("Alpha", "alpha");
    let mut t2 = TenantDescriptor::new("Beta", "alpha");

    ctx.catalog.create(&t1).await.expect("first insert");
    let err = ctx.catalog.create(&t2).await.expect_err("duplicate slug");
    matches!(err, CoreError::AlreadyExists { .. })
        .then_some(())
        .expect("expected already exists error");

    t2.slug = "alpha-2".to_string();
    ctx.catalog
        .create(&t2)
        .await
        .expect("second insert with new slug");
}

#[tokio::test]
async fn cascade_delete_removes_databases() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Cascade", "cascade");
    ctx.catalog.create(&tenant).await.expect("insert tenant");

    let db = DatabaseDescriptor::new(tenant.tenant_id, "primary", Some("main".into()));
    ctx.databases.create(&db).await.expect("insert database");

    ctx.catalog
        .delete(tenant.tenant_id)
        .await
        .expect("delete tenant");

    let database = ctx
        .databases
        .get(db.database_id)
        .await
        .expect("fetch database");
    assert!(
        database.is_none(),
        "database should be removed on cascade delete"
    );
}

#[tokio::test]
async fn create_database_under_tenant() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("DB Tenant", "db-tenant");
    ctx.catalog.create(&tenant).await.expect("insert tenant");

    let mut database = DatabaseDescriptor::new(tenant.tenant_id, "analytics", None);
    database.transition_to(DatabaseState::Ready);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    let fetched = ctx
        .databases
        .get(database.database_id)
        .await
        .expect("fetch database")
        .expect("database exists");
    assert_eq!(fetched.name, "analytics");
    assert_eq!(fetched.state, DatabaseState::Ready);
}

#[tokio::test]
async fn query_databases_by_tenant() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Tenant With DBs", "tenant-dbs");
    let other = TenantDescriptor::new("Other", "other");
    ctx.catalog.create(&tenant).await.expect("insert tenant");
    ctx.catalog.create(&other).await.expect("insert other");

    let db1 = DatabaseDescriptor::new(tenant.tenant_id, "alpha", None);
    let db2 = DatabaseDescriptor::new(tenant.tenant_id, "beta", None);
    let db3 = DatabaseDescriptor::new(other.tenant_id, "gamma", None);

    ctx.databases.create(&db1).await.expect("db1");
    ctx.databases.create(&db2).await.expect("db2");
    ctx.databases.create(&db3).await.expect("db3");

    let tenant_dbs = ctx
        .databases
        .list_by_tenant(tenant.tenant_id)
        .await
        .expect("list databases");
    assert_eq!(tenant_dbs.len(), 2);
}

#[tokio::test]
async fn quota_validation_rejects_out_of_range_values() {
    let ctx = setup_context().await;
    let mut tenant = TenantDescriptor::new("Quota", "quota");
    tenant.quotas.memory_quota_bytes = (i64::MAX as u64) + 1;

    let err = ctx
        .catalog
        .create(&tenant)
        .await
        .expect_err("should fail memory quota conversion");
    matches!(err, CoreError::InvalidState { .. })
        .then_some(())
        .expect("expected invalid state error");
}

#[tokio::test]
async fn updating_database_persists_changes() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Database Updater", "db-updater");
    ctx.catalog.create(&tenant).await.expect("tenant");

    let mut database =
        DatabaseDescriptor::new(tenant.tenant_id, "warehouse", Some("initial".into()));
    ctx.databases.create(&database).await.expect("create db");

    database.description = Some("updated".into());
    database.schema_version = 2;
    database.transition_to(DatabaseState::Migrating);
    ctx.databases
        .update(&database)
        .await
        .expect("update database");

    let updated = ctx
        .databases
        .get(database.database_id)
        .await
        .expect("fetch db")
        .expect("db exists");
    assert_eq!(updated.description.as_deref(), Some("updated"));
    assert_eq!(updated.schema_version, 2);
    assert_eq!(updated.state, DatabaseState::Migrating);
}

// ==================== Collection CRUD Tests ====================

#[tokio::test]
async fn create_collection_successfully() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Collection Tenant", "coll-tenant");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let database = DatabaseDescriptor::new(tenant.tenant_id, "vectors", None);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    let collection =
        CollectionDescriptor::new(database.database_id, "embeddings", 512, "qwen3-embed-8b");
    ctx.collections
        .create(&collection)
        .await
        .expect("create collection");

    let fetched = ctx
        .collections
        .get(collection.collection_id)
        .await
        .expect("fetch collection")
        .expect("collection exists");
    assert_eq!(fetched.name, "embeddings");
    assert_eq!(fetched.dimension, 512);
    assert_eq!(fetched.metric, DistanceMetric::Cosine);
}

#[tokio::test]
async fn list_collections_by_database() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Multi Collection", "multi-coll");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let database = DatabaseDescriptor::new(tenant.tenant_id, "vectors", None);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    let c1 = CollectionDescriptor::new(database.database_id, "coll1", 256, "model-a");
    let c2 = CollectionDescriptor::new(database.database_id, "coll2", 512, "model-b");

    ctx.collections.create(&c1).await.expect("create c1");
    ctx.collections.create(&c2).await.expect("create c2");

    let collections = ctx
        .collections
        .list_by_database(database.database_id)
        .await
        .expect("list collections");
    assert_eq!(collections.len(), 2);
}

#[tokio::test]
async fn update_collection_parameters() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Update Coll", "update-coll");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let database = DatabaseDescriptor::new(tenant.tenant_id, "vectors", None);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    let mut collection =
        CollectionDescriptor::new(database.database_id, "updates", 512, "old-model");
    ctx.collections.create(&collection).await.expect("create");

    // Update HNSW parameters and model
    collection.hnsw_m = 48;
    collection.embedding_model = "new-model".to_string();
    collection.touch();
    ctx.collections.update(&collection).await.expect("update");

    let updated = ctx
        .collections
        .get(collection.collection_id)
        .await
        .expect("fetch")
        .expect("exists");
    assert_eq!(updated.hnsw_m, 48);
    assert_eq!(updated.embedding_model, "new-model");
}

#[tokio::test]
async fn enforce_unique_collection_name_per_database() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Unique Coll", "unique-coll");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let database = DatabaseDescriptor::new(tenant.tenant_id, "vectors", None);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    let c1 = CollectionDescriptor::new(database.database_id, "same-name", 512, "model");
    let c2 = CollectionDescriptor::new(database.database_id, "same-name", 256, "model");

    ctx.collections.create(&c1).await.expect("first create");
    let err = ctx
        .collections
        .create(&c2)
        .await
        .expect_err("duplicate name");
    assert!(matches!(err, CoreError::AlreadyExists { .. }));
}

#[tokio::test]
async fn validate_dimension_bounds() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Dimension", "dimension");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let database = DatabaseDescriptor::new(tenant.tenant_id, "vectors", None);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    // Test too small dimension
    let mut collection = CollectionDescriptor::new(database.database_id, "too-small", 8, "model");
    let err = ctx
        .collections
        .create(&collection)
        .await
        .expect_err("too small");
    assert!(matches!(err, CoreError::InvalidState { .. }));

    // Test too large dimension
    collection.name = "too-large".to_string();
    collection.dimension = 8192;
    let err = ctx
        .collections
        .create(&collection)
        .await
        .expect_err("too large");
    assert!(matches!(err, CoreError::InvalidState { .. }));
}

#[tokio::test]
async fn cascade_delete_database_removes_collections() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Cascade Coll", "cascade-coll");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let database = DatabaseDescriptor::new(tenant.tenant_id, "vectors", None);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    let collection = CollectionDescriptor::new(database.database_id, "test", 512, "model");
    ctx.collections
        .create(&collection)
        .await
        .expect("create collection");

    // Delete database should cascade to collection
    ctx.databases
        .delete(database.database_id)
        .await
        .expect("delete database");

    let result = ctx
        .collections
        .get(collection.collection_id)
        .await
        .expect("fetch");
    assert!(result.is_none(), "collection should be deleted");
}

#[tokio::test]
async fn delete_collection() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Delete Coll", "delete-coll");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let database = DatabaseDescriptor::new(tenant.tenant_id, "vectors", None);
    ctx.databases
        .create(&database)
        .await
        .expect("create database");

    let collection = CollectionDescriptor::new(database.database_id, "to-delete", 512, "model");
    ctx.collections
        .create(&collection)
        .await
        .expect("create collection");

    ctx.collections
        .delete(collection.collection_id)
        .await
        .expect("delete");

    let result = ctx
        .collections
        .get(collection.collection_id)
        .await
        .expect("fetch");
    assert!(result.is_none());
}

// ==================== User Management Tests ====================

#[tokio::test]
async fn create_user_successfully() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("User Corp", "user-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "alice@example.com", Role::Developer);
    user.password_hash = password::hash_password("secure_password").expect("hash password");
    ctx.users.create(&user).await.expect("create user");

    let fetched = ctx
        .users
        .get(user.user_id)
        .await
        .expect("get user")
        .expect("user exists");
    assert_eq!(fetched.email, "alice@example.com");
    assert_eq!(fetched.role, Role::Developer);
    assert_eq!(fetched.status, UserStatus::Active);
}

#[tokio::test]
async fn get_user_by_email() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Email Corp", "email-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "bob@example.com", Role::Viewer);
    user.password_hash = password::hash_password("password123").expect("hash password");
    ctx.users.create(&user).await.expect("create user");

    let fetched = ctx
        .users
        .get_by_email(tenant.tenant_id, "bob@example.com")
        .await
        .expect("get by email")
        .expect("user exists");
    assert_eq!(fetched.user_id, user.user_id);
}

#[tokio::test]
async fn list_users_by_tenant() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Multi User Corp", "multi-user");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user1 = UserDescriptor::new(tenant.tenant_id, "user1@example.com", Role::Admin);
    user1.password_hash = password::hash_password("pass1").expect("hash");
    let mut user2 = UserDescriptor::new(tenant.tenant_id, "user2@example.com", Role::Developer);
    user2.password_hash = password::hash_password("pass2").expect("hash");

    ctx.users.create(&user1).await.expect("create user1");
    ctx.users.create(&user2).await.expect("create user2");

    let users = ctx
        .users
        .list_by_tenant(tenant.tenant_id)
        .await
        .expect("list users");
    assert_eq!(users.len(), 2);
}

#[tokio::test]
async fn update_user_role() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Update Corp", "update-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "charlie@example.com", Role::Viewer);
    user.password_hash = password::hash_password("password").expect("hash");
    ctx.users.create(&user).await.expect("create user");

    user.role = Role::Developer;
    user.touch();
    ctx.users.update(&user).await.expect("update user");

    let updated = ctx
        .users
        .get(user.user_id)
        .await
        .expect("get user")
        .expect("user exists");
    assert_eq!(updated.role, Role::Developer);
}

#[tokio::test]
async fn update_user_status() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Status Corp", "status-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "dave@example.com", Role::Admin);
    user.password_hash = password::hash_password("password").expect("hash");
    ctx.users.create(&user).await.expect("create user");

    user.transition_to(UserStatus::Suspended);
    ctx.users.update(&user).await.expect("update user");

    let updated = ctx
        .users
        .get(user.user_id)
        .await
        .expect("get user")
        .expect("user exists");
    assert_eq!(updated.status, UserStatus::Suspended);
}

#[tokio::test]
async fn record_user_login() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Login Corp", "login-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "eve@example.com", Role::Developer);
    user.password_hash = password::hash_password("password").expect("hash");
    ctx.users.create(&user).await.expect("create user");

    assert!(user.last_login_at.is_none());

    user.record_login();
    ctx.users.update(&user).await.expect("update user");

    let updated = ctx
        .users
        .get(user.user_id)
        .await
        .expect("get user")
        .expect("user exists");
    assert!(updated.last_login_at.is_some());
}

#[tokio::test]
async fn enforce_unique_email_per_tenant() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Unique Corp", "unique-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user1 = UserDescriptor::new(tenant.tenant_id, "duplicate@example.com", Role::Admin);
    user1.password_hash = password::hash_password("pass1").expect("hash");
    let mut user2 = UserDescriptor::new(tenant.tenant_id, "duplicate@example.com", Role::Viewer);
    user2.password_hash = password::hash_password("pass2").expect("hash");

    ctx.users.create(&user1).await.expect("create first user");
    let err = ctx.users.create(&user2).await.expect_err("duplicate email");
    assert!(matches!(err, CoreError::AlreadyExists { .. }));
}

#[tokio::test]
async fn cascade_delete_tenant_removes_users() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Cascade User", "cascade-user");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "frank@example.com", Role::Admin);
    user.password_hash = password::hash_password("password").expect("hash");
    ctx.users.create(&user).await.expect("create user");

    ctx.catalog
        .delete(tenant.tenant_id)
        .await
        .expect("delete tenant");

    let fetched = ctx.users.get(user.user_id).await.expect("get user");
    assert!(fetched.is_none(), "user should be deleted on cascade");
}

// ==================== RBAC Permission Tests ====================

#[tokio::test]
async fn admin_has_all_permissions() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Admin Test", "admin-test");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let user = UserDescriptor::new(tenant.tenant_id, "admin@example.com", Role::Admin);

    // Admin should have all permissions
    assert!(user.has_permission(Action::UserCreate));
    assert!(user.has_permission(Action::DatabaseCreate));
    assert!(user.has_permission(Action::CollectionDelete));
    assert!(user.has_permission(Action::DocumentInsert));
    assert!(user.has_permission(Action::AuditRead));
}

#[tokio::test]
async fn developer_has_limited_permissions() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Dev Test", "dev-test");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let user = UserDescriptor::new(tenant.tenant_id, "dev@example.com", Role::Developer);

    // Developer can manage collections and documents
    assert!(user.has_permission(Action::CollectionCreate));
    assert!(user.has_permission(Action::DocumentInsert));
    assert!(user.has_permission(Action::DocumentSearch));

    // But cannot manage users
    assert!(!user.has_permission(Action::UserCreate));
    assert!(!user.has_permission(Action::UserDelete));
}

#[tokio::test]
async fn viewer_is_read_only() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Viewer Test", "viewer-test");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let user = UserDescriptor::new(tenant.tenant_id, "viewer@example.com", Role::Viewer);

    // Viewer can read
    assert!(user.has_permission(Action::DatabaseRead));
    assert!(user.has_permission(Action::CollectionRead));
    assert!(user.has_permission(Action::DocumentSearch));

    // But cannot write
    assert!(!user.has_permission(Action::CollectionCreate));
    assert!(!user.has_permission(Action::DocumentInsert));
    assert!(!user.has_permission(Action::UserCreate));
}

#[tokio::test]
async fn suspended_user_has_no_permissions() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Suspended Test", "suspended-test");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "suspended@example.com", Role::Admin);
    user.transition_to(UserStatus::Suspended);

    // Suspended users have no permissions, even if they're admins
    assert!(!user.has_permission(Action::DatabaseRead));
    assert!(!user.has_permission(Action::CollectionCreate));
    assert!(!user.has_permission(Action::DocumentSearch));
}

// ==================== Audit Log Tests ====================

#[tokio::test]
async fn create_audit_log_entry() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Audit Corp", "audit-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "auditor@example.com", Role::Auditor);
    user.password_hash = password::hash_password("password").expect("hash");
    ctx.users.create(&user).await.expect("create user");

    let entry = AuditLogEntry::new(
        tenant.tenant_id,
        Some(user.user_id),
        Action::CollectionCreate,
        "collection",
        "coll-123",
        AuditResult::Allowed,
    )
    .with_reason("Valid permissions")
    .with_ip("192.168.1.100");

    ctx.audit_logs
        .create(&entry)
        .await
        .expect("create audit log");

    let logs = ctx
        .audit_logs
        .list_by_tenant(tenant.tenant_id, 10, 0)
        .await
        .expect("list logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].action, Action::CollectionCreate);
    assert_eq!(logs[0].result, AuditResult::Allowed);
}

#[tokio::test]
async fn list_audit_logs_by_tenant() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Multi Audit", "multi-audit");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let entry1 = AuditLogEntry::new(
        tenant.tenant_id,
        None,
        Action::DatabaseCreate,
        "database",
        "db-1",
        AuditResult::Allowed,
    );
    let entry2 = AuditLogEntry::new(
        tenant.tenant_id,
        None,
        Action::CollectionDelete,
        "collection",
        "coll-1",
        AuditResult::Denied,
    );

    ctx.audit_logs.create(&entry1).await.expect("create log 1");
    ctx.audit_logs.create(&entry2).await.expect("create log 2");

    let logs = ctx
        .audit_logs
        .list_by_tenant(tenant.tenant_id, 10, 0)
        .await
        .expect("list logs");
    assert_eq!(logs.len(), 2);
}

#[tokio::test]
async fn list_audit_logs_by_user() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("User Audit", "user-audit");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let mut user = UserDescriptor::new(tenant.tenant_id, "tracked@example.com", Role::Developer);
    user.password_hash = password::hash_password("password").expect("hash");
    ctx.users.create(&user).await.expect("create user");

    let entry1 = AuditLogEntry::new(
        tenant.tenant_id,
        Some(user.user_id),
        Action::DocumentInsert,
        "document",
        "doc-1",
        AuditResult::Allowed,
    );
    let entry2 = AuditLogEntry::new(
        tenant.tenant_id,
        Some(user.user_id),
        Action::DocumentUpdate,
        "document",
        "doc-2",
        AuditResult::Allowed,
    );

    ctx.audit_logs.create(&entry1).await.expect("create log 1");
    ctx.audit_logs.create(&entry2).await.expect("create log 2");

    let logs = ctx
        .audit_logs
        .list_by_user(user.user_id, 10, 0)
        .await
        .expect("list logs");
    assert_eq!(logs.len(), 2);
}

// ==================== API Key Tests ====================

#[tokio::test]
async fn create_api_key_successfully() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("API Key Corp", "api-key-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);
    let descriptor = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "test-key".to_string(),
        vec!["collection::read".to_string()],
        None,
        None,
    );

    ctx.api_keys
        .create(&descriptor, &key_hash)
        .await
        .expect("create API key");

    let fetched = ctx
        .api_keys
        .get(descriptor.key_id)
        .await
        .expect("get API key")
        .expect("key exists");
    assert_eq!(fetched.name, "test-key");
    assert_eq!(fetched.permissions, vec!["collection::read"]);
}

#[tokio::test]
async fn get_api_key_by_hash() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Hash Corp", "hash-corp");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);
    let descriptor = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "hash-test-key".to_string(),
        vec!["collection::write".to_string()],
        None,
        None,
    );

    ctx.api_keys
        .create(&descriptor, &key_hash)
        .await
        .expect("create API key");

    let fetched = ctx
        .api_keys
        .get_by_hash(&key_hash)
        .await
        .expect("get by hash")
        .expect("key exists");
    assert_eq!(fetched.key_id, descriptor.key_id);
    assert_eq!(fetched.name, "hash-test-key");
}

#[tokio::test]
async fn list_api_keys_by_tenant() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Multi Key Corp", "multi-key");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let key1 = generate_api_key();
    let hash1 = hash_api_key(&key1);
    let descriptor1 = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "key-1".to_string(),
        vec!["collection::read".to_string()],
        None,
        None,
    );

    let key2 = generate_api_key();
    let hash2 = hash_api_key(&key2);
    let descriptor2 = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "key-2".to_string(),
        vec!["collection::write".to_string()],
        None,
        None,
    );

    ctx.api_keys
        .create(&descriptor1, &hash1)
        .await
        .expect("create key 1");
    ctx.api_keys
        .create(&descriptor2, &hash2)
        .await
        .expect("create key 2");

    let keys = ctx
        .api_keys
        .list_by_tenant(tenant.tenant_id)
        .await
        .expect("list keys");
    assert_eq!(keys.len(), 2);
}

#[tokio::test]
async fn delete_api_key() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Delete Key Corp", "delete-key");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);
    let descriptor = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "to-delete".to_string(),
        vec!["collection::read".to_string()],
        None,
        None,
    );

    ctx.api_keys
        .create(&descriptor, &key_hash)
        .await
        .expect("create API key");

    ctx.api_keys
        .delete(descriptor.key_id)
        .await
        .expect("delete key");

    let fetched = ctx.api_keys.get(descriptor.key_id).await.expect("get key");
    assert!(fetched.is_none(), "key should be deleted");
}

#[tokio::test]
async fn update_last_used_timestamp() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Last Used Corp", "last-used");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);
    let descriptor = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "usage-key".to_string(),
        vec!["collection::read".to_string()],
        None,
        None,
    );

    ctx.api_keys
        .create(&descriptor, &key_hash)
        .await
        .expect("create API key");

    // Initially, last_used_at should be None
    let fetched = ctx
        .api_keys
        .get(descriptor.key_id)
        .await
        .expect("get key")
        .expect("key exists");
    assert!(fetched.last_used_at.is_none());

    // Update last_used_at
    ctx.api_keys
        .update_last_used(descriptor.key_id)
        .await
        .expect("update last used");

    // Now last_used_at should be set
    let updated = ctx
        .api_keys
        .get(descriptor.key_id)
        .await
        .expect("get key")
        .expect("key exists");
    assert!(updated.last_used_at.is_some());
}

#[tokio::test]
async fn cascade_delete_tenant_removes_api_keys() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Cascade Key", "cascade-key");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);
    let descriptor = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "cascade-test-key".to_string(),
        vec!["collection::read".to_string()],
        None,
        None,
    );

    ctx.api_keys
        .create(&descriptor, &key_hash)
        .await
        .expect("create API key");

    ctx.catalog
        .delete(tenant.tenant_id)
        .await
        .expect("delete tenant");

    let fetched = ctx.api_keys.get(descriptor.key_id).await.expect("get key");
    assert!(fetched.is_none(), "key should be deleted on cascade");
}

#[tokio::test]
async fn enforce_unique_key_hash() {
    let ctx = setup_context().await;
    let tenant = TenantDescriptor::new("Unique Key Corp", "unique-key");
    ctx.catalog.create(&tenant).await.expect("create tenant");

    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);

    let descriptor1 = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "key-1".to_string(),
        vec!["collection::read".to_string()],
        None,
        None,
    );
    let descriptor2 = ApiKeyDescriptor::new(
        tenant.tenant_id,
        "key-2".to_string(),
        vec!["collection::write".to_string()],
        None,
        None,
    );

    ctx.api_keys
        .create(&descriptor1, &key_hash)
        .await
        .expect("create first key");
    let err = ctx
        .api_keys
        .create(&descriptor2, &key_hash)
        .await
        .expect_err("duplicate key hash");
    assert!(matches!(err, CoreError::AlreadyExists { .. }));
}
