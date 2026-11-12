#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeChange {
    Created(String),
    Appended(String),
    Refreshed(String),
    Unchanged,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestInfo {
    pub description: Option<String>,
    pub entry_points: Vec<String>,
    pub main_folders: Vec<(String, &'static str)>,
}
