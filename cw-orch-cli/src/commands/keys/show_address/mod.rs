use cw_orch::{daemon::DaemonAsync, tokio::runtime::Runtime};

use crate::{common::seed_phrase_for_id, types::CliLockedChain};

#[derive(Debug, Clone, interactive_clap::InteractiveClap)]
#[interactive_clap(input_context = ())]
#[interactive_clap(output_context = ShowAddressOutput)]
pub struct ShowAddressCommand {
    /// Id of the key
    name: String,
    #[interactive_clap(skip_default_input_arg)]
    chain_id: CliLockedChain,
}

impl ShowAddressCommand {
    fn input_chain_id(_: &()) -> color_eyre::eyre::Result<Option<CliLockedChain>> {
        crate::common::select_chain()
    }
}

pub struct ShowAddressOutput;

impl ShowAddressOutput {
    fn from_previous_context(
        _previous_context: (),
        scope:&<ShowAddressCommand as interactive_clap::ToInteractiveClapContextScope>::InteractiveClapContextScope,
    ) -> color_eyre::eyre::Result<Self> {
        let mnemonic = seed_phrase_for_id(&scope.name)?;
        let chain = scope.chain_id;

        let rt = Runtime::new()?;
        rt.block_on(async {
            let daemon = DaemonAsync::builder()
                .chain(chain)
                .mnemonic(mnemonic)
                .build()
                .await?;
            let address = daemon.sender();
            println!("Your address: {address}");
            color_eyre::Result::<(), color_eyre::Report>::Ok(())
        })?;
        Ok(ShowAddressOutput)
    }
}