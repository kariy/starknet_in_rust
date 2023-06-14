use cairo_vm::felt::Felt252;
use getset::Getters;
use num_traits::Zero;
use starknet_contract_class::EntryPointType;

use crate::{
    business_logic::{
        execution::{
            execution_entry_point::ExecutionEntryPoint, TransactionExecutionContext,
            TransactionExecutionInfo,
        },
        state::{
            state_api::{State, StateReader},
            ExecutionResourcesManager,
        },
        transaction::{error::TransactionError, fee::calculate_tx_fee},
    },
    core::transaction_hash::{calculate_transaction_hash_common, TransactionHashPrefix},
    definitions::{
        constants::L1_HANDLER_VERSION, general_config::TransactionContext,
        transaction_type::TransactionType,
    },
    utils::{calculate_tx_resources, Address},
};

#[allow(dead_code)]
#[derive(Debug, Getters)]
pub struct L1Handler {
    #[getset(get = "pub")]
    hash_value: Felt252,
    #[getset(get = "pub")]
    contract_address: Address,
    entry_point_selector: Felt252,
    calldata: Vec<Felt252>,
    nonce: Option<Felt252>,
    paid_fee_on_l1: Option<Felt252>,
}

impl L1Handler {
    pub fn new(
        contract_address: Address,
        entry_point_selector: Felt252,
        calldata: Vec<Felt252>,
        nonce: Felt252,
        chain_id: Felt252,
        paid_fee_on_l1: Option<Felt252>,
    ) -> Result<L1Handler, TransactionError> {
        let hash_value = calculate_transaction_hash_common(
            TransactionHashPrefix::L1Handler,
            L1_HANDLER_VERSION.into(),
            &contract_address,
            entry_point_selector.clone(),
            &calldata,
            0,
            chain_id,
            &[nonce.clone()],
        )?;

        Ok(L1Handler {
            hash_value,
            contract_address,
            entry_point_selector,
            calldata,
            nonce: Some(nonce),
            paid_fee_on_l1,
        })
    }

    /// Applies self to 'state' by executing the L1-handler entry point.
    pub fn execute<S>(
        &self,
        state: &mut S,
        general_config: &TransactionContext,
        remaining_gas: u128,
    ) -> Result<TransactionExecutionInfo, TransactionError>
    where
        S: State + StateReader,
    {
        let mut resources_manager = ExecutionResourcesManager::default();
        let entrypoint = ExecutionEntryPoint::new(
            self.contract_address.clone(),
            self.calldata.clone(),
            self.entry_point_selector.clone(),
            Address(0.into()),
            EntryPointType::L1Handler,
            None,
            None,
            remaining_gas,
        );

        let call_info = entrypoint.execute(
            state,
            general_config,
            &mut resources_manager,
            &self.get_execution_context(general_config.invoke_tx_max_n_steps)?,
            false,
        )?;

        let changes = state.count_actual_storage_changes();
        let actual_resources = calculate_tx_resources(
            resources_manager,
            &[Some(call_info.clone())],
            TransactionType::L1Handler,
            changes,
            Some(self.get_payload_size()),
        )?;

        // Enforce L1 fees.
        if general_config.enforce_l1_handler_fee {
            // Backward compatibility; Continue running the transaction even when
            // L1 handler fee is enforced, and paid_fee_on_l1 is None; If this is the case,
            // the transaction is an old transaction.
            if let Some(paid_fee) = self.paid_fee_on_l1.clone() {
                let required_fee = calculate_tx_fee(
                    &actual_resources,
                    general_config.starknet_os_config.gas_price,
                    general_config,
                )?;
                // For now, assert only that any amount of fee was paid.
                if paid_fee.is_zero() {
                    return Err(TransactionError::FeeError(format!(
                        "Insufficient fee was paid. Expected: {required_fee};\n got: {paid_fee}."
                    )));
                };
            }
        }

        Ok(
            TransactionExecutionInfo::create_concurrent_stage_execution_info(
                None,
                Some(call_info),
                actual_resources,
                Some(TransactionType::L1Handler),
            ),
        )
    }

    /// Returns the payload size of the corresponding L1-to-L2 message.
    pub fn get_payload_size(&self) -> usize {
        // The calldata includes the "from" field, which is not a part of the payload.
        // We thus subtract 1.
        self.calldata.len().saturating_sub(1)
    }

    /// Returns the execution context of the transaction.
    pub fn get_execution_context(
        &self,
        n_steps: u64,
    ) -> Result<TransactionExecutionContext, TransactionError> {
        Ok(TransactionExecutionContext::new(
            self.contract_address.clone(),
            self.hash_value.clone(),
            [].to_vec(),
            0,
            self.nonce.clone().ok_or(TransactionError::MissingNonce)?,
            n_steps,
            L1_HANDLER_VERSION.into(),
        ))
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
    };

