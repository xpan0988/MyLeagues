use std::sync::Arc;

use crate::db::Database;
use crate::db::repositories::static_data::StaticDataRepository;
use crate::error::AppResult;
use crate::riot::ddragon::DataDragonClient;

pub async fn refresh(database: Arc<Database>, client: DataDragonClient) -> AppResult<String> {
    let version = client.latest_version().await?;
    {
        let mut connection = database.connection()?;
        if StaticDataRepository::has_version(&connection, &version)? {
            StaticDataRepository::activate(&mut connection, &version)?;
            tracing::info!(target: "data_dragon", version, "reused cached static data");
            return Ok(version);
        }
    }
    let bundle = client.fetch_bundle(&version).await?;
    let mut connection = database.connection()?;
    StaticDataRepository::store(&mut connection, &bundle)?;
    Ok(version)
}
