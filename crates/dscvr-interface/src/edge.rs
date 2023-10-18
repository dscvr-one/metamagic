use crate::{Interface, Principal};
use ic_cdk::api::call::RejectionCode;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use time::OffsetDateTime;

pub struct Edge {
    caller: Principal,
    time: Option<u64>,
}

impl Edge {
    pub fn new_with_caller_and_time(caller: Principal, time: Option<u64>) -> Self {
        Self { caller, time }
    }
}

impl Default for Edge {
    fn default() -> Self {
        Self {
            caller: Principal::from_text("aaaaa-aa").unwrap(),
            time: None,
        }
    }
}

impl Interface for Edge {
    fn time(&self) -> u64 {
        self.time
            .unwrap_or_else(|| OffsetDateTime::now_utc().unix_timestamp_nanos() as u64)
    }

    fn caller(&self) -> Principal {
        self.caller
    }

    fn canister_balance(&self) -> u64 {
        500_u64
    }

    fn call_canister(
        &self,
        _canister_id: Principal,
        _method: String,
        _args: Vec<u8>,
        _payment: u64,
    ) -> Result<Vec<u8>, (RejectionCode, String)> {
        unimplemented!();
    }

    fn id(&self) -> Principal {
        self.caller()
    }
    fn get_memory_usage(&self) -> u64 {
        // FIXME
        0
    }

    fn performance_counter(&self, _counter_type: u32) -> u64 {
        0
    }

    fn instruction_counter(&self) -> u64 {
        0
    }

    fn stable64_size(&self) -> u64 {
        0
    }
}

struct TestFuture;

impl Future for TestFuture {
    type Output = Result<Vec<u8>, (RejectionCode, String)>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = Ok(vec![]);
        Poll::Ready(result)
    }
}
