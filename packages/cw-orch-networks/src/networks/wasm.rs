use crate::chain_info::{ChainInfo, ChainKind, NetworkInfo};

// ANCHOR: wasmd
pub const WASM_NETWORK: NetworkInfo = NetworkInfo {
    id: "wasm",
    pub_address_prefix: "wasm",
    coin_type: 118u32,
};

pub const LOCAL_WASM: ChainInfo = ChainInfo {
    kind: ChainKind::Local,
    chain_id: "testing",
    gas_denom: "ucosm",
    gas_price: 0.0,
    grpc_urls: &["http://localhost:9090"],
    rpc_urls: &["http://localhost:26657"],
    network_info: WASM_NETWORK,
    lcd_url: None,
    fcd_url: None,
};
// ANCHOR_END: wasmd