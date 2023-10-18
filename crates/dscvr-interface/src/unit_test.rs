use crate::{Interface, Principal};
use ic_cdk::api::call::RejectionCode;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use time::OffsetDateTime;

pub const SYSTEM: &dyn Interface = &UnitTest;

#[derive(Default)]
pub struct UnitTest;

impl Interface for UnitTest {
    fn time(&self) -> u64 {
        OffsetDateTime::now_utc().unix_timestamp_nanos() as u64
    }

    fn caller(&self) -> Principal {
        Principal::from_text("aaaaa-aa").unwrap()
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
