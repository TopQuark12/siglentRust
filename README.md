# siglentRust
Control Siglent Devices in Rust

Early beta, only tested on SDS2000X HD and SDS2000X Plus.
SCPI implementation is hopelessly broken on early firmware versions for the devices, make sure you have the latest version of the device firmware when using this code.

## Build instructions 

Prerequisite: Nix build system

```
nix-shell
cargo run --release
```
