# JoyCon

An open-source cross-platform Rust library for interfacing with Nintendo Switch Joy-Con controllers (Joy-Con 1 and Joy-Con 2).

---

## Overview 🎮

**JoyCon** provides a foundation for developers and researchers interested in low-level interaction with Nintendo Switch Joy-Cons, supporting both Bluetooth Classic (Joy-Con 1) and Bluetooth Low Energy (Joy-Con 2).

* **Joy-Con 1** devices require manual Bluetooth pairing to appear as HID devices. Although full Joy-Con 1 support isn't the initial focus of the first commit, it is supported through the `JoyCon` struct in `joycon.rs`. This struct can be instantiated by passing the vendor ID, product ID, and serial number obtained from HID enumeration, enabling direct reading and writing of input reports. Implementing Joy-Con 1 functionality is straightforward with this approach.
* **Joy-Con 2** devices broadcast BLE advertisements when the sync button is held. JoyCon scans for these advertisements, allowing users to select and connect to a device and subscribe to the correct BLE characteristics. However, input notifications from Joy-Con 2 have not been successfully received yet despite subscribing, making this the current main focus for further development and experimentation.

---

## Current Functionality

* Scans for Joy-Con 1 devices via HID enumeration (requires manual pairing beforehand).
* Scans and detects Joy-Con 2 devices broadcasting BLE advertisements during sync mode.
* Enables connecting and subscribing to BLE characteristics on Joy-Con 2.
* Includes a `JoyCon` struct for Joy-Con 1 input report handling (not fully integrated yet).

---

## Intended Use and Design

The goal is to provide an open framework for anyone interested in exploring, decoding, or building on Joy-Con input data. JoyCon is primarily a developer tool and research project rather than a finished user-facing product.

* Each Joy-Con is treated as an individual device with its own inputs and sensors.
* The library does **not** combine inputs or provide shared controls out of the box.
* This approach allows developers to build custom logic for combining or processing multiple controllers however they see fit.
* The library supports connecting multiple Joy-Cons independently, which is useful for advanced use cases (e.g., motion-controlled games using several controllers per player, requiring separate gyro and accelerometer values).

---

## Quirks and Notes ⚠️

* For **Joy-Con 1 (HID)**, ensure the device is fully paired and connected before scanning. The Joy-Con light should be solid, otherwise the device won't appear in HID enumeration.
* For **Joy-Con 2 (BLE)**, devices show up when broadcasting advertisements such as when pressing buttons, but you must hold the sync button for pairing mode before connecting and subscribing.
* The `joycon.rs` file is included to assist with Joy-Con 1 input decoding and may be useful for Joy-Con 2 research, but it was not the focus of the initial implementation.
* Full Joy-Con 1 support through HID input reading is possible with the current codebase but requires additional integration work.

---

## Dependencies and Environment

* **Cross-platform**: Works on Windows, Linux, and macOS (tested on Windows 10/11 64-bit).
* Relies on `hidapi` for HID support and `btleplug` for BLE support (both cross-platform libraries).
* Requires Rust 1.70+ (2021 edition).

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
joycon = { path = "../joycon" }  # or from git
```

Or from crates.io (when published):

```toml
[dependencies]
joycon = "0.1.0"
```

---

## Usage

### Joy-Con 1 (HID)

```rust
use joycon::{JoyCon, scan_joycon1_devices};

// Scan for devices
let devices = scan_joycon1_devices()?;
for (serial, vid, pid) in devices {
    println!("Found Joy-Con: {} (VID: {:04X}, PID: {:04X})", serial, vid, pid);
    
    // Connect to a device
    let joycon = JoyCon::new(vid, pid, &serial)?;
    
    // Read input reports
    let mut buffer = [0u8; 64];
    let bytes_read = joycon.read_input_report(&mut buffer)?;
    println!("Read {} bytes", bytes_read);
}
```

### Joy-Con 2 (BLE)

```rust
use joycon::{JoyCon2, scan_joycon2_devices};

// Scan for devices (hold sync button on Joy-Con 2)
let devices = scan_joycon2_devices().await?;
for address in devices {
    println!("Found Joy-Con 2: {}", address);
    
    // Connect to a device
    let joycon2 = JoyCon2::connect(&address).await?;
    
    // Subscribe to input characteristics
    joycon2.subscribe_to_inputs().await?;
    
    // Note: Reading notifications is still a work in progress
}
```

---

## Contributing 🤝

* Experiment freely with the code and your own implementations.
* Document what works and what doesn't.
* This project focuses on building a foundation to decode and handle Joy-Con 2 inputs; Joy-Con 1 support is secondary but present.
* Raise issues or pull requests on GitHub to share your findings or improvements.
* Open to collaboration for completing and improving this library.

---

## License

MIT License — fully open source. Use it however you want. The goal is to build a complete and useful library for the community.

---

## Acknowledgements 🙏

Thanks to these researchers whose work inspired and aided this project:

* Guineapigmc: Described efforts writing C# software that successfully subscribes to the correct BLE characteristic to read Joy-Con 2 input data.
* coffincolors: Shared research using nRF Connect app to discover relevant BLE characteristics on Joy-Con 2.

Their insights sparked the motivation to build this Rust library.

---

## Summary

JoyCon is an experimental and open toolset for working with Joy-Con controllers on Windows at a low level, aimed at developers and hobbyists interested in custom input handling, research, and new ways of interfacing with these devices.

