mod commands;
mod config;
mod db;
mod domain;
mod dto;
mod error;
mod riot;
mod services;

use std::fs;
use std::sync::Arc;

use config::BackendConfig;
use db::Database;
use db::repositories::settings::SettingsRepository;
use riot::client::RiotApiClient;
use riot::ddragon::DataDragonClient;
use services::sync::SyncCoordinator;
use tauri::Manager;

pub struct AppState {
    database: Arc<Database>,
    config: BackendConfig,
    riot: Option<Arc<RiotApiClient>>,
    sync: Arc<SyncCoordinator>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "my_league=info".into()),
        )
        .try_init();

    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let database = Database::open(&app_data_dir.join("myleague.db"))?;
            let config = BackendConfig::from_environment();
            let riot = config
                .riot_api_key()
                .map(RiotApiClient::new)
                .transpose()?
                .map(Arc::new);
            let should_auto_sync = {
                let connection = database.connection()?;
                let settings = SettingsRepository::new(&connection).get()?;
                riot.is_some() && !settings.game_name.is_empty() && !settings.tag_line.is_empty()
            };
            let database = Arc::new(database);
            let sync = Arc::new(SyncCoordinator::new());
            let data_dragon = DataDragonClient::new()?;

            tracing::info!(
                database_path = %app_data_dir.join("myleague.db").display(),
                api_key_configured = config.api_key_configured(),
                "initialized application state"
            );

            app.manage(AppState {
                database: Arc::clone(&database),
                config,
                riot: riot.clone(),
                sync: Arc::clone(&sync),
            });
            if should_auto_sync {
                if let Some(riot) = riot {
                    let app_handle = app.handle().clone();
                    let sync_database = Arc::clone(&database);
                    let background_sync = Arc::clone(&sync);
                    tauri::async_runtime::spawn(async move {
                        services::sync::start_background(sync_database, riot, background_sync, app_handle).await;
                    });
                }
            }
            let static_database = Arc::clone(&database);
            tauri::async_runtime::spawn(async move {
                if let Err(error) = services::static_data::refresh(static_database, data_dragon).await {
                    tracing::warn!(target: "data_dragon", error = %error, "static data refresh unavailable; cached data remains usable");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::home::get_home,
            commands::champions::list_champions,
            commands::champions::get_champion_profile,
            commands::matches::list_matches,
            commands::matches::get_match_detail,
            commands::career::get_career,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::launcher::get_client_state,
            commands::launcher::launch_client,
            commands::sync::start_sync,
            commands::sync::get_sync_state,
            commands::maintenance::rebuild_aggregates,
            commands::maintenance::clear_static_cache,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run the MyLeague desktop application");
}

#[cfg(test)]
mod tests {
    use crate::db::Database;

    #[test]
    fn database_bootstraps_in_memory() -> Result<(), Box<dyn std::error::Error>> {
        let database = Database::open_in_memory()?;
        let connection = database.connection()?;
        let settings_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )?;

        assert_eq!(settings_count, 1);
        Ok(())
    }
}
