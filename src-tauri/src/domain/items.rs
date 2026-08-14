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
        // The Timeline describes completed purchases, while static metadata
        // distinguishes real build items from small starter purchases without
        // relying on an item-ID allow/block list.
        self.is_completed_item()
            && self.gold >= 1_000
            && !self.is_boot()
            && !self.is_trinket()
            && !self.is_consumable()
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
    /// A core build is a time-ordered completion path, not an inventory set.
    pub fn first_three(mut item_ids: Vec<i64>) -> Self {
        item_ids.truncate(3);
        Self { item_ids }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemTimelineEvent {
    pub source_id: String,
    pub timestamp_ms: i64,
    pub participant_id: i64,
    pub event_type: String,
    pub item_id: Option<i64>,
    pub before_item_id: Option<i64>,
    pub after_item_id: Option<i64>,
}

/// Derives first-core completion order from normalized Timeline events. Sales
/// preserve historical completion; an undo removes the most recent matching
/// purchase so an immediately undone item cannot occupy a core slot.
pub fn first_completed_core_items<F>(
    events: impl IntoIterator<Item = ItemTimelineEvent>,
    participant_id: i64,
    mut is_core_item: F,
) -> Vec<i64>
where
    F: FnMut(i64) -> bool,
{
    let mut events: Vec<_> = events
        .into_iter()
        .filter(|event| event.participant_id == participant_id)
        .collect();
    events.sort_by(|left, right| {
        (left.timestamp_ms, &left.source_id).cmp(&(right.timestamp_ms, &right.source_id))
    });
    let mut completions: Vec<(String, i64)> = Vec::new();
    for event in events {
        match event.event_type.as_str() {
            "ITEM_PURCHASED" => {
                if let Some(item_id) = event.item_id.filter(|id| is_core_item(*id)) {
                    completions.push((event.source_id, item_id));
                }
            }
            "ITEM_UNDO" => {
                if let Some(item_id) = event.before_item_id.or(event.item_id) {
                    if let Some(index) = completions
                        .iter()
                        .rposition(|(_, purchased_id)| *purchased_id == item_id)
                    {
                        completions.remove(index);
                    }
                }
            }
            "ITEM_SOLD" | "ITEM_DESTROYED" => {}
            _ => {}
        }
    }
    completions
        .into_iter()
        .map(|(_, item_id)| item_id)
        .take(3)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CoreBuild, ItemMetadata, ItemTimelineEvent, first_completed_core_items};

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
        let mut starter = item(&["Damage"], &[], true);
        starter.gold = 450;
        assert_eq!(starter.semantic_classification(), "other");
    }

    #[test]
    fn completion_order_is_not_final_inventory_slot_order() {
        assert_eq!(
            CoreBuild::first_three(vec![3, 1, 2, 4]).item_ids,
            vec![3, 1, 2]
        );
    }

    #[test]
    fn completed_core_items_follow_purchases_and_ignore_boots_components_and_undo() {
        let events = vec![
            event("0", 1, "ITEM_PURCHASED", Some(100), None),
            event("1", 2, "ITEM_PURCHASED", Some(3006), None),
            event("2", 3, "ITEM_PURCHASED", Some(6631), None),
            event("3", 4, "ITEM_PURCHASED", Some(3053), None),
            event("4", 5, "ITEM_UNDO", None, Some(3053)),
            event("5", 6, "ITEM_PURCHASED", Some(3053), None),
            event("6", 7, "ITEM_SOLD", Some(6631), None),
            event("7", 8, "ITEM_PURCHASED", Some(3071), None),
        ];
        assert_eq!(
            first_completed_core_items(events, 1, |id| matches!(id, 6631 | 3053 | 3071)),
            vec![6631, 3053, 3071]
        );
    }

    fn event(
        source_id: &str,
        timestamp_ms: i64,
        event_type: &str,
        item_id: Option<i64>,
        before_item_id: Option<i64>,
    ) -> ItemTimelineEvent {
        ItemTimelineEvent {
            source_id: source_id.to_owned(),
            timestamp_ms,
            participant_id: 1,
            event_type: event_type.to_owned(),
            item_id,
            before_item_id,
            after_item_id: None,
        }
    }
}
