use counter_contract::CounterContract;
use cw_orch::prelude::*;
use cw_orch_clone_testing::CloneTesting;
use cw_orch_daemon::networks::JUNO_1;
use networks::OSMOSIS_1;

#[test]
fn multiple_upload() -> anyhow::Result<()> {
    let mut chain = OSMOSIS_1;
    // ANCHOR: clone_testing_setup
    let chain = CloneTesting::new(chain)?;
    // ANCHOR_END: clone_testing_setup
    // ANCHOR: counter_contract_setup
    let contract = CounterContract::new(chain.clone());
    // ANCHOR_END: counter_contract_setup

    // Either upload using the RUST code (`wrapper`)
    // ANCHOR: upload
    contract.upload()?;
    // ANCHOR_END: upload
    let code_id = contract.code_id()?;

    // OR upload using the wasm binaries
    // ANCHOR: upload_wasm
    use cw_orch_clone_testing::WasmUpload;
    contract.upload_wasm()?;
    // ANCHOR_END: upload_wasm
    let new_code_id = contract.code_id()?;

    assert_ne!(new_code_id, code_id);

    Ok(())
}
