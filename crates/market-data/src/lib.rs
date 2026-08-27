//! Market data ingestion adapters, WebSocket client, and reconnect logic.

pub mod websocket;

pub use websocket::{ConnectionState, RawMarketMessage, WebSocketClient};
