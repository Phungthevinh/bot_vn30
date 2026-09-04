//! Market data ingestion adapters, WebSocket client, and reconnect logic.

pub mod auth;
pub mod health;
pub mod parser;
pub mod reconnect;
pub mod subscription;
pub mod symbol_mapper;
pub mod websocket;

pub use auth::{AuthMethod, Authenticator, DefaultAuthenticator};
pub use health::{HealthMonitor, HealthStatus};
pub use parser::{
    ExchangeErrorEvent, HeartbeatEvent, MarketDataParser, MarketMessage, QuoteEvent, TradeEvent,
};
pub use subscription::SubscriptionManager;
pub use symbol_mapper::SymbolMapper;
pub use websocket::{ConnectionState, RawMarketMessage, WebSocketClient};
