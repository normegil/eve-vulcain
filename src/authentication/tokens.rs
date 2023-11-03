use std::num::ParseIntError;

use jsonwebtoken::Algorithm::RS256;
use jsonwebtoken::{DecodingKey, TokenData, Validation};
use rfesi::prelude::TokenClaims;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Access Token decoding failed: {source}")]
    AccessTokenDecodingError { source: jsonwebtoken::errors::Error },
}

#[derive(Debug, Error, PartialEq)]
pub enum CharacterIDError {
    #[error("Could not parse character ID from token '{sub}': {source}")]
    ParseTokenError { sub: String, source: ParseIntError },
    #[error("Could not found character ID from token '{sub}'")]
    NotFoundInSubError { sub: String },
}

pub struct TokenHelper {
    pub api_client_id: String,
}

impl TokenHelper {
    pub fn decode(&self, access_token: &str) -> Result<TokenData<TokenClaims>, TokenError> {
        let key = DecodingKey::from_secret(&[]);
        let mut validation = Validation::new(RS256);
        validation.insecure_disable_signature_validation();
        validation.set_audience(&[self.api_client_id.clone(), "EVE Online".to_string()]);
        let token_data: TokenData<TokenClaims> =
            jsonwebtoken::decode(access_token, &key, &validation)
                .map_err(|src| TokenError::AccessTokenDecodingError { source: src })?;
        Ok(token_data)
    }

