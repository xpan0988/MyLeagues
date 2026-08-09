use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameEntity {
    pub id: i64,
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatShardMetadata {
    pub id: i64,
    pub name: &'static str,
    pub icon: &'static str,
}

impl GameEntity {
    pub fn unknown(id: i64) -> Self {
        Self {
            id,
            name: format!("Unknown ({id})"),
            icon: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StaticCatalog {
    pub version: Option<String>,
    pub champions: HashMap<i64, GameEntity>,
    pub items: HashMap<i64, GameEntity>,
    pub rune_styles: HashMap<i64, GameEntity>,
    pub runes: HashMap<i64, GameEntity>,
    pub summoner_spells: HashMap<i64, GameEntity>,
}

impl StaticCatalog {
    pub fn champion(&self, id: i64) -> GameEntity {
        self.champions
            .get(&id)
            .cloned()
            .unwrap_or_else(|| GameEntity::unknown(id))
    }
    pub fn item(&self, id: i64) -> GameEntity {
        self.items.get(&id).cloned().unwrap_or_else(|| GameEntity {
            id,
            name: format!("Unknown Item ({id})"),
            icon: String::new(),
        })
    }
    pub fn rune_style(&self, id: i64) -> GameEntity {
        self.rune_styles
            .get(&id)
            .cloned()
            .unwrap_or_else(|| GameEntity::unknown(id))
    }
    pub fn rune(&self, id: i64) -> GameEntity {
        self.runes
            .get(&id)
            .cloned()
            .unwrap_or_else(|| GameEntity::unknown(id))
    }
    pub fn stat_shard(&self, id: i64) -> GameEntity {
        stat_shard_metadata(id)
            .map(|metadata| GameEntity {
                id: metadata.id,
                name: metadata.name.to_owned(),
                icon: metadata.icon.to_owned(),
            })
            .unwrap_or_else(|| GameEntity {
                id,
                name: format!("Unknown Shard ({id})"),
                icon: String::new(),
            })
    }
    pub fn spell(&self, id: i64) -> GameEntity {
        self.summoner_spells
            .get(&id)
            .cloned()
            .unwrap_or_else(|| GameEntity::unknown(id))
    }
}

// Data Dragon's runesReforged payload omits stat shards. Keep this compatibility
// table isolated to shard IDs and official CommunityDragon/Data Dragon asset paths.
fn stat_shard_metadata(id: i64) -> Option<StatShardMetadata> {
    let metadata = match id {
        5008 => StatShardMetadata {
            id,
            name: "Adaptive Force",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsAdaptiveForceIcon.png",
        },
        5005 => StatShardMetadata {
            id,
            name: "Attack Speed",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsAttackSpeedIcon.png",
        },
        5007 => StatShardMetadata {
            id,
            name: "Ability Haste",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsCDRScalingIcon.png",
        },
        5001 => StatShardMetadata {
            id,
            name: "Health Scaling",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsHealthScalingIcon.png",
        },
        5002 => StatShardMetadata {
            id,
            name: "Armor",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsArmorIcon.png",
        },
        5003 => StatShardMetadata {
            id,
            name: "Magic Resist",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsMagicResIcon.MagicResist_Fix.png",
        },
        5010 => StatShardMetadata {
            id,
            name: "Move Speed",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsMovementSpeedIcon.png",
        },
        5011 => StatShardMetadata {
            id,
            name: "Health",
            icon: "https://ddragon.leagueoflegends.com/cdn/img/perk-images/StatMods/StatModsHealthPlusIcon.png",
        },
        _ => return None,
    };
    Some(metadata)
}

#[cfg(test)]
mod tests {
    use super::StaticCatalog;

    #[test]
    fn resolves_known_stat_shards_and_explicit_unknown_fallback() {
        let catalog = StaticCatalog::default();
        for (id, name) in [
            (5008, "Adaptive Force"),
            (5005, "Attack Speed"),
            (5001, "Health Scaling"),
        ] {
            let shard = catalog.stat_shard(id);
            assert_eq!(shard.name, name);
            assert!(!shard.icon.is_empty());
        }
        let unknown = catalog.stat_shard(5999);
        assert_eq!(unknown.name, "Unknown Shard (5999)");
        assert!(unknown.icon.is_empty());
    }

    #[test]
    fn item_fallback_is_explicit() {
        assert_eq!(
            StaticCatalog::default().item(6610).name,
            "Unknown Item (6610)"
        );
    }
}
