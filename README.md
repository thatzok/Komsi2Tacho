# Komsi2Tacho

Dieses Projekt ermöglicht die Ansteuerung eines physischen **VDO MTCO 1323** Tachographen (bekannt aus Linienbussen)
direkt aus einer Simulation heraus. Die Kommunikation basiert auf dem **KOMSI-Protokoll** via USB/Serial und der Ausgabe
der Daten über den **CAN-Bus**.

## Projektbeschreibung

`Komsi2Tacho` fungiert als Übersetzer zwischen Simulations-Software am PC und realer Hardware. Es liest
Telemetriedaten (insbesondere Geschwindigkeit) ein, transformiert diese in die entsprechenden CAN-Frames, die vom
MTCO-Tachographen verarbeitet werden können und kümmert sich darum die CAN-Daten regelmäßig in den notwendigen
Zeitintervallen zu senden.

## Features

- **Protokoll-Translation:** Direkte Umsetzung von KOMSI-Daten in CAN-Bus Nachrichten.
- **Echtzeit-Verarbeitung:** Optimiert für geringe Latenz zwischen Simulation und Zeigerausschlag.
- **MTCO-Kompatibilität:** Speziell angepasst an die spezifischen CAN-Parameter des VDO 1323.

## Voraussetzungen

- **Hardware:** Ein Mikrocontroller mit CAN-Controller und Transceiver.
- **Spannungsversorgung:** Da der MTCO 1323 meist mit 24V betrieben wird, ist eine entsprechende externe Stromquelle
  erforderlich.
- **Software:** Eine Simulation oder ein Plugin, welches das KOMSI-Protokoll über die serielle Schnittstelle ausgibt (
  z.B. **TheBus2Komsi** oder **Omsi2Komsi**).

## Installation & Nutzung

1. Installiere den Rust Toolchain für das **ESP32C6**
2. Klone das Repository in deinen Workspace.
3. Verbinde einen oder beide USB-Anschlüsse (siehe PINOUT.md) mit dem PC
4. Konfiguriere die projektspezifischen Parameter (z.B. CAN-Bitrate).
5. `cargo run` - Flashe die Firmware auf deinen Controller.
6. Verbinde den CAN-Bus mit dem Tacho (CAN-High / CAN-Low) und achte auf die korrekte Terminierung (60 Ohm).

## Falls Flashen nicht startet: Bootloader/Download‑Mode

Manche Boards gehen automatisch in den Flash‑Mode, manche nicht. Dann:

BOOT gedrückt halten
EN/RESET kurz drücken
BOOT loslassen
Danach nochmal flashen.

## Disclaimer

Dieses Projekt dient ausschließlich Simulationszwecken im privaten Bereich. Eine Verwendung im realen Straßenverkehr zur
Manipulation von Kontrollgeräten ist strikt untersagt.

