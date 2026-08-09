#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuneSelectionType {
    Primary,
    Secondary,
    StatShard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuneSelection {
    pub selection_type: RuneSelectionType,
    pub slot: i64,
    pub rune_id: i64,
    pub style_id: Option<i64>,
}
