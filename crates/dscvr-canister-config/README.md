# Dscvr Canister Configuration Library

## About
This library provides a simple way to read and write both `dscvr.json` and `dfx.json` configuration files. The library is designed to be easy to use and to be extensible.

## Modules
- schema    
  - dfx: Implementation of the `dfx.json` configuration file.  Allows reading with and interacting with the `dfx` cli.
  - dscvr: Custom implementation file supporting multi-canister.  This file organizes information in a more hierarchical manner than dfx.json.

## Usage

- Import the lib canister.
- In most instances, you'll want to use the custom `dscvr.json` configuration
```rust
    use dscvr_canister_config::schema::dscvr;

    fn main() {
        let cfg: DSCVRConfig = DSCVRConfig::try_new("some_network_name").unwrap();
    }
```

This library still converts our new custom `dscvr.json` to and from `dfx.json` to be able to be used wit the `dfx` cli. This was done to eliminate the need
for implementing the lower level `dfx` commands ourselves.  Helper methods are written and documented that follow the proper flow between files when allocating and provisioning new canisters
for use in a multi-canister environment.  In most instances, using these methods as opposed to manipulating the configuration object directly in memory is recommended.