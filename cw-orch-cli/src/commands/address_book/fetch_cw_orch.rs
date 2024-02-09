use std::{fs::File, str::FromStr};

use cw_orch::daemon::DEFAULT_DEPLOYMENT;
use serde_json::Value;
use strum::IntoEnumIterator;

const STATE_FILE_DAMAGED_ERROR: &str = "State file is corrupted";

use crate::types::{address_book, CliSkippable};

use super::AddresBookContext;

#[derive(Debug, strum::EnumDiscriminants, strum::Display, Clone, clap::ValueEnum)]
#[strum_discriminants(derive(strum::EnumMessage, strum::EnumIter))]
pub enum AliasNameStrategy {
    #[strum(serialize = "keep")]
    #[strum_discriminants(strum(message = "Keep contract ids as name aliases"))]
    /// Keep contract ids as name aliases
    Keep,
    #[strum(serialize = "rename")]
    #[strum_discriminants(strum(message = "Give prompt to rename aliases"))]
    /// Give prompt to rename aliases
    Rename,
}

impl interactive_clap::ToCli for AliasNameStrategy {
    type CliVariant = AliasNameStrategy;
}

impl std::fmt::Display for AliasNameStrategyDiscriminants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AliasNameStrategyDiscriminants::Keep => write!(f, "Keep"),
            AliasNameStrategyDiscriminants::Rename => write!(f, "Rename"),
        }
    }
}

impl FromStr for AliasNameStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "keep" => Ok(Self::Keep),
            "rename" => Ok(Self::Rename),
            _ => Err("AliasNameStrategy: incorrect alias name strategy".to_string()),
        }
    }
}

#[derive(Debug, Clone, interactive_clap::InteractiveClap)]
#[interactive_clap(input_context = AddresBookContext)]
#[interactive_clap(output_context = FetchAddressesOutput)]
pub struct FetchAddresses {
    /// Deployment id, leave empty for default
    deployment_id: CliSkippable<String>,
    #[interactive_clap(value_enum)]
    #[interactive_clap(skip_default_input_arg)]
    /// Alias names strategy
    name_strategy: AliasNameStrategy,
}

impl FetchAddresses {
    fn input_name_strategy(
        _context: &AddresBookContext,
    ) -> color_eyre::eyre::Result<Option<AliasNameStrategy>> {
        let variants = AliasNameStrategyDiscriminants::iter().collect::<Vec<_>>();
        let selected = inquire::Select::new("Select alias names strategy", variants).prompt()?;
        match selected {
            AliasNameStrategyDiscriminants::Keep => Ok(Some(AliasNameStrategy::Keep)),
            AliasNameStrategyDiscriminants::Rename => Ok(Some(AliasNameStrategy::Rename)),
        }
    }
}

pub struct FetchAddressesOutput;

#[derive(Debug, strum::EnumDiscriminants, strum::Display, Clone)]
#[strum_discriminants(derive(strum::EnumMessage, strum::EnumIter))]
pub enum DuplicateResolve {
    #[strum_discriminants(strum(message = "Rename duplicate"))]
    Rename,
    #[strum_discriminants(strum(message = "Skip duplicate"))]
    Skip,
    #[strum_discriminants(strum(message = "Override duplicate"))]
    Override,
    #[strum_discriminants(strum(message = "Skip all duplicates"))]
    SkipAll,
    #[strum_discriminants(strum(message = "Override all duplicates"))]
    OverrideAll,
}

impl std::fmt::Display for DuplicateResolveDiscriminants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DuplicateResolveDiscriminants::Rename => write!(f, "Rename"),
            DuplicateResolveDiscriminants::Skip => write!(f, "Skip"),
            DuplicateResolveDiscriminants::Override => write!(f, "Override"),
            DuplicateResolveDiscriminants::SkipAll => write!(f, "Skip All"),
            DuplicateResolveDiscriminants::OverrideAll => write!(f, "Override All"),
        }
    }
}

