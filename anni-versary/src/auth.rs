use jsonwebtoken::{decode_header, Algorithm, Validation};

// pub fn validate_jwt(token: &str) -> Result<bool, &'static str> {
//     let header = decode_header(token)?;
//     match header.alg {
//         Algorithm::HS256 => { Ok(true) }
//         Algorithm::RS256 => { Ok(true) }
//         _ => Err("Unsupported alg"),
//     }
// }