use chrono::Datelike;
use vn30_domain::config::{AppConfig, RiskLevelConfig};
use vn30_domain::errors::MarketDataError;
use vn30_domain::market::{MarketEvent, Quote, Trade};
use vn30_domain::symbol::{FutureContract, IndexSymbol, Instrument, StockSymbol};
use vn30_domain::timestamp::MarketTimestamp;

#[test]
fn test_qa_serde_bypasses_market_timestamp_invariants() {
    // Invariant: year must be 2000-2100 and timestamp > 0.
    // But serde deserialization can bypass from_utc / from_epoch_millis!
    let json_pre_2000 = r#"{"date_time":"1970-01-01T00:00:00Z"}"#;
    let ts_pre_2000: Result<MarketTimestamp, _> = serde_json::from_str(json_pre_2000);
    assert!(
        ts_pre_2000.is_ok(),
        "Serde bypasses MarketTimestamp year < 2000 invariant!"
    );
    let ts = ts_pre_2000.unwrap();
    assert_eq!(ts.as_utc().year(), 1970);

    let json_post_2100 = r#"{"date_time":"2999-12-31T23:59:59Z"}"#;
    let ts_post_2100: Result<MarketTimestamp, _> = serde_json::from_str(json_post_2100);
    assert!(
        ts_post_2100.is_ok(),
        "Serde bypasses MarketTimestamp year > 2100 invariant!"
    );
}

#[test]
fn test_qa_serde_bypasses_trade_and_quote_invariants() {
    // Trade with negative price, zero volume, empty symbol bypasses constructor via serde
    let json_invalid_trade = r#"{
        "symbol": "",
        "price": -9999.0,
        "volume": 0.0,
        "timestamp": {"date_time": "1970-01-01T00:00:00Z"}
    }"#;

    let trade_res: Result<Trade, _> = serde_json::from_str(json_invalid_trade);
    assert!(
        trade_res.is_ok(),
        "Serde directly constructs invalid Trade bypassing Trade::new!"
    );
    let t = trade_res.unwrap();
    assert_eq!(t.price, -9999.0);
    assert_eq!(t.volume, 0.0);
    assert!(t.symbol.is_empty());

    // Quote with crossed market (bid > ask) bypasses constructor via serde
    let json_crossed_quote = r#"{
        "symbol": "HPG",
        "bid_price": 30000.0,
        "bid_vol": 100.0,
        "ask_price": 25000.0,
        "ask_vol": 100.0,
        "timestamp": {"date_time": "2024-01-01T00:00:00Z"}
    }"#;
    let quote_res: Result<Quote, _> = serde_json::from_str(json_crossed_quote);
    assert!(
        quote_res.is_ok(),
        "Serde directly constructs crossed Quote bypassing Quote::new!"
    );
    let q = quote_res.unwrap();
    assert!(q.bid_price > q.ask_price);
}

#[test]
fn test_qa_quote_allows_zero_price_with_positive_volume() {
    let ts = MarketTimestamp::from_epoch_secs(1725530400).unwrap();

    // Zero bid price with 10,000 volume: buying at 0 VND?!
    let zero_bid = Quote::new("HPG".to_string(), 0.0, 10000.0, 28000.0, 500.0, ts);
    // Observe actual behavior:
    println!("Zero bid price with positive vol result: {:?}", zero_bid);
    assert!(
        zero_bid.is_ok(),
        "CONFIRMED BUG: Quote::new allows bid_price = 0.0 with positive bid_vol = 10000.0"
    );

    // Zero ask price with 10,000 volume: free shares for sale?!
    let zero_ask = Quote::new("HPG".to_string(), 28000.0, 500.0, 0.0, 10000.0, ts);
    println!("Zero ask price with positive vol result: {:?}", zero_ask);
    assert!(
        zero_ask.is_ok(),
        "CONFIRMED BUG: Quote::new allows ask_price = 0.0 with positive ask_vol = 10000.0"
    );
}

#[test]
fn test_qa_quote_negative_zero_handling() {
    let ts = MarketTimestamp::from_epoch_secs(1725530400).unwrap();
    // Negative zero in IEEE 754: -0.0 < 0.0 is false!
    let neg_zero = -0.0f64;
    let quote_res = Quote::new("HPG".to_string(), 28000.0, 100.0, neg_zero, neg_zero, ts);
    // In Rust: -0.0 < 0.0 is false, -0.0 == 0.0 is true!
    println!("Quote with -0.0 result: {:?}", quote_res);
    assert!(
        quote_res.is_ok(),
        "Quote::new accepts -0.0 as ceiling representation"
    );
}

#[test]
fn test_qa_stock_symbol_trailing_garbage_asymmetry() {
    // "HPG:GARBAGE" vs "GARBAGE:HPG"
    let trailing = StockSymbol::new("HPG:GARBAGE");
    println!("StockSymbol('HPG:GARBAGE') = {:?}", trailing);
    assert!(
        trailing.is_ok(),
        "CONFIRMED BUG: StockSymbol accepts trailing garbage 'HPG:GARBAGE'"
    );

    let leading = StockSymbol::new("GARBAGE:HPG");
    println!("StockSymbol('GARBAGE:HPG') = {:?}", leading);
    assert!(
        leading.is_err(),
        "StockSymbol rejects leading garbage 'GARBAGE:HPG'"
    );
}

#[test]
fn test_qa_instrument_parse_canonical_with_exchange_prefix() {
    // Instrument::parse_canonical works for "HOSE:HPG", but what about "HOSE:VN30" or "HOSE:VN30F2409"?
    let stock = Instrument::parse_canonical("HOSE:HPG");
    assert!(stock.is_ok());

    let index_with_prefix = Instrument::parse_canonical("HOSE:VN30");
    println!("parse_canonical('HOSE:VN30') = {:?}", index_with_prefix);
    assert!(
        index_with_prefix.is_err(),
        "CONFIRMED BUG: parse_canonical fails on 'HOSE:VN30' because it doesn't strip exchange for Index"
    );

    let future_with_prefix = Instrument::parse_canonical("HNX:VN30F2409");
    println!("parse_canonical('HNX:VN30F2409') = {:?}", future_with_prefix);
    assert!(
        future_with_prefix.is_err(),
        "CONFIRMED BUG: parse_canonical fails on 'HNX:VN30F2409' because it doesn't strip exchange for Future"
    );
}

#[test]
fn test_qa_risk_config_beta_min_unvalidated_when_beta_max_is_none() {
    // When beta_max is None (like in risky profile), beta_min is never validated!
    let invalid_risk_level = RiskLevelConfig {
        target_min: 0.20,
        target_max: 0.25,
        sl_min: -0.08,
        sl_max: -0.07,
        beta_min: Some(-999.0), // nonsensical negative beta
        beta_max: None,
    };
    let val_res = invalid_risk_level.validate();
    println!("RiskLevelConfig with beta_min = -999.0 and beta_max = None: {:?}", val_res);
    assert!(
        val_res.is_ok(),
        "CONFIRMED BUG: RiskLevelConfig allows negative beta_min when beta_max is None!"
    );

    let nan_risk_level = RiskLevelConfig {
        target_min: 0.20,
        target_max: 0.25,
        sl_min: -0.08,
        sl_max: -0.07,
        beta_min: Some(f64::NAN), // NaN beta
        beta_max: None,
    };
    let nan_res = nan_risk_level.validate();
    println!("RiskLevelConfig with beta_min = NaN and beta_max = None: {:?}", nan_res);
    assert!(
        nan_res.is_ok(),
        "CONFIRMED BUG: RiskLevelConfig allows NaN beta_min when beta_max is None!"
    );
}