impl FetchAddressesOutput {
    fn from_previous_context(
        previous_context: AddresBookContext,
        scope: &<FetchAddresses as interactive_clap::ToInteractiveClapContextScope>::InteractiveClapContextScope,
    ) -> color_eyre::eyre::Result<Self> {
        let state_file = cw_orch::daemon::DaemonState::state_file_path()?;
        let deployment_id = scope
            .deployment_id
            .0
            .as_deref()
            .unwrap_or(DEFAULT_DEPLOYMENT);

        let chain_name = previous_context.chain.chain_info().network_info.id;
        let chain_id = previous_context.chain.chain_info().chain_id;

        let json = read(&state_file)?;

        let Some(chain_state) = json.get(chain_name) else {
            return Err(color_eyre::eyre::eyre!("State is empty for {chain_name}"));
        };

        let Some(chain_id_state) = chain_state.get(chain_id) else {
            return Err(color_eyre::eyre::eyre!(
                "State is empty for {chain_name}.{chain_id}"
            ));
        };

        let Some(deployment) = chain_id_state.get(deployment_id) else {
            return Err(color_eyre::eyre::eyre!(
                "State is empty for {chain_name}.{chain_id}.{deployment_id}"
            ));
        };

        let contracts = deployment
            .as_object()
            .ok_or(color_eyre::eyre::eyre!(STATE_FILE_DAMAGED_ERROR))?;

        let mut duplicate_resolve_global = None;
        for (contract_id, address) in contracts {
            let address = address
                .as_str()
                .ok_or(color_eyre::eyre::eyre!(STATE_FILE_DAMAGED_ERROR))?;
            let mut alias = match scope.name_strategy {
                AliasNameStrategy::Keep => contract_id.clone(),
                AliasNameStrategy::Rename => inquire::Text::new("Input new contract alias")
                    .with_initial_value(contract_id)
                    .prompt()?,
            };
            let maybe_address = address_book::get_account_id(chain_id, &alias)?;

            if maybe_address.is_some() {
                // Duplicate happened
                let duplicate_resolve = if let Some(global_resolved) = &duplicate_resolve_global {
                    // Check if it's already globally resolved
                    match global_resolved {
                        DuplicateResolve::SkipAll => DuplicateResolve::Skip,
                        DuplicateResolve::OverrideAll => DuplicateResolve::Override,
                        _ => unreachable!(),
                    }
                } else {
                    // Or input new one
                    input_duplicate_resolve(&alias)?
                };

                match duplicate_resolve {
                    DuplicateResolve::Rename => {
                        while address_book::get_account_id(chain_id, &alias)?.is_some() {
                            alias = inquire::Text::new("Rename contract alias")
                                .with_initial_value(contract_id)
                                .prompt()?;
                        }
                    }
                    DuplicateResolve::Skip => continue,
                    DuplicateResolve::SkipAll => {
                        duplicate_resolve_global = Some(duplicate_resolve);
                        continue;
                    }
                    DuplicateResolve::Override => {}
                    DuplicateResolve::OverrideAll => {
                        duplicate_resolve_global = Some(duplicate_resolve);
                    }
                }
            }
            address_book::insert_account_id(chain_id, &alias, address)?;
        }
        Ok(FetchAddressesOutput)
    }
}

fn input_duplicate_resolve(original: &str) -> color_eyre::eyre::Result<DuplicateResolve> {
    let variants = DuplicateResolveDiscriminants::iter().collect::<Vec<_>>();
    let selected = inquire::Select::new(
        "A duplicate has occurred, what do you prefer to do?",
        variants,
    )
    .with_help_message(original)
    .prompt()?;
    let selected = match selected {
        DuplicateResolveDiscriminants::Rename => DuplicateResolve::Rename,
        DuplicateResolveDiscriminants::Skip => DuplicateResolve::Skip,
        DuplicateResolveDiscriminants::Override => DuplicateResolve::Override,
        DuplicateResolveDiscriminants::SkipAll => DuplicateResolve::SkipAll,
        DuplicateResolveDiscriminants::OverrideAll => DuplicateResolve::OverrideAll,
    };
    Ok(selected)
}

pub fn read(filename: &String) -> color_eyre::Result<Value> {
    let file =
        File::open(filename).unwrap_or_else(|_| panic!("File should be present at {}", filename));
    let json: Value = serde_json::from_reader(file)?;
    Ok(json)
}