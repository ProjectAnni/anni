use std::str::FromStr;

use crate::graphql::types::{
    MetadataOrganizeLevel as MetadataOrganizeLevelEnum, TagType as TagTypeEnum,
    TrackType as TrackTypeEnum,
};

pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

pub fn timestamp(input: chrono::DateTime<chrono::Utc>) -> i64 {
    input.timestamp()
}

impl From<&String> for TagTypeEnum {
    fn from(value: &String) -> Self {
        TagTypeEnum::from_str(&value).unwrap()
    }
}

impl From<&String> for TrackTypeEnum {
    fn from(value: &String) -> Self {
        TrackTypeEnum::from_str(&value).unwrap()
    }
}

impl From<&String> for MetadataOrganizeLevelEnum {
    fn from(value: &String) -> Self {
        MetadataOrganizeLevelEnum::from_str(&value).unwrap()
    }
}

impl From<TagTypeEnum> for String {
    fn from(value: TagTypeEnum) -> Self {
        value.to_string()
    }
}

impl From<TrackTypeEnum> for String {
    fn from(value: TrackTypeEnum) -> Self {
        value.to_string()
    }
}

impl From<MetadataOrganizeLevelEnum> for String {
    fn from(value: MetadataOrganizeLevelEnum) -> Self {
        value.to_string()
    }
}
