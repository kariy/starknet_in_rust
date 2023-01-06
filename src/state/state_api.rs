use num_bigint::BigInt;

use crate::{core::errors::state_errors::StateError, services::api::contract_class::ContractClass};

use super::{state_api_objects::BlockInfo, state_chache::StorageEntry};

pub(crate) trait StateReader {
    /// Returns the contract class of the given class hash.
    fn get_contract_class(&mut self, class_hash: &[u8]) -> Result<&ContractClass, StateError>;
    /// Returns the class hash of the contract class at the given address.
    fn get_class_hash_at(&mut self, contract_address: &BigInt) -> Result<&Vec<u8>, StateError>;
    /// Returns the nonce of the given contract instance.
    fn get_nonce_at(&mut self, contract_address: &BigInt) -> Result<&BigInt, StateError>;
    /// Returns the storage value under the given key in the given contract instance.
    fn get_storage_at(&mut self, storage_entry: &StorageEntry) -> Result<&BigInt, StateError>;
}

pub(crate) trait State {
    fn get_block_info(&self) -> &BlockInfo;
    fn set_contract_class(&mut self, class_hash: &[u8], contract_class: &ContractClass);
    fn deploy_contract(
        &mut self,
        contract_address: BigInt,
        class_hash: Vec<u8>,
    ) -> Result<(), StateError>;
    fn increment_nonce(&mut self, contract_address: &BigInt) -> Result<(), StateError>;
    fn update_block_info(&mut self, block_info: BlockInfo);
    fn set_storage_at(&mut self, storage_entry: &StorageEntry, value: BigInt);
}