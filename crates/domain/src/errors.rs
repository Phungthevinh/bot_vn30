use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Không thể đọc file cấu hình tại '{path}': {source}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Lỗi cú pháp TOML trong cấu hình: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Dữ liệu cấu hình không hợp lệ: {0}")]
    ValidationError(String),
    #[error("Biến môi trường bắt buộc '{0}' chưa được thiết lập")]
    MissingEnvVar(String),
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Lỗi cấu hình: {0}")]
    Config(#[from] ConfigError),

    #[error("Lỗi dữ liệu: {0}")]
    InvalidData(String),
}
