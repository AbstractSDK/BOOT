# Mock Interchain Environment

This environment allows you to test your IBC application inside your Rust environment without having to create new crates, new packages or even run other programs in the background. This is perfect for iterating through your designs and testing on the go without additional wait time or setup tutorials.

## Environment creation

You can create your interchain environment using the following simple setup script:

```rust,ignore
use cw_orch::prelude::*;
use cw_orch_interchain::prelude::*;
fn main(){
    // Here `juno-1` is the chain-id and `juno` is the address prefix for this chain    
    # #[allow(unused)]
    let interchain = MockBech32InterchainEnv::new(
        vec![("juno-1", "juno"), ("osmosis-1", "osmo")],
    );
}
```

Behind the scenes, `MockBech32` objects are created with the specified chain ids. These mock environments can be used on their own to interact with testing environment directly. You can get those objects like so:

```rust,ignore
# use cw_orch::prelude::*;
# use cw_orch_interchain::prelude::*;
# fn main() -> anyhow::Result<()>{
    # let interchain = MockBech32InterchainEnv::new(
    #      vec![("juno-1", "juno"), ("osmosis-1", "osmosis")],
    # );
    # #[allow(unused)]
    let local_juno = interchain.get_chain("juno-1")?;
    # #[allow(unused)]
    let local_osmo = interchain.get_chain("osmosis-1")?;
    # Ok(())
# }
```

where the argument of the `chain` method is the chain id of the chain you are interacting with. Note that this environment can't work with chains that have the same `chain_id`.

You can also add mocks manually to the `interchain` object, after instantiation:

```rust,ignore
# fn main(){
    # use cw_orch::prelude::*;
    # use cw_orch_interchain::prelude::*;
    # let mut interchain = MockInterchainEnv::new(
    #    vec![("juno-1", "juno"), ("osmosis-1", "osmosis")],
    # );
    let test_migaloo = MockBech32::new_with_chain_id("migaloo-1","migaloo");
    interchain.add_mocks(vec![test_migaloo]);
# }
```

## General Usage

All interchain environments are centered around the `await_single_packet` function. In the `Mock` case, this function is responsible for relaying the packets between the different chains. Using the exact same interface as with other environments, it takes care of all packet relaying procedures.

This function will relay a packets successfully from the receiving chain back the source chain. Here is what the full cycle looks like:

1. <span style="color:purple">⬤</span> On the `source chain`, it queries the packet data associated with the packet channel and sequence.
2. <span style="color:red">⬤</span> On the `destination chain`, it triggers a receive transaction for that packet.
3. <span style="color:purple">⬤</span> On the `source chain`, it finally triggers an acknowledgement transaction with the data the `destination_chain` returned.

The `await_packets` function is very similar except that instead of following a single packet, it follows all packets that are being sent within a transaction. This works in a very similar manner and will allows for testing timeouts on IBC packets. This function is recursive as it will also look for packets inside the receive/ack transactions and also follow their IBC cycles. You can think of this function as going down the rabbit-hole of IBC execution and only returning when all IBC interactions are complete.

Finally, the recommended function is `await_and_check_packets`. This will run `await_packets` and then make sure that the resulting IBC acknowledgments and executions are successful. You can find more live examples <a target="_blank" href="https://github.com/AbstractSDK/cw-orchestrator/blob/dd7238f3108ad38b416171c3b1b2f8a0ce368539/cw-orch-interchain/tests/common/ica_demo.rs">here</a>. Here is an example usage:

```rust,ignore
# use cw_orch_interchain::prelude::*;
# fn main() -> anyhow::Result<()>{
    # let mut interchain = MockBech32InterchainEnv::new(
    #    vec![("juno-1", "juno"), ("osmosis-1", "osmosis")],
    # );
    # let chain = interchain.get_chain("juno-1")?;
    # let contract = Contract::new(chain);
    let response = contract.execution_with_ibc_consequences()?;

    interchain.await_and_check_packets("juno-1", response)?;

    let port_id = PortId::transfer();
    
    let ChannelCreationResult {
        interchain_channel,
        channel_creation_txs,
    } = interchain
        .create_channel(
            "juno-1", 
            "osmosis-1", 
            &port_id, 
            &port_id, 
            "ics20-1",
            Some(cosmwasm_std::IbcOrder::Unordered)
        )?;
    # Ok(())
# }
```

## IBC Channel creation

cw-orchestrator also provides tooling for creating channels between mock environments. Here is how you do it:

```rust,ignore
# use cw_orch_interchain::prelude::*;
# fn main() -> anyhow::Result<()>{
    let port_id = PortId::transfer();
    # let mut interchain = MockBech32InterchainEnv::new(
    #    vec![("juno-1", "juno"), ("osmosis-1", "osmosis")],
    # );
    let ChannelCreationResult {
        interchain_channel,
        channel_creation_txs,
    } = interchain
        .create_channel(
            "juno-1", 
            "osmosis-1", 
            &port_id, 
            &port_id, 
            "ics20-1",
            Some(cosmwasm_std::IbcOrder::Unordered)
        )?;
    # Ok(())
# }
```

- The resulting `interchain_channel` object allows you to identify the channel that was just created. It can be useful to retrieve the channel identifiers for instance (e.g. `channel-567`)
- The resulting `channel_creation_txs` object allows you to identify the different steps of channel creation as well as the IBC packets sent during this creation. This is very useful to analyze the effects of the channel creation on external contracts and structures.
