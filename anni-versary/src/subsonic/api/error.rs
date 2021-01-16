use serde::Serialize;

#[derive(Serialize, PartialEq)]
#[serde(rename = "error")]
pub(crate) struct SonicError {
    code: u8,
    message: String,
}

impl SonicError {
    fn new(kind: ErrorKind, message: &str) -> Self {
        SonicError {
            code: kind as u8,
            message: message.to_owned(),
        }
    }
}

pub enum ErrorKind {
    Generic = 0,
    RequiredParameterMissing = 10,
    IncompatibleClient = 20,
    NotImplemented = 30,
    WrongUsernameOrPassword = 40,
    TokenNotSupportedForLDAP = 41,
    Unauthorized = 50,
    TrialOver = 60,
    NotFound = 70,
}
