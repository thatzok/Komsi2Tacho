# Komsi2Tacho

[Deutsche Version](README.de.md)

This project allows controlling a physical **VDO MTCO 1323** (speedometer / display unit) (known from public buses) directly from a simulation.

Communication is based on the [KOMSI protocol](https://github.com/thatzok/Komsi-Protocol) via USB/Serial and outputting the data via the **CAN bus**.

To the VDO TCO 1323 (speedometer display), we act on the CAN bus as if we were a VDO MTCO 1324 (tachograph).

## Project Description

`Komsi2Tacho` acts as a translator between simulation software on the PC and real hardware. It reads telemetry data (especially speed), transforms it into the corresponding CAN frames that can be processed by the MTCO tachograph, and takes care of sending the CAN data regularly at the required time intervals.

## Features

- Direct translation of KOMSI data into CAN bus messages.
- Optimized for low latency between simulation and needle movement.
- Specifically adapted to the specific CAN parameters of the VDO MTCO 1323.

## Prerequisites

- An ESP32C6 microcontroller and a CAN bus transceiver.
- Since the MTCO 1323 operates at 24V, a 24V power supply is required.
- A simulation or plugin that outputs the KOMSI protocol via the serial interface (e.g., **TheBus2Komsi** or **Omsi2Komsi**). For testing, you can also connect to the USB port using a terminal program and enter commands manually.

## Installation / Flashing

1. Install the Rust development environment "cargo".
2. Install the Rust toolchain for the **ESP32C6** (sounds easier than it is, sorry).
3. Clone the repository into your workspace.
4. Connect one or both USB ports (see PINOUT.md) of the **ESP32C6** to the PC.
5. `cargo run` - Flash the firmware to your controller and start it.
6. Connect the CAN bus to the speedometer (CAN-High / CAN-Low) and ensure correct termination (60 ohms).

## If Flashing Doesn't Start: Bootloader/Download Mode

Some boards go into flash mode automatically, some don't. In that case:

1. Hold down BOOT.
2. Briefly press EN/RESET.
3. Release BOOT.
4. Then try flashing again.

## Usage

1. Connect the CAN bus to the speedometer (CAN-High / CAN-Low) and ensure correct termination (60 ohms total).
2. Connect one or both USB ports (see PINOUT.md) of the **ESP32C6** to the PC.

## Disclaimer

This project is for simulation purposes in the private sector only. Use in real road traffic for manipulating control devices is strictly prohibited.

This project comes without any warranty or guarantee. You use it entirely at your own risk. I am not responsible if you cause a short circuit or damage your PC, ESP, power supply, speedometer, or anything else.

Have Fun!
