use serde::Serialize;
use crate::subsonic::api::Response;

/// Used to test connectivity with the server. Takes no extra parameters.
///
/// Returns an empty `<subsonic-response>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/ping_example_1.xml
#[get("/ping")]
pub fn ping() -> Response {
    unimplemented!()
}

/// Get details about the software license. Takes no extra parameters.
/// Please note that access to the REST API requires that the server has a valid license (after a 30-day trial period).
/// To get a license key you must upgrade to Subsonic Premium.
///
/// Returns a `<subsonic-response>` element with a nested `<license>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/license_example_1.xml
#[get("/getLicense")]
pub fn get_license() -> Response {
    unimplemented!()
}

/// <xs:complexType name="License">
#[derive(Serialize, PartialEq)]
#[serde(rename = "error")]
pub(crate) struct SonicLicense {
    /// <xs:attribute name="valid" type="xs:boolean" use="required"/>
    valid: bool,
}