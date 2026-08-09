use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::domain::items::ItemMetadata;
use crate::error::{AppError, AppResult};

const DDRAGON_ROOT: &str = "https://ddragon.leagueoflegends.com";

#[derive(Clone)]
pub struct DataDragonClient {
    http: Client,
}

#[derive(Clone, Debug)]
pub struct StaticDataBundle {
    pub version: String,
    pub champions: Vec<ChampionMetadata>,
    pub items: Vec<ItemMetadata>,
    pub rune_styles: Vec<RuneStyleMetadata>,
    pub runes: Vec<RuneMetadata>,
    pub summoner_spells: Vec<SpellMetadata>,
    pub raw_payloads: Vec<(&'static str, String)>,
}

#[derive(Clone, Debug)]
pub struct ChampionMetadata {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug)]
pub struct RuneStyleMetadata {
    pub id: i64,
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug)]
pub struct RuneMetadata {
    pub id: i64,
    pub style_id: i64,
    pub slot_order: i64,
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug)]
pub struct SpellMetadata {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub icon: String,
}

impl DataDragonClient {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            http: Client::builder()
                .https_only(true)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .user_agent("MyLeague/0.1")
                .build()?,
        })
    }

    pub async fn latest_version(&self) -> AppResult<String> {
        let versions: Vec<String> = self
            .http
            .get(format!("{DDRAGON_ROOT}/api/versions.json"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        versions.into_iter().next().ok_or_else(|| {
            AppError::StaticData("versions response did not contain a version".to_owned())
        })
    }

    pub async fn fetch_bundle(&self, version: &str) -> AppResult<StaticDataBundle> {
        validate_version(version)?;
        let champion_json = self.fetch_text(version, "champion.json").await?;
        let item_json = self.fetch_text(version, "item.json").await?;
        let spell_json = self.fetch_text(version, "summoner.json").await?;
        let runes_json = self
            .http
            .get(format!(
                "{DDRAGON_ROOT}/cdn/{version}/data/en_US/runesReforged.json"
            ))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let champions = parse_champions(version, &champion_json)?;
        let items = parse_items(version, &item_json)?;
        let (rune_styles, runes) = parse_runes(&runes_json)?;
        let summoner_spells = parse_spells(version, &spell_json)?;
        Ok(StaticDataBundle {
            version: version.to_owned(),
            champions,
            items,
            rune_styles,
            runes,
            summoner_spells,
            raw_payloads: vec![
                ("champions", champion_json),
                ("items", item_json),
                ("runes", runes_json),
                ("summoner_spells", spell_json),
            ],
        })
    }

    async fn fetch_text(&self, version: &str, file: &str) -> AppResult<String> {
        Ok(self
            .http
            .get(format!("{DDRAGON_ROOT}/cdn/{version}/data/en_US/{file}"))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?)
    }
}

fn validate_version(version: &str) -> AppResult<()> {
    if version.is_empty()
        || !version
            .chars()
            .all(|value| value.is_ascii_digit() || value == '.')
    {
        return Err(AppError::StaticData(
            "invalid Data Dragon version".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct Catalog<T> {
    data: HashMap<String, T>,
}

#[derive(Deserialize)]
struct Image {
    full: String,
}

#[derive(Deserialize)]
struct ChampionRaw {
    id: String,
    key: String,
    name: String,
    image: Image,
}

fn parse_champions(version: &str, json: &str) -> AppResult<Vec<ChampionMetadata>> {
    let catalog: Catalog<ChampionRaw> = serde_json::from_str(json)?;
    catalog
        .data
        .into_values()
        .map(|value| {
            Ok(ChampionMetadata {
                id: value
                    .key
                    .parse()
                    .map_err(|_| AppError::StaticData("invalid champion ID".to_owned()))?,
                key: value.id,
                name: value.name,
                icon: format!(
                    "{DDRAGON_ROOT}/cdn/{version}/img/champion/{}",
                    value.image.full
                ),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct GoldRaw {
    total: i64,
    purchasable: bool,
}

#[derive(Deserialize)]
struct ItemRaw {
    name: String,
    description: String,
    image: Image,
    gold: GoldRaw,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    from: Vec<String>,
    #[serde(default)]
    into: Vec<String>,
    #[serde(default)]
    maps: BTreeMap<String, bool>,
}

fn parse_items(version: &str, json: &str) -> AppResult<Vec<ItemMetadata>> {
    let catalog: Catalog<ItemRaw> = serde_json::from_str(json)?;
    catalog
        .data
        .into_iter()
        .map(|(id, value)| {
            Ok(ItemMetadata {
                id: id
                    .parse()
                    .map_err(|_| AppError::StaticData("invalid item ID".to_owned()))?,
                name: value.name,
                description: value.description,
                icon: format!("{DDRAGON_ROOT}/cdn/{version}/img/item/{}", value.image.full),
                gold: value.gold.total,
                purchasable: value.gold.purchasable,
                tags: value.tags,
                from: parse_ids(value.from)?,
                into: parse_ids(value.into)?,
                maps: value.maps,
            })
        })
        .collect()
}

fn parse_ids(values: Vec<String>) -> AppResult<Vec<i64>> {
    values
        .into_iter()
        .map(|value| {
            value
                .parse()
                .map_err(|_| AppError::StaticData("invalid linked item ID".to_owned()))
        })
        .collect()
}

#[derive(Deserialize)]
struct RuneStyleRaw {
    id: i64,
    key: String,
    icon: String,
    slots: Vec<RuneSlotRaw>,
}
#[derive(Deserialize)]
struct RuneSlotRaw {
    runes: Vec<RuneRaw>,
}
#[derive(Deserialize)]
struct RuneRaw {
    id: i64,
    name: String,
    icon: String,
}

fn parse_runes(json: &str) -> AppResult<(Vec<RuneStyleMetadata>, Vec<RuneMetadata>)> {
    let raw: Vec<RuneStyleRaw> = serde_json::from_str(json)?;
    let mut styles = Vec::new();
    let mut runes = Vec::new();
    for style in raw {
        styles.push(RuneStyleMetadata {
            id: style.id,
            name: style.key,
            icon: asset_url(&style.icon),
        });
        for (slot_order, slot) in style.slots.into_iter().enumerate() {
            runes.extend(slot.runes.into_iter().map(|rune| RuneMetadata {
                id: rune.id,
                style_id: style.id,
                slot_order: slot_order as i64,
                name: rune.name,
                icon: asset_url(&rune.icon),
            }));
        }
    }
    Ok((styles, runes))
}

#[derive(Deserialize)]
struct SpellRaw {
    key: String,
    name: String,
    description: String,
    image: Image,
}

fn parse_spells(version: &str, json: &str) -> AppResult<Vec<SpellMetadata>> {
    let catalog: Catalog<SpellRaw> = serde_json::from_str(json)?;
    catalog
        .data
        .into_values()
        .map(|value| {
            Ok(SpellMetadata {
                id: value
                    .key
                    .parse()
                    .map_err(|_| AppError::StaticData("invalid spell ID".to_owned()))?,
                name: value.name,
                description: value.description,
                icon: format!(
                    "{DDRAGON_ROOT}/cdn/{version}/img/spell/{}",
                    value.image.full
                ),
            })
        })
        .collect()
}

fn asset_url(path: &str) -> String {
    format!("{DDRAGON_ROOT}/cdn/img/{path}")
}

#[cfg(test)]
mod tests {
    use super::parse_items;

    #[test]
    fn parses_known_item_id_name_and_icon_from_string_key() {
        let items = parse_items("16.15.1", r#"{"data":{"6610":{"name":"Sundered Sky","description":"item","image":{"full":"6610.png"},"gold":{"total":3100,"purchasable":true},"tags":["Damage"],"from":["1037"],"into":[],"maps":{"11":true}}}}"#).unwrap();
        assert_eq!(items[0].id, 6610);
        assert_eq!(items[0].name, "Sundered Sky");
        assert!(items[0].icon.ends_with("/img/item/6610.png"));
    }
}