    pub fn character_id(&self, claim: &TokenClaims) -> Result<Option<u64>, CharacterIDError> {
        let sub = claim.sub.as_str();
        let mut splitted = sub.split(':');
        splitted.next();
        splitted.next();
        let character_id = splitted
            .next()
            .ok_or(CharacterIDError::NotFoundInSubError {
                sub: sub.to_string(),
            })?;
        let character_id =
            character_id
                .parse::<u64>()
                .map_err(|source| CharacterIDError::ParseTokenError {
                    sub: sub.to_string(),
                    source,
                })?;
        Ok(Some(character_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test tokens can be decoded using https://jwt.io/

    #[test]
    fn decode_valid_token() {
        let token_helper = TokenHelper {
            api_client_id: "test_client_id".to_string(),
        };
        let valid_token = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzY3AiOlsicHVibGljRGF0YSIsImVzaS1sb2NhdGlvbi5yZWFkX2xvY2F0aW9uLnYxIiwiZXNpLXNraWxscy5yZWFkX3NraWxscy52MSIsImVzaS13YWxsZXQucmVhZF9jaGFyYWN0ZXJfd2FsbGV0LnYxIiwiZXNpLXNlYXJjaC5zZWFyY2hfc3RydWN0dXJlcy52MSIsImVzaS11bml2ZXJzZS5yZWFkX3N0cnVjdHVyZXMudjEiLCJlc2ktaW5kdXN0cnkucmVhZF9jaGFyYWN0ZXJfam9icy52MSIsImVzaS1tYXJrZXRzLnJlYWRfY2hhcmFjdGVyX29yZGVycy52MSJdLCJqdGkiOiIyODY4OGU0ZS0wN2NmLTRjYzItODA2Yy0wMzJmNjlkZGQwMTgiLCJraWQiOiJKV1QtU2lnbmF0dXJlLUtleSIsInN1YiI6IkNIQVJBQ1RFUjpFVkU6MTIzNDU2Nzg5IiwiYXpwIjoiZTdmMmU1ZjlhNTQ3NGVjN2IwOGU5ZmIyNDliYzYyZDkiLCJ0ZW5hbnQiOiJ0cmFucXVpbGl0eSIsInRpZXIiOiJsaXZlIiwicmVnaW9uIjoid29ybGQiLCJhdWQiOlsidGVzdF9jbGllbnRfaWQiLCJFVkUgT25saW5lIl0sIm5hbWUiOiJJc2hva2VyYSBJY2hpbnVtaSIsIm93bmVyIjoiL0ZuOGdSbXJzQnhvWHNxMlVRRE9wQjhaMExnPSIsImV4cCI6MTcwNTg2NzA4OTk5OTk5LCJpYXQiOjE3MDU4NjU4ODAsImlzcyI6Imh0dHBzOi8vbG9naW4uZXZlb25saW5lLmNvbSJ9.K-3iCA-iIy0VmNA9UVUsK7rD9oaRmVZoaxZeC3idHk43KdvmRaojUdVEJDo_0UNMhGwyKf6jDtj--RS9F4MEQyO1bsgofVEF3YCVpalbEyw76CEyWQuA82DOksXqI5tsrvS8qCB_d8l2etZuNignkRbOSb6dPrTfzubUl1ak5_QmREYGNyBwGT5ytNJdJLKs88TPTmZRo2Qdw4yrYyZeSnHSpbVRBR9Qe3-TNpWWbmmY9R0kknemH6h7uF4BH7H7z884iIvvEs6xCBnBJEInU1XifHMDNgXbbgUMOZAKXHkHhxztXeMpKRGugeIFB4Ikevh-l3-hmjy5_WzXuUjiRg";
        let result = token_helper.decode(valid_token).unwrap();
        assert!(result.claims.aud.contains(&"EVE Online".to_string()));
        assert!(result.claims.aud.contains(&"test_client_id".to_string()));
        assert_eq!(result.claims.sub, "CHARACTER:EVE:123456789");
    }

    #[test]
    fn decode_invalid_token() {
        let token_helper = TokenHelper {
            api_client_id: "test_client_id".to_string(),
        };
        let invalid_token = "invalid_access_token";
        let result = token_helper.decode(invalid_token);
        assert!(result.is_err());
    }

    #[test]
    fn character_id_valid_claim() {
        let token_helper = TokenHelper {
            api_client_id: "test_client_id".to_string(),
        };
        let valid_claim = TokenClaims {
            sub: "CHARACTER:EVE:123456789".to_string(),
            aud: vec!["test_client_id".to_string(), "EVE Online".to_string()],
            azp: "e7f2e5f9a5474ec7b08e9fb249bc62d9".to_string(),
            exp: 170586708999999,
            iat: 1705865880,
            iss: "https://login.eveonline.com".to_string(),
            jti: "28688e4e-07cf-4cc2-806c-032f69ddd018".to_string(),
            kid: "JWT-Signature-Key".to_string(),
            name: "John Doe".to_string(),
            owner: "/Fn8gRmrsBxoXsq2UQDOpB8Z0Lg=".to_string(),
            region: "world".to_string(),
            scp: None,
            tenant: "tranquility".to_string(),
            tier: "tier".to_string(),
        };
        let result = token_helper.character_id(&valid_claim);
        assert_eq!(result, Ok(Some(123456789)));
    }

    #[test]
    fn character_id_invalid_claim() {
        let token_helper = TokenHelper {
            api_client_id: "test_client_id".to_string(),
        };
        let invalid_claim = TokenClaims {
            sub: "CHARACTER:EVE:invalid".to_string(),
            aud: vec!["test_client_id".to_string(), "EVE Online".to_string()],
            azp: "e7f2e5f9a5474ec7b08e9fb249bc62d9".to_string(),
            exp: 170586708999999,
            iat: 1705865880,
            iss: "https://login.eveonline.com".to_string(),
            jti: "28688e4e-07cf-4cc2-806c-032f69ddd018".to_string(),
            kid: "JWT-Signature-Key".to_string(),
            name: "John Doe".to_string(),
            owner: "/Fn8gRmrsBxoXsq2UQDOpB8Z0Lg=".to_string(),
            region: "world".to_string(),
            scp: None,
            tenant: "tranquility".to_string(),
            tier: "tier".to_string(),
        };
        let result = token_helper.character_id(&invalid_claim);
        assert!(result.is_err());
    }
}
