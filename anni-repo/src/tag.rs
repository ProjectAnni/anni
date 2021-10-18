use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnniTag {
    name: String,
    edition: Option<String>,
}