    use cairo_vm::{
        felt::{felt_str, Felt252},
        vm::runners::cairo_runner::ExecutionResources,
    };
    use num_traits::{Num, Zero};
    use starknet_contract_class::EntryPointType;

    use crate::{
        business_logic::{
            execution::{CallInfo, TransactionExecutionInfo},
            state::{
                cached_state::CachedState, in_memory_state_reader::InMemoryStateReader,
                state_api::State,
            },
            transaction::l1_handler::L1Handler,
        },
        definitions::{general_config::TransactionContext, transaction_type::TransactionType},
        services::api::contract_classes::deprecated_contract_class::ContractClass,
        utils::Address,
    };

    #[test]
    fn test_execute_l1_handler() {
        let l1_handler = L1Handler::new(
            Address(0.into()),
            Felt252::from_str_radix(
                "c73f681176fc7b3f9693986fd7b14581e8d540519e27400e88b8713932be01",
                16,
            )
            .unwrap(),
            vec![
                Felt252::from_str_radix("8359E4B0152ed5A731162D3c7B0D8D56edB165A0", 16).unwrap(),
                1.into(),
                10.into(),
            ],
            0.into(),
            0.into(),
            Some(10000.into()),
        )
        .unwrap();

        // Instantiate CachedState
        let mut state_reader = InMemoryStateReader::default();
        // Set contract_class
        let class_hash = [1; 32];
        let contract_class =
            ContractClass::try_from(PathBuf::from("starknet_programs/l1l2.json")).unwrap();
        // Set contact_state
        let contract_address = Address(0.into());
        let nonce = Felt252::zero();

        state_reader
            .address_to_class_hash_mut()
            .insert(contract_address.clone(), class_hash);
        state_reader
            .address_to_nonce
            .insert(contract_address, nonce);

        let mut state = CachedState::new(state_reader.clone(), None, None);

        // Initialize state.contract_classes
        state.set_contract_classes(HashMap::new()).unwrap();

        state
            .set_contract_class(&class_hash, &contract_class)
            .unwrap();

        let mut config = TransactionContext::default();
        config.cairo_resource_fee_weights = HashMap::from([
            (String::from("l1_gas_usage"), 0.into()),
            (String::from("pedersen_builtin"), 16.into()),
            (String::from("range_check_builtin"), 70.into()),
        ]);
        config.starknet_os_config.gas_price = 1;

        let tx_exec = l1_handler.execute(&mut state, &config, 100000).unwrap();

        let expected_tx_exec = expected_tx_exec_info();
        assert_eq!(tx_exec, expected_tx_exec)
    }

    fn expected_tx_exec_info() -> TransactionExecutionInfo {
        TransactionExecutionInfo {
            validate_info: None,
            call_info: Some(CallInfo {
                caller_address: Address(0.into()),
                call_type: Some(crate::business_logic::execution::CallType::Call),
                contract_address: Address(0.into()),
                code_address: None,
                class_hash: Some([
                    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                    1, 1, 1, 1, 1, 1,
                ]),
                entry_point_selector: Some(felt_str!(
                    "352040181584456735608515580760888541466059565068553383579463728554843487745"
                )),
                entry_point_type: Some(EntryPointType::L1Handler),
                calldata: vec![
                    felt_str!("749882478819638189522059655282096373471980381600"),
                    1.into(),
                    10.into(),
                ],
                retdata: vec![],
                execution_resources: ExecutionResources {
                    n_steps: 141,
                    n_memory_holes: 20,
                    builtin_instance_counter: HashMap::from([
                        ("range_check_builtin".to_string(), 6),
                        ("pedersen_builtin".to_string(), 2),
                    ]),
                },
                events: vec![],
                l2_to_l1_messages: vec![],
                storage_read_values: vec![0.into()],
                accessed_storage_keys: HashSet::from([[
                    4, 40, 11, 247, 0, 35, 63, 18, 141, 159, 101, 81, 182, 2, 213, 216, 100, 110,
                    5, 5, 101, 122, 13, 252, 204, 72, 77, 8, 58, 226, 194, 24,
                ]]),
                internal_calls: vec![],
                gas_consumed: 0,
                failure_flag: false,
            }),
            fee_transfer_info: None,
            actual_fee: 0,
            actual_resources: HashMap::from([
                ("pedersen_builtin".to_string(), 13),
                ("range_check_builtin".to_string(), 23),
                ("l1_gas_usage".to_string(), 18471),
            ]),
            tx_type: Some(TransactionType::L1Handler),
        }
    }
}