use std::str::FromStr;

use sea_orm::prelude::DateTimeUtc;

use crate::graphql::types::{
    MetadataOrganizeLevel as MetadataOrganizeLevelEnum, TagType as TagTypeEnum,
    TrackType as TrackTypeEnum,
};

pub fn now() -> DateTimeUtc {
    chrono::Utc::now()
}

pub fn timestamp(input: DateTimeUtc) -> DateTimeUtc {
    input
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
