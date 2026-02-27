# Pinout & Hardware Setup

### [Deutsche Version](PINOUT.de.md)

This overview describes the physical connection for the **Komsi2Tacho** project.

## 1. ESP32-C6 to CAN-Transceiver (SN65HVD230)

| Signal  | ESP32-C6 Pin       | Transceiver Pin | Note                  |
|:--------|:-------------------|:----------------|:----------------------|
| **TX**  | `GPIO 6` (TWAI_TX) | `CTX`           | Data from ESP to bus  |
| **RX**  | `GPIO 7` (TWAI_RX) | `CRX`           | Data from bus to ESP  |
| **VCC** | `3V3`              | `3V3`           | 3.3V power supply     |
| **GND** | `GND`              | `GND`           | Common ground         |

## 2. Connection VDO MTCO 1323 (Speedometer)

The speedometer uses standardized ISO connectors (A, B, C, D) on the back; only connector A is used.

*Attention: The speedometer requires **24V** DC!*

| Pin | Signal       | Connection               |
|:----|:-------------|:-------------------------|
| A1  | Constant +   | +24V power supply        |
| A2  | Illumination | +24V power supply        |
| A3  | Ignition     | +24V power supply        |
| A4  | CAN-High     | Transceiver `CAN-H`      |
| A5  | -            | -                        |
| A6  | GND          | Power supply ground      |
| A7  | CAN-GND      | - not connected -        |
| A8  | CAN-Low      | Transceiver `CAN-L`      |

A1/A2/A3 are connected to 24V of the power supply and A6 is connected to ground/minus of the power supply.

## 3. USB Connection ESP32-C6 to PC

The `nanoESP32‑C6 V1.0 development board` has two USB ports, one for flashing, status information, and debugging, and the other for KOMSI communication with the PC.

| USB    | Label on board | Function                                       | Note                                        |
|:-------|:---------------|:-----------------------------------------------|:--------------------------------------------|
| Native | `ESP32C6`      | Integrated into ESP32C6, JTAG debugging, flashing | Only for flashing and debugging, KOMSI commands |
| CH343  | `CH343`        | USB-to-Serial connection (UART)                 | KOMSI commands from PC                      |

If only one USB port is connected after flashing, the Native port can be used for KOMSI communication.
When in doubt, try it out. If you connect a terminal program to the USB, a Komsi message should be output on the KOMSI port.


## Important Notes

- **Termination:** Approximately 60 ohms should be measured between CAN-H and CAN-L. Inexpensive transceivers already have an internal resistor of 120 ohms. If a resistance of 120 ohms is measured between CAN-H and CAN-L, an additional 120 ohm resistor should be connected between the two lines (parallel connection). After that, 60 ohms should be measured between CAN-H and CAN-L.
- **Common Ground:** Connect the `GND` of the ESP32 to the `CAN-GND` ground of the speedometer/power supply only if there are signal errors. Normally, this is not necessary in this use case.
- **ESP32 Power Supply:** The ESP32 receives its power supply via the USB connection from the PC.
- **ESP32 Type:** There are many different variants of the ESP32. This project uses the `ESP32-C6` and would need to be significantly adapted for other variants.
- Currently used board: `nanoESP32‑C6 V1.0 development board`
- Currently used CAN bus transceiver: `SN65HVD230`. It is important that it is the VP230 type; others may not reliably enough reach the necessary voltages of the CAN bus with the 3.3 volt supply voltage of the ESP32.
