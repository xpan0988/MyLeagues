use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalItem {
    pub item_id: i64,
    pub slot: i64,
    pub classification: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemMetadata {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub gold: i64,
    pub purchasable: bool,
    pub tags: Vec<String>,
    pub from: Vec<i64>,
    pub into: Vec<i64>,
    pub maps: BTreeMap<String, bool>,
}

impl ItemMetadata {
    pub fn is_boot(&self) -> bool {
        self.has_tag("Boots")
    }

    pub fn is_trinket(&self) -> bool {
        self.has_tag("Trinket")
    }

    pub fn is_consumable(&self) -> bool {
        self.has_tag("Consumable")
    }

    pub fn is_component(&self) -> bool {
        !self.into.is_empty()
    }

    pub fn is_completed_item(&self) -> bool {
        self.purchasable && self.gold > 0 && self.into.is_empty()
    }

    pub fn is_valid_core_item(&self) -> bool {
        self.is_completed_item() && !self.is_boot() && !self.is_trinket() && !self.is_consumable()
    }

    pub fn semantic_classification(&self) -> &'static str {
        if self.is_boot() {
            "boot"
        } else if self.is_trinket() {
            "trinket"
        } else if self.is_consumable() {
            "consumable"
        } else if self.is_component() {
            "component"
        } else if self.is_valid_core_item() {
            "core"
        } else {
            "other"
        }
    }

    fn has_tag(&self, expected: &str) -> bool {
        self.tags.iter().any(|tag| tag == expected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CoreBuild {
    pub item_ids: Vec<i64>,
}

impl CoreBuild {
    pub fn canonical(mut item_ids: Vec<i64>) -> Self {
        item_ids.sort_unstable();
        item_ids.truncate(3);
        Self { item_ids }
    }
}

#[cfg(test)]
mod tests {
    use super::{CoreBuild, ItemMetadata};

    fn item(tags: &[&str], into: &[i64], purchasable: bool) -> ItemMetadata {
        ItemMetadata {
            id: 1,
            name: "Item".to_owned(),
            description: String::new(),
            icon: String::new(),
            gold: 3_000,
            purchasable,
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            from: Vec::new(),
            into: into.to_vec(),
            maps: Default::default(),
        }
    }

    #[test]
    fn classifies_boots_consumables_trinkets_components_and_core_items() {
        assert_eq!(
            item(&["Boots"], &[], true).semantic_classification(),
            "boot"
        );
        assert_eq!(
            item(&["Consumable"], &[], true).semantic_classification(),
            "consumable"
        );
        assert_eq!(
            item(&["Trinket"], &[], true).semantic_classification(),
            "trinket"
        );
        assert_eq!(item(&[], &[2], true).semantic_classification(), "component");
        assert_eq!(
            item(&["SpellDamage"], &[], true).semantic_classification(),
            "core"
        );
        assert!(!item(&[], &[], false).is_valid_core_item());
    }

    #[test]
    fn canonical_build_ignores_inventory_slot_permutations() {
        assert_eq!(
            CoreBuild::canonical(vec![3, 1, 2]),
            CoreBuild::canonical(vec![2, 3, 1])
        );
        assert_eq!(
            CoreBuild::canonical(vec![4, 3, 2, 1]).item_ids,
            vec![1, 2, 3]
        );
    }
}
