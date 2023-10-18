// TODO: use generic system interface

// Counts the number of instructions for the liftetime of this object
#[cfg(target_arch = "wasm32")]
mod internal {
    pub struct ScopedInstructionCounter<'a> {
        name: &'a str,
        start: u64,
        system: &'a dyn dscvr_interface::Interface,
    }

    impl<'a> ScopedInstructionCounter<'a> {
        pub fn new(name: &'a str, system: &'a dyn dscvr_interface::Interface) -> Self {
            Self {
                name,
                start: system.instruction_counter(),
                system,
            }
        }
    }

    impl<'a> Drop for ScopedInstructionCounter<'a> {
        fn drop(&mut self) {
            let end = self.system.instruction_counter();
            tracing::info!("{} {}", self.name, end - self.start);
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
mod internal {
    pub struct ScopedInstructionCounter;

    impl ScopedInstructionCounter {
        #[inline]
        pub fn new(_name: &str, _system: &dyn dscvr_interface::Interface) -> Self {
            Self
        }
    }
}

pub use internal::ScopedInstructionCounter;
