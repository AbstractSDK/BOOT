use crate::daemon::queriers::CosmWasm;
use crate::environment::TxResponse;
use crate::error::CwOrchError;
use crate::prelude::*;

use super::sync::core::Daemon;

/// This trait contains helper methods for the upload of a contract
pub trait UploadHelpers: CwOrcUpload<Daemon> {
    /// Only upload the contract if it is not uploaded yet (checksum does not match)
    fn upload_if_needed(&self) -> Result<Option<TxResponse<Daemon>>, CwOrchError> {
        if self.latest_is_uploaded()? {
            Ok(None)
        } else {
            Some(self.upload()).transpose()
        }
    }

    /// Returns boolean for whether the checksum of the WASM file matches the checksum of the previously uploaded code
    fn latest_is_uploaded(&self) -> Result<bool, CwOrchError> {
        let Some(latest_uploaded_code_id) = self.code_id().ok() else {
            return Ok(false);
        };

        let chain = self.get_chain();
        let on_chain_hash = chain.rt_handle.block_on(
            chain
                .query_client::<CosmWasm>()
                .code_id_hash(latest_uploaded_code_id),
        )?;
        let local_hash = self.wasm().checksum(&self.id())?;

        Ok(local_hash == on_chain_hash)
    }

    /// Returns boolean for whether the contract is running the latest uploaded code for it
    fn is_running_latest(&self) -> Result<bool, CwOrchError> {
        let Some(latest_uploaded_code_id) = self.code_id().ok() else {
            return Ok(false);
        };
        let chain = self.get_chain();
        let info = chain.rt_handle.block_on(
            chain
                .query_client::<CosmWasm>()
                .contract_info(self.address()?),
        )?;
        Ok(latest_uploaded_code_id == info.code_id)
    }
}

impl<T> UploadHelpers for T where T: CwOrcUpload<Daemon> {}

/// This trait contains helper a method related with the migration of a contract
pub trait MigrateHelpers: CwOrcMigrate<Daemon> + UploadHelpers {
    /// Only migrate the contract if it is not on the latest code-id yet
    fn migrate_if_needed(
        &self,
        migrate_msg: &Self::MigrateMsg,
    ) -> Result<Option<TxResponse<Daemon>>, CwOrchError> {
        if self.is_running_latest()? {
            log::info!("{} is already running the latest code", self.id());
            Ok(None)
        } else {
            Some(self.migrate(migrate_msg, self.code_id()?)).transpose()
        }
    }
}

impl<T> MigrateHelpers for T where T: CwOrcMigrate<Daemon> + CwOrcUpload<Daemon> {}
