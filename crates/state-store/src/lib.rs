//! In-memory state store with DashMap and bounded rolling windows.
pub mod latest;
pub mod ohlcv;

pub use latest::{MarketStateStore, SymbolState};
pub use ohlcv::{default_timeframe_config, OhlcvStateStore, SymbolOhlcv};
