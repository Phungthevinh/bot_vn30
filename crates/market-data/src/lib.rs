//! Market data ingestion adapters, WebSocket client, and reconnect logic.

pub mod auth;
pub mod health;
pub mod parser;
pub mod subscription;
pub mod websocket;

pub use auth::{AuthMethod, Authenticator, DefaultAuthenticator};
pub use health::{HealthMonitor, HealthStatus};
pub use parser::{
    ExchangeErrorEvent, HeartbeatEvent, MarketDataParser, MarketMessage, QuoteEvent, TradeEvent,
};
pub use subscription::SubscriptionManager;
pub use websocket::{ConnectionState, RawMarketMessage, WebSocketClient};
