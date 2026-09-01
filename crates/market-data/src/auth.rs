use serde_json::json;
use serde_json::Value;
use vn30_domain::errors::MarketDataError;

/// Các phương thức xác thực hỗ trợ
#[derive(Clone, PartialEq, Eq)]
pub enum AuthMethod {
    None, // Dùng cho Mock Server hoặc môi trường dev
    ApiKey { api_key: String, secret_key: String },
    BearerToken(String),
}

/// Interface chung cho việc tạo payload xác thực và thẩm định phản hồi
pub trait Authenticator: Send + Sync {
    /// Sinh message/frame xác thực gửi lên sàn
    fn generate_auth_message(&self) -> Result<Option<String>, MarketDataError>;

    /// Thẩm định frame phản hồi từ sàn xem xác thực đã thành công hay thất bại
    fn verify_auth_response(&self, response: &str) -> Result<bool, MarketDataError>;
}

pub struct DefaultAuthenticator {
    method: AuthMethod,
}

impl DefaultAuthenticator {
    pub fn new(method_auth: AuthMethod) -> Self {
        Self {
            method: method_auth,
        }
    }
}

impl Authenticator for DefaultAuthenticator {
    fn generate_auth_message(&self) -> Result<Option<String>, MarketDataError> {
        match &self.method {
            AuthMethod::None => Ok(None),
            AuthMethod::ApiKey {
                api_key,
                secret_key,
            } => {
                if api_key.trim().is_empty() || secret_key.trim().is_empty() {
                    return Err(MarketDataError::AuthenticationError(
                        "API key and secret key are required".to_string(),
                    ));
                }
                let msg = json!({"type": "auth", "api_key": api_key, "secret_key": secret_key})
                    .to_string();
                return Ok(Some(msg));
            }
            AuthMethod::BearerToken(token) => {
                if token.trim().is_empty() {
                    return Err(MarketDataError::AuthenticationError(
                        "Bearer token is required".to_string(),
                    ));
                }
                let auth = json!({"type": "auth", "token": token});
                Ok(Some(auth.to_string()))
            }
        }
    }
    fn verify_auth_response(&self, response: &str) -> Result<bool, MarketDataError> {
        let res: Value = serde_json::from_str(response).map_err(|e| {
            MarketDataError::AuthenticationError(format!(
                "Failed to parse authentication response: {}",
                e
            ))
        })?;

        if res["status"] == "ok" {
            Ok(true)
        } else {
            let message = res["message"]
                .as_str()
                .unwrap_or("Unknown error")
                .to_string();
            Err(MarketDataError::AuthenticationError(message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_auth_returns_none() {
        let auth = DefaultAuthenticator::new(AuthMethod::None);
        let result = auth.generate_auth_message().unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_api_key_valid_generates_json() {
        let auth = DefaultAuthenticator::new(AuthMethod::ApiKey {
            api_key: "my_api_key".to_string(),
            secret_key: "my_secret_key".to_string(),
        });
        let result = auth.generate_auth_message().unwrap();
        assert!(result.is_some());
        let payload: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(payload["type"], "auth");
        assert_eq!(payload["api_key"], "my_api_key");
        assert_eq!(payload["secret_key"], "my_secret_key");
    }

    #[test]
    fn test_api_key_empty_or_whitespace_fails() {
        let empty_key = DefaultAuthenticator::new(AuthMethod::ApiKey {
            api_key: "   ".to_string(),
            secret_key: "secret".to_string(),
        });
        assert!(empty_key.generate_auth_message().is_err());

        let empty_secret = DefaultAuthenticator::new(AuthMethod::ApiKey {
            api_key: "key".to_string(),
            secret_key: "".to_string(),
        });
        assert!(empty_secret.generate_auth_message().is_err());
    }

    #[test]
    fn test_bearer_token_valid_generates_json() {
        let auth = DefaultAuthenticator::new(AuthMethod::BearerToken("jwt_token_123".to_string()));
        let result = auth.generate_auth_message().unwrap();
        assert!(result.is_some());
        let payload: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(payload["type"], "auth");
        assert_eq!(payload["token"], "jwt_token_123");
    }

    #[test]
    fn test_bearer_token_empty_or_whitespace_fails() {
        let empty_token = DefaultAuthenticator::new(AuthMethod::BearerToken("   ".to_string()));
        assert!(empty_token.generate_auth_message().is_err());

        let blank_token = DefaultAuthenticator::new(AuthMethod::BearerToken("".to_string()));
        assert!(blank_token.generate_auth_message().is_err());
    }

    #[test]
    fn test_verify_auth_response_success() {
        let auth = DefaultAuthenticator::new(AuthMethod::None);
        let response = r#"{"status": "ok", "message": "Authenticated"}"#;
        let result = auth.verify_auth_response(response).unwrap();
        assert!(result);
    }

    #[test]
    fn test_verify_auth_response_error_with_server_message() {
        let auth = DefaultAuthenticator::new(AuthMethod::None);
        let response = r#"{"status": "error", "message": "Invalid API key"}"#;
        let err = auth.verify_auth_response(response).unwrap_err();
        match err {
            MarketDataError::AuthenticationError(msg) => {
                assert_eq!(msg, "Invalid API key");
            }
            other => panic!("Expected AuthenticationError, got: {:?}", other),
        }
    }

    #[test]
    fn test_verify_auth_response_error_fallback_message() {
        let auth = DefaultAuthenticator::new(AuthMethod::None);
        let response = r#"{"status": "error"}"#;
        let err = auth.verify_auth_response(response).unwrap_err();
        match err {
            MarketDataError::AuthenticationError(msg) => {
                assert_eq!(msg, "Unknown error");
            }
            other => panic!("Expected AuthenticationError, got: {:?}", other),
        }
    }

    #[test]
    fn test_verify_auth_response_invalid_json_fails() {
        let auth = DefaultAuthenticator::new(AuthMethod::None);
        let response = "invalid json payload";
        let err = auth.verify_auth_response(response).unwrap_err();
        assert!(matches!(err, MarketDataError::AuthenticationError(_)));
    }
}
