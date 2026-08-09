#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueFilter {
    All,
    RankedSolo,
    Normal,
    Aram,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeRangeFilter {
    CurrentPatch,
    CurrentSeason,
    AllTracked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalyticsFilter {
    pub queue: QueueFilter,
    pub time_range: TimeRangeFilter,
}
