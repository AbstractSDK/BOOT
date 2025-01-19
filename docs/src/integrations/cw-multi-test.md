# CW Multi Test

Cw Multi Test is a rust-based test framework that allows developers to test  for multi-contract interactions without having to dispatch messages, storage and other variables themselves. With cw-orchestrator, most of the `cw-multi-test` logic is abstracted away, but you can still <a href="https://github.com/CosmWasm/cw-multi-test" target="_blank">learn more about the framework here</a>.

> **⚠️ Custom-typed contracts**: cw-orch's integration with cw-multi-test is not compatible with custom-typed contracts.
> 
> Custom-typed contracts are contracts where the endpoints return `Response<M>` where `M`is not `cosmwasm_std::Empty` OR where the `Deps` and `DepsMut` endpoint arguments have a non `Empty` generic parameter specified. 
>
> You can still test and deploy your contracts on actual nodes (testnets, mainnets). To achieve that, don't specify the `wrapper` function in the `Uploadable` trait implementation on your interface.

## Quick Start

Before starting, here are a few examples utilizing the mock structures:

- <a href="https://github.com/AbstractSDK/cw-orchestrator/blob/main/cw-orch/examples/mockbech32.rs" target="_blank">Using actual cosmos addresses</a>
- <a href="https://github.com/AbstractSDK/cw-orchestrator/blob/main/cw-orch/examples/mock.rs" target="_blank">Customizing the execution environment</a>

The `cw-multi-test` integration comes at no extra cost for the developer. Creating a test environment in cw-orchestrator that leverages `cw-multi-test` goes along the lines of:

```rust,ignore
    use cw_orch::prelude::*;
    use cosmwasm_std::Addr;
{{#include ../../../cw-orch/examples/mock.rs:mock_creation}}
```

> **_NOTE:_** When using `cw-multi-test`, the addresses ARE NOT validated like on a live chain. Therefore, you can use any string format for designating addresses. For instance,`Addr::unchecked("my-first-sender")` is a valid `cw-multi-test` address.

> **_NOTE:_** When using `cw-multi-test`, NO gas fees are charged to the sender address.

## Interacting with contracts

You can then use the resulting `Mock` variable to interact with your [contracts](../contracts/index.md):

```rust,ignore
{{#include ../../../cw-orch/examples/mock.rs:mock_usage}}
```

When executing contracts in a `cw-multi-test` environment, the messages and sub-messages sent along the Response of an endpoint, will be executed as well.
This environment mocks the actual on-chain execution exactly.

> - This environment uses the actual functions of your contract **without** having to compile them into WASM. When you are calling `upload` with this environment, no wasm files are included in the test environment. This allows for better debugging of your contract code.
>
> - You will need to have implemented the `wrapper` function for interacting the `Mock` environment. This function will allow you to "connect" your contract endpoints to your `Contract` struct. [See the dedicated page for more details](../contracts/interfaces.md#creating-an-interface).
>
> - **_NOTE:_** Keep in mind that `cw-multi-test` is based solely in rust and that a lot of actual blockchain modules are not mocked in the environment. The main cosmos modules are there (Bank, Staking), but some very useful ones (tokenfactory, ibc) as well as Stargate messages are not supported by the environment.

## Cloning

When cloning a cw_multi_test environment, you are not cloning the entire environment, but instead you are creating a new `Mock` typed variable with the same underlying `cw_multi_test::App` object reference. This is useful for objects that require to pass the chain as an object rather than by reference.
The underlying `cw_multi_test::App` object is however not clonable.

## Bech32

When testing your smart contracts, you may need to use valid bech32 addresses instead of arbitrary strings. Notably Bech32 addresses are a requirement whenever `instantiate2` is used in your codebase. Therefore `cw-orch` allows this behavior using the `MockBech32` struct. This structure has the same features as the above described `Mock` with the following differences:

- The constructor now takes the bech32 prefix instead of the sender. Under the hood, the environment creates an address that will be used as the default sender for actions executed on that environment:

  ```rust,ignore
  let mock = MockBech32::new("<chain-prefix>");
  // For instance: 
  let mock = MockBech32::new("juno");
  // With chain id: 
  let mock = MockBech32::new_with_chain_id("juno", "juno-1");
  // Default sender address for this env
  let sender = mock.sender();
  ```

- To facilitate the creation of additional valid bech32 addresses, we have added an `addr_make` function. This function uses its string argument to create a new address. That way you can create a new address without having to care about bech32 encoding/decoding yourself:

   ```rust,ignore
    let named_sender = mock.addr_make("sender");
    /// This creates a new address with some funds inside of it
    let rich_sender = mock.addr_make_with_balance("sender_with_funds", coins(100_000_000_000_000, "ujuno"));
   ```

## Snapshot testing

`cw-orch` provides snapshot testing capabilities to assist you catching breaking changes to your contracts. The `Mock::take_storage_snapshot` function allows you to dump all the deployed contracts' storage values into <a href="https://insta.rs/docs/quickstart/" target="_blank">insta.rs</a> that executes snapshot testing. An example application of this feature is to make sure that the storage of your contracts don't change when migrating a contract. Using this tool, you should have a test that looks something like this:

```rust,ignore

#[test]
fn storage_stays_the_same(){
    let mock = Mock::new(Addr::unchecked("sender"));

    ... // Upload, instantiate, execute contracts

    // Make sure that the operations have a fixed result
    mock.take_storage_snapshot("mock_snapshot")?;
}
```

At any point of development, if the storage variables are modified, this test will fail and alert you that you are doing breaking changes to your storage variables. Learn more about the underlying tool in the <a href="https://insta.rs/" target="_blank">official documentation</a>.

## Additional tools

The `Mock` test environment allows you to change application variables (such as the balance of an account) using wrappers around the underlying `cw_multi_test::App` object. Here are some examples of those wrappers in context:

```rust,ignore
{{#include ../../../cw-orch/examples/mock.rs:mock_customization}}
```

## Additional customization

As we don't provide wrappers around each and every functionality that `cw-multi-test` provides, you can also customize the underlying `cw_multi_test::App`object to your specific needs. In the following example, we create a new validator in the test environment:

```rust,ignore
{{#include ../../../cw-orch/examples/mock.rs:deep_mock_customization}}
````
