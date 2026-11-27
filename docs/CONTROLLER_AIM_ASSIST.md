# Controller Support, 3-Way Prompts & Console Aim Assist

Reference for Project OPFOR's pure-Rust controller pipeline in `opfor-host`:

## 1. Architecture
- **Plug-and-Play XInput + Native Sony HID (`opfor-host/src/controller.rs`)**:
  - Detects connected Sony DualShock 3 / DualShock 4 / DualSense / DualSense Edge controllers (`VID_054C`) via Win32 Raw Input (`WM_INPUT`) and translates HID reports into live `XInputState` without requiring DS4Windows or Steam Input.
  - Supports `controller_prompts = auto | xbox | ps3` in `config/opfor.ini`.
- **3-Way Dynamic Button Prompts (`opfor-host/src/prompts.rs`)**:
  - Automatically switches between **Keyboard/Mouse**, **Xbox 360 (`Xenon`)**, and **PlayStation (`Ps3`)** prompts as input arrives.
- **Console `HaloAimerData` Aim Assist (`opfor-host/src/padaimer.rs`)**:
  - PC BFBC2 ships `DefaultHaloAimer` with mouse-oriented speeds (`50 / 105`), polynomial `0.3x + 0.4x^4 + 0.3x^8`, and `Suck*` aim assist disabled.
  - Console BFBC2 uses `0.1x + 0.4x^2`, speeds `3.5 / 2.5`, 1.5x acceleration above 90% deflection, and full `Suck*` aim assist.
  - When the user generates `config/opfor_console_aimer.ini` from their own Xbox 360 disc (never shipped) using:
    ```powershell
    opfor-cli extract-console-aimer --pc engine/Dist/win32/levels/mp_common/level-00.fbrb --xbox <xbox_mp_common>/level-00.fbrb
    ```
    `padaimer.rs` scans committed private heap pages for live `HaloAimerData` instances and swaps between console tuning (while a gamepad is active) and exact saved PC values (when KBM is active).
- **Live APT Menu Tree Reader (`opfor-host/src/probes.rs`)**:
  - Reads the live `AptCIH` display tree (`[[[[0x01BE9C5C]+0x94]+0x20]+0x60]` `"menuMC"`) for controller-aware menu inspection.
