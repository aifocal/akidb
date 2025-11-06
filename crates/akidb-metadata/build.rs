use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let migrations_dir = manifest_dir.join("migrations");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let db_path = out_dir.join("sqlx_build.db");

    if let Some(parent) = db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::remove_file(&db_path);

    let mut migration_files: Vec<PathBuf> = fs::read_dir(&migrations_dir)
        .expect("read migrations")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("sql") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    migration_files.sort();

    for migration in &migration_files {
        apply_migration(&db_path, migration);
        println!("cargo:rerun-if-changed={}", migration.display());
    }

    println!(
        "cargo:rustc-env=DATABASE_URL=sqlite://{}",
        db_path.display()
    );
}

fn apply_migration(db_path: &Path, migration: &Path) {
    let status = Command::new("sqlite3")
        .arg(db_path)
        .arg(format!(".read {}", migration.display()))
        .status()
        .expect("failed to spawn sqlite3");

    if !status.success() {
        panic!(
            "sqlite3 returned non-zero exit status while applying migration {}",
            migration.display()
        );
    }
}
