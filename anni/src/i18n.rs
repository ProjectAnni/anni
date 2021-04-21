use i18n_embed::{DesktopLanguageRequester, fluent::{
    FluentLanguageLoader, fluent_language_loader,
}, LanguageLoader};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

fn init_i18n() -> FluentLanguageLoader {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let mut references: Vec<_> = requested_languages.iter().collect();
    references.push(loader.fallback_language());
    loader.load_languages(&Localizations, &references).expect("Failed to load localization.");
    loader
}

lazy_static::lazy_static! {
    pub static ref LOCALIZATION_LOADER: FluentLanguageLoader = init_i18n();
}

#[macro_export]
macro_rules! fl {
    ($message_id: literal) => {
        //TODO: do not leak if possible
        Box::leak(i18n_embed_fl::fl!(crate::i18n::LOCALIZATION_LOADER, $message_id).into_boxed_str()) as &'static str
    };

    ($message_id: literal, $($args: expr),*) => {
        Box::leak(i18n_embed_fl::fl!(crate::i18n::LOCALIZATION_LOADER, $message_id, $($args), *).into_boxed_str()) as &'static str
    };
}

#[macro_export]
macro_rules! ll {
    ($message_id: literal) => {
        i18n_embed_fl::fl!(crate::i18n::LOCALIZATION_LOADER, $message_id)
    };

    ($message_id: literal, $($args: expr),*) => {
        i18n_embed_fl::fl!(crate::i18n::LOCALIZATION_LOADER, $message_id, $($args), *)
    };
}


#[macro_export]
macro_rules! ball {
    ($message_id: literal) => {
        bail!(crate::ll!($message_id))
    };

    ($message_id: literal, $($args: expr),*) => {
        bail!(crate::ll!($message_id, $($args), *))
    };
}
