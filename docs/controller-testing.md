# Test controller input

Run these commands from the Romlin repository, with its normal Rust/CMake build
dependencies installed. No ROM, RomM account, or saved settings are needed.

## Test on Windows, macOS, or Linux without a driver

```sh
cargo test -p romlin-app --bin romlin --locked sdl_virtual_controller_reaches_game_input -- --test-threads=1
```

This test attaches an SDL virtual gamepad and exercises Romlin's production
`Gamepads::poll` and `states` methods. It checks button presses, stick direction
and deadzone, trigger threshold, remapping, menu controls, player assignment,
runner input messages, release, and unplug while held. The device detaches when
the test ends, including on an assertion failure. It is also part of ordinary
`cargo test --workspace --locked` and the existing Windows/macOS/Linux CI matrix.

The virtual pad exists inside the test process. This proves the path from SDL
to Romlin's runner messages; it does not exercise a native OS device driver,
another running Romlin process, rendered UI behavior, or core consumption.

## Test a Windows virtual device with hidmaestro-rs

Use a Windows x64 test machine. The test connects an Xbox 360 wired profile
through the Windows driver into SDL, then observes Romlin's mapped input.
It checks enumeration, A, left stick, left trigger, neutral reset, and removal.
Successful bridge submissions alone do not pass the test.

The optional `controller-hidmaestro` feature pins the MIT-licensed
[hidmaestro-rs source](https://github.com/auron-labs/hidmaestro-rs/tree/b0bb3a827c7448bc721d87574b6c46f7622f4513).
Normal builds leave the feature disabled. Enabling it on macOS/Linux does not
provide virtual OS devices or a hidmaestro test there.

### Build the separate Windows bundle

Use Rust with the MSVC toolchain, .NET SDK 10, PowerShell 7, Visual Studio
2022+ C++ tools (x64 and ARM64), and Windows SDK/WDK 10.0.26100.0.
See the pinned [build prerequisites](https://github.com/auron-labs/hidmaestro-rs/blob/b0bb3a827c7448bc721d87574b6c46f7622f4513/docs/getting-started.md).
From a source parent directory in ordinary PowerShell:

```powershell
git clone https://github.com/auron-labs/hidmaestro-rs.git
Set-Location hidmaestro-rs
git checkout b0bb3a827c7448bc721d87574b6c46f7622f4513
git submodule update --init --recursive
pwsh -File scripts/package-windows.ps1 -Mode Stage
```

Keep the entire printed staged directory together, including runtime files,
`LICENSE`, `LICENSE-HIDMAESTRO`, and `UNSIGNED.txt`. A Cargo build does not build
the .NET bridge or driver payload. This integration neither redistributes that
bundle nor adds the upstream repository to the Romlin workspace manifest.

### Prepare the driver once

Follow the pinned upstream [first-controller setup](https://github.com/auron-labs/hidmaestro-rs/blob/b0bb3a827c7448bc721d87574b6c46f7622f4513/docs/mcp.md#create-your-first-controller)
using its MCP client configuration: check `status`, then use `install_driver`
only if the driver is absent. Finish that session before starting the test.
Driver installation requires Administrator privileges, adds a local test
certificate to machine trust stores, and sweeps existing HIDMaestro devices.
The bundle is unsigned. Perform this deliberate setup on a test machine;
Romlin's test never installs or reinstalls drivers.

### Run the device test

In an Administrator PowerShell under the **same Windows account** as the test,
change to the complete staged bundle directory and run:

```powershell
.\hidmaestro-bridge.exe --pipe romlin-controller-test
```

Wait for its listening message. Use a dedicated bridge session: it accepts one
client, and the test shuts it down. In an ordinary PowerShell in Romlin:

```powershell
$env:HIDMAESTRO_PIPE_NAME = 'romlin-controller-test'
cargo test -p romlin-app --bin romlin --locked --features controller-hidmaestro gamepads::controller_tests::hidmaestro_controller_reaches_sdl -- --ignored --exact --test-threads=1 --nocapture
```

The expected result is **one passed test**, not zero tests or an ignored test.
Keep other controllers connected throughout the run; do not connect additional
devices during enumeration. Restart the bridge for each run. Missing setup or
unobserved input fails the test instead of silently skipping it.

The test attempts reset and removal of its own controller even after an input
error, then shuts down its bridge session. If the process is killed or cleanup
fails, stop the dedicated bridge and verify the virtual device is gone before
continuing. Never use system-wide `remove_all_controllers` as routine test cleanup.

The Windows CI job compiles the optional test using `--all-features`.
The Windows device test is ignored by default and is not run in hosted CI.
A green standard CI run proves SDL injection coverage, not Windows driver setup
or hidmaestro compatibility on the target machine.

## Native macOS/Linux coverage

The pinned hidmaestro bridge and driver operate only on Windows x64. Its Rust
client building on Unix does not supply a Unix device backend.

For native end-to-end coverage today, connect a physical controller, run Romlin,
and check Settings controller identification and remapping, library navigation,
in-game buttons/sticks/triggers, player reassignment, and unplug while held.
Repeat with the controller families and transport modes being shipped.

A Linux OS-visible virtual backend would require `/dev/uinput` access plus a
gamepad descriptor recognized by SDL. macOS would need an IOHIDUserDevice
implementation with appropriate permissions and deployment validation. These
are separate device backends; neither is implemented by this integration.
SDL injection gives repeatable automated input coverage on both platforms now.
