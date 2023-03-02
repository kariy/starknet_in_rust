use crate::business_logic::execution::{
    error::ExecutionError,
    objects::{CallInfo, Event, L2toL1MessageInfo, TransactionExecutionInfo},
};

pub(crate) enum ExecutionInfo {
    Transaction(Box<TransactionExecutionInfo>),
    Call(Box<CallInfo>),
}

impl ExecutionInfo {
    pub fn get_sorted_l2_to_l1_messages(&self) -> Result<Vec<L2toL1MessageInfo>, ExecutionError> {
        match self {
            ExecutionInfo::Transaction(tx) => tx.get_sorted_l2_to_l1_messages(),
            ExecutionInfo::Call(call) => call.get_sorted_l2_to_l1_messages(),
        }
    }

    pub fn get_sorted_events(&self) -> Result<Vec<Event>, ExecutionError> {
        match self {
            ExecutionInfo::Transaction(tx) => tx.get_sorted_events(),
            ExecutionInfo::Call(call) => call.get_sorted_events(),
        }
    }
}