use sea_orm::prelude::DateTimeUtc;
use sea_orm::ActiveEnum;

use crate::entities::sea_orm_active_enums::{MetadataOrganizeLevel, TagType, TrackType};
use crate::graphql::types::{
    MetadataOrganizeLevel as MetadataOrganizeLevelEnum, TagType as TagTypeEnum,
    TrackType as TrackTypeEnum,
};

pub fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

pub fn timestamp(input: chrono::NaiveDateTime) -> DateTimeUtc {
    input.and_utc()
}

impl ToString for TagType {
    fn to_string(&self) -> String {
        self.to_value()
    }
}

impl ToString for TrackType {
    fn to_string(&self) -> String {
        self.to_value()
    }
}

impl ToString for MetadataOrganizeLevel {
    fn to_string(&self) -> String {
        self.to_value()
    }
}

impl From<TagTypeEnum> for TagType {
    fn from(value: TagTypeEnum) -> Self {
        match value {
            TagTypeEnum::Artist => TagType::Artist,
            TagTypeEnum::Group => TagType::Group,
            TagTypeEnum::Animation => TagType::Animation,
            TagTypeEnum::Radio => TagType::Radio,
            TagTypeEnum::Series => TagType::Series,
            TagTypeEnum::Project => TagType::Project,
            TagTypeEnum::Game => TagType::Game,
            TagTypeEnum::Organization => TagType::Organization,
            TagTypeEnum::Category => TagType::Category,
            TagTypeEnum::Others => TagType::Others,
        }
    }
}

impl From<&TagType> for TagTypeEnum {
    fn from(value: &TagType) -> Self {
        match value {
            TagType::Artist => TagTypeEnum::Artist,
            TagType::Group => TagTypeEnum::Group,
            TagType::Animation => TagTypeEnum::Animation,
            TagType::Radio => TagTypeEnum::Radio,
            TagType::Series => TagTypeEnum::Series,
            TagType::Project => TagTypeEnum::Project,
            TagType::Game => TagTypeEnum::Game,
            TagType::Organization => TagTypeEnum::Organization,
            TagType::Category => TagTypeEnum::Category,
            TagType::Others => TagTypeEnum::Others,
        }
    }
}

impl From<TrackTypeEnum> for TrackType {
    fn from(value: TrackTypeEnum) -> Self {
        match value {
            TrackTypeEnum::Absolute => TrackType::Absolute,
            TrackTypeEnum::Drama => TrackType::Drama,
            TrackTypeEnum::Instrumental => TrackType::Instrumental,
            TrackTypeEnum::Normal => TrackType::Normal,
            TrackTypeEnum::Radio => TrackType::Radio,
            TrackTypeEnum::Unknown => TrackType::Unknown,
            TrackTypeEnum::Vocal => TrackType::Vocal,
        }
    }
}

impl From<&TrackType> for TrackTypeEnum {
    fn from(value: &TrackType) -> Self {
        match value {
            TrackType::Absolute => TrackTypeEnum::Absolute,
            TrackType::Drama => TrackTypeEnum::Drama,
            TrackType::Instrumental => TrackTypeEnum::Instrumental,
            TrackType::Normal => TrackTypeEnum::Normal,
            TrackType::Radio => TrackTypeEnum::Radio,
            TrackType::Unknown => TrackTypeEnum::Unknown,
            TrackType::Vocal => TrackTypeEnum::Vocal,
        }
    }
}

impl From<MetadataOrganizeLevelEnum> for MetadataOrganizeLevel {
    fn from(value: MetadataOrganizeLevelEnum) -> Self {
        match value {
            MetadataOrganizeLevelEnum::Initial => MetadataOrganizeLevel::Initial,
            MetadataOrganizeLevelEnum::Partial => MetadataOrganizeLevel::Partial,
            MetadataOrganizeLevelEnum::Reviewed => MetadataOrganizeLevel::Reviewed,
            MetadataOrganizeLevelEnum::Finished => MetadataOrganizeLevel::Finished,
        }
    }
}

impl From<&MetadataOrganizeLevel> for MetadataOrganizeLevelEnum {
    fn from(value: &MetadataOrganizeLevel) -> Self {
        match value {
            MetadataOrganizeLevel::Initial => MetadataOrganizeLevelEnum::Initial,
            MetadataOrganizeLevel::Partial => MetadataOrganizeLevelEnum::Partial,
            MetadataOrganizeLevel::Reviewed => MetadataOrganizeLevelEnum::Reviewed,
            MetadataOrganizeLevel::Finished => MetadataOrganizeLevelEnum::Finished,
        }
    }
}
