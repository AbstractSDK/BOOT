use cosmwasm_std::{coin, CosmosMsg, IbcMsg, IbcTimeout, IbcTimeoutBlock};
use cw_orch::{
    environment::{QueryHandler, TxHandler},
    mock::cw_multi_test::Executor,
};
use cw_orch_interchain_core::InterchainEnv;
use cw_orch_interchain_mock::MockInterchainEnv;
use ibc_relayer_types::core::ics24_host::identifier::PortId;

#[test]
fn timeout_packet_mock() -> cw_orch::anyhow::Result<()> {
    pretty_env_logger::init();

    let interchain = MockInterchainEnv::new(vec![("juno-1", "sender"), ("stargaze-1", "sender")]);

    let channel = interchain.create_channel(
        "juno-1",
        "stargaze-1",
        &PortId::transfer(),
        &PortId::transfer(),
        "ics20-1",
        None,
    )?;
    let juno = interchain.chain("juno-1")?;
    let stargaze = interchain.chain("stargaze-1")?;

    let stargaze_height = stargaze.block_info()?;
    let channel = channel
        .interchain_channel
        .get_ordered_ports_from("juno-1")?;

    juno.add_balance(juno.sender_addr().to_string(), vec![coin(100_000, "ujuno")])?;
    let tx_resp = juno.app.borrow_mut().execute(
        juno.sender(),
        CosmosMsg::Ibc(IbcMsg::Transfer {
            channel_id: channel.0.channel.unwrap().to_string(),
            to_address: stargaze.sender_addr().to_string(),
            amount: coin(100_000, "ujuno"),
            timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                revision: 1,
                height: stargaze_height.height,
            }),
        }),
    )?;

    let result = interchain.wait_ibc("juno-1", tx_resp)?;

    match &result.packets[0].outcome {
        cw_orch_interchain_core::types::IbcPacketOutcome::Timeout { .. } => {}
        cw_orch_interchain_core::types::IbcPacketOutcome::Success { .. } => {
            panic!("Expected timeout")
        }
    }

    Ok(())
}
