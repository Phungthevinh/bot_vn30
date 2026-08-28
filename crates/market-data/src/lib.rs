//! Market data ingestion adapters, WebSocket client, and reconnect logic.

pub mod auth;
pub mod websocket;

pub use auth::{AuthMethod, Authenticator, DefaultAuthenticator};
pub use websocket::{ConnectionState, RawMarketMessage, WebSocketClient};
