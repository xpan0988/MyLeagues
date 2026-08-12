//! Canonical user-facing queue labels. Persist numeric Riot queue IDs; resolve
//! labels only at product boundaries.

pub fn display_name(queue_id: i64) -> String {
    match queue_id {
        420 => "Ranked Solo/Duo".into(),
        400 => "Normal Draft".into(),
        430 => "Normal Blind".into(),
        450 => "ARAM".into(),
        480 => "Swiftplay".into(),
        490 => "Quickplay".into(),
        value => format!("Queue {value}"),
    }
}

#[cfg(test)]
mod tests {
    use super::display_name;

    #[test]
    fn uses_semantic_known_queue_labels_and_an_explicit_unknown_fallback() {
        assert_eq!(display_name(480), "Swiftplay");
        assert_eq!(display_name(420), "Ranked Solo/Duo");
        assert_eq!(display_name(1234), "Queue 1234");
    }
}
