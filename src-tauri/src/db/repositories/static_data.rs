use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::domain::static_data::{GameEntity, StaticCatalog};
use crate::error::AppResult;
use crate::riot::ddragon::StaticDataBundle;

pub struct StaticDataRepository;

impl StaticDataRepository {
    pub fn catalog(connection: &Connection) -> AppResult<StaticCatalog> {
        let Some(version) = Self::active_version(connection)? else {
            return Ok(StaticCatalog::default());
        };
        let mut catalog = StaticCatalog {
            version: Some(version.clone()),
            ..Default::default()
        };
        load_entities(
            connection,
            "static_champions",
            "champion_id",
            &version,
            &mut catalog.champions,
        )?;
        load_entities(
            connection,
            "static_items",
            "item_id",
            &version,
            &mut catalog.items,
        )?;
        load_entities(
            connection,
            "static_rune_styles",
            "style_id",
            &version,
            &mut catalog.rune_styles,
        )?;
        load_entities(
            connection,
            "static_runes",
            "rune_id",
            &version,
            &mut catalog.runes,
        )?;
        load_entities(
            connection,
            "static_summoner_spells",
            "spell_id",
            &version,
            &mut catalog.summoner_spells,
        )?;
        Ok(catalog)
    }
    pub fn active_version(connection: &Connection) -> AppResult<Option<String>> {
        Ok(connection
            .query_row(
                "SELECT version FROM static_data_versions WHERE is_active = 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn has_version(connection: &Connection, version: &str) -> AppResult<bool> {
        Ok(connection
            .query_row(
                "SELECT 1 FROM static_data_versions WHERE version = ?1",
                [version],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some())
    }

    pub fn activate(connection: &mut Connection, version: &str) -> AppResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE static_data_versions SET is_active = 0 WHERE is_active = 1",
            [],
        )?;
        transaction.execute(
            "UPDATE static_data_versions SET is_active = 1 WHERE version = ?1",
            [version],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn store(connection: &mut Connection, bundle: &StaticDataBundle) -> AppResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE static_data_versions SET is_active = 0 WHERE is_active = 1",
            [],
        )?;
        transaction.execute(
            "INSERT INTO static_data_versions (version, is_active) VALUES (?1, 1)
             ON CONFLICT(version) DO UPDATE SET is_active = 1, fetched_at = CURRENT_TIMESTAMP",
            [&bundle.version],
        )?;
        for champion in &bundle.champions {
            transaction.execute(
                "INSERT OR REPLACE INTO static_champions
                 (version, champion_id, champion_key, name, icon) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    bundle.version,
                    champion.id,
                    champion.key,
                    champion.name,
                    champion.icon
                ],
            )?;
        }
        for item in &bundle.items {
            transaction.execute(
                "INSERT OR REPLACE INTO static_items (
                    version, item_id, name, description, icon, gold, purchasable,
                    tags_json, from_json, into_json, maps_json, classification
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    bundle.version,
                    item.id,
                    item.name,
                    item.description,
                    item.icon,
                    item.gold,
                    item.purchasable,
                    serde_json::to_string(&item.tags)?,
                    serde_json::to_string(&item.from)?,
                    serde_json::to_string(&item.into)?,
                    serde_json::to_string(&item.maps)?,
                    item.semantic_classification(),
                ],
            )?;
        }
        for style in &bundle.rune_styles {
            transaction.execute(
                "INSERT OR REPLACE INTO static_rune_styles
                 (version, style_id, name, icon) VALUES (?1, ?2, ?3, ?4)",
                params![bundle.version, style.id, style.name, style.icon],
            )?;
        }
        for rune in &bundle.runes {
            transaction.execute(
                "INSERT OR REPLACE INTO static_runes
                 (version, rune_id, style_id, slot_order, name, icon)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    bundle.version,
                    rune.id,
                    rune.style_id,
                    rune.slot_order,
                    rune.name,
                    rune.icon
                ],
            )?;
        }
        for spell in &bundle.summoner_spells {
            transaction.execute(
                "INSERT OR REPLACE INTO static_summoner_spells
                 (version, spell_id, name, description, icon) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    bundle.version,
                    spell.id,
                    spell.name,
                    spell.description,
                    spell.icon
                ],
            )?;
        }
        for (kind, json) in &bundle.raw_payloads {
            transaction.execute(
                "INSERT OR REPLACE INTO static_payload_cache (version, payload_kind, json)
                 VALUES (?1, ?2, ?3)",
                params![bundle.version, kind, json],
            )?;
        }
        transaction.commit()?;
        tracing::info!(target: "data_dragon", version = %bundle.version, "stored static data cache");
        Ok(())
    }
}

fn load_entities(
    connection: &Connection,
    table: &str,
    id_column: &str,
    version: &str,
    target: &mut std::collections::HashMap<i64, GameEntity>,
) -> AppResult<()> {
    // Table and column names are internal constants selected by the caller above.
    let sql = format!("SELECT {id_column}, name, icon FROM {table} WHERE version = ?1");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([version], |row| {
        Ok(GameEntity {
            id: row.get(0)?,
            name: row.get(1)?,
            icon: row.get(2)?,
        })
    })?;
    for entity in rows {
        let entity = entity?;
        target.insert(entity.id, entity);
    }
    Ok(())
}
