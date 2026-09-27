//! In-memory state store with DashMap and bounded rolling windows.
pub mod latest;

pub use latest::{MarketStateStore, SymbolState};

