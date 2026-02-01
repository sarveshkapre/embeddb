use std::path::PathBuf;

use anyhow::Result;
use embeddb::{Config, EmbedDb};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let data_dir = PathBuf::from("./data");
    let _db = EmbedDb::open(Config::new(data_dir))?;

    println!("embeddb-server scaffold (no HTTP yet)");
    Ok(())
}
