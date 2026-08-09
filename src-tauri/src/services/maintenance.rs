use crate::db::Database;
use crate::db::repositories::aggregates::AggregateRepository;
use crate::error::AppResult;

pub struct MaintenanceService<'state> {
    database: &'state Database,
}

impl<'state> MaintenanceService<'state> {
    pub fn new(database: &'state Database) -> Self {
        Self { database }
    }

    pub fn rebuild_aggregates(&self) -> AppResult<()> {
        let mut connection = self.database.connection()?;
        AggregateRepository::rebuild(&mut connection)
    }

    pub fn clear_static_cache(&self) -> AppResult<()> {
        let connection = self.database.connection()?;
        connection.execute("DELETE FROM static_data_versions", [])?;
        tracing::info!(target: "data_dragon", "cleared static data cache");
        Ok(())
    }
}
