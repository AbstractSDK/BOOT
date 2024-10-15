use cw_orch_core::environment::{ChainInfo, ChainKind, NetworkInfo};

// ANCHOR: bitsong
pub const BITSONG_NETWORK: NetworkInfo = NetworkInfo {
    chain_name: "bitsong",
    pub_address_prefix: "bitsong",
    coin_type: 639u32,
};

pub const BITSONG_1: ChainInfo = ChainInfo {
    kind: ChainKind::Mainnet,
    chain_id: "bitsong-1",
    gas_denom: "ubtsg",
    gas_price: 0.025,
    grpc_urls: &["http://grpc-bitsong-ia.cosmosia.notional.ventures:443"],
    network_info: BITSONG_NETWORK,
    lcd_url: None,
    fcd_url: None,
};

pub const BOBNET: ChainInfo = ChainInfo {
    kind: ChainKind::Testnet,
    chain_id: "bobnet",
    gas_denom: "ubtsg",
    gas_price: 0.025,
    grpc_urls: &["http://grpc-testnet.explorebitsong.com:443"],
    network_info: BITSONG_NETWORK,
    lcd_url: None,
    fcd_url: None,
};

pub const LOCAL_BITSONG: ChainInfo = ChainInfo {
    kind: ChainKind::Local,
    chain_id: "localbitsong",
    gas_denom: "ubtsg",
    gas_price: 0.0026,
    grpc_urls: &["tcp://localhost:9094"],
    network_info: BITSONG_NETWORK,
    lcd_url: None,
    fcd_url: None,
};
// ANCHOR_END: bitsong
