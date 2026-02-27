# Pinbelegung & Hardware-Setup

### [English version](PINOUT.en.md)

Diese Übersicht beschreibt die physische Verbindung für das **Komsi2Tacho** Projekt.

## 1. ESP32-C6 zu CAN-Transceiver (SN65HVD230)

| Signal  | ESP32-C6 Pin       | Transceiver Pin | Notiz                 |
|:--------|:-------------------|:----------------|:----------------------|
| **TX**  | `GPIO 6` (TWAI_TX) | `CTX`           | Daten vom ESP zum Bus |
| **RX**  | `GPIO 7` (TWAI_RX) | `CRX`           | Daten vom Bus zum ESP |
| **VCC** | `3V3`              | `3V3`           | 3.3V Versorgung       |
| **GND** | `GND`              | `GND`           | Gemeinsame Masse      |

## 2. Anschluss VDO MTCO 1323 (Tacho)

Der Tacho nutzt auf der Rückseite standardisierte ISO-Stecker (A, B, C, D), es wird nur der Stecker A verwendet.

*Achtung: Der Tacho benötigt **24V** Gleichspannung!*

| Pin | Signal      | Verbindung              |
|:----|:------------|:------------------------|
| A1  | Dauerplus   | +24V Stromversorgung    |
| A2  | Beleuchtung | +24V Stromversorgung    |
| A3  | Zündung     | +24V Stromversorgung    |
| A4  | CAN-High    | Transceiver `CAN-H`     |
| A5  | -           | -                       |
| A6  | GND         | Masse Stromversorgung   |
| A7  | CAN-GND     | - nicht angeschlossen - |
| A8  | CAN-Low     | Transceiver `CAN-L`     |

A1/A2/A3 werden mit 24V der Stromversorgung verbunden und A6 mit Masse/Minus der Stromversorgung verbunden.

## 3. USB-Anschluss ESP32-C6 zum PC

Das `nanoESP32‑C6 V1.0 Entwicklungsboard` besitzt zwei USB-Anschlüsse, von denen einer für Flashen, Status-Infos und
Debugging und der andere für die KOMSI-Kommunikation mit dem PC verwendet wird.

| USB    | Beschriftung auf Board | Funktion                                       | Notiz                                       |
|:-------|:-----------------------|:-----------------------------------------------|:--------------------------------------------|
| Native | `ESP32C6`              | In ESP32C6 integriert, JTAG-Debugging, Flashen | Nur zum Flashen und Debuggen, KOMSI-Befehle |
| CH343  | `CH343`                | USB-to-Serial Anschluss (UART)                 | KOMSI-Befehle vom PC                        |

Wenn nach dem Flashen nur ein USB-Anschluss angeschlossen wird, kann der Native-Anschluss für die KOMSI-Kommunikation verwendet werden.
Im Zweifelsfall ausprobieren. Wenn man ein Terminalprogramm mit dem USB-Verbindet, sollte auf dem KOMSI-Port eine Komsi-Nachricht ausgegeben werden.


## Wichtige Hinweise

- **Terminierung:** Zwischen CAN-H und CAN-L müssen ca. 60 Ohm gemessen werden. Die preiswerten Transceiver haben
  bereits einen internen Widerstand von 120 Ohm. Wenn zwischen CAN-H und CAN-L ein Widerstand von 120 Ohm gemessen wird,
  sollte zwischen die beiden Leitungen ein zusätzlicher 120 Ohm Widerstand geschaltet werden (Parallelschaltung). Danach
  sollte zwischen CAN-H und CAN-L 60 Ohm gemessen werden.
- **Gemeinsame Masse:** Verbinde den `GND` des ESP32 nur mit der `CAN-GND` Masse des Tachos/Netzteils, wenn es
  Signalfehler gib. Normalweise ist das aber nicht notwendig bei diesem Anwendungsfall.
- **Stromversorgung ESP32:** Der ESP32 bezieht seine Stromversorgung über den USB-Anschluss vom PC.
- **ESP32-Typ:** Es gibt vom ESP32 viele verschiedene Varianten. Dieses Projekt verwendet den `ESP32-C6` und müsste für
  andere Varianten teils erheblich angepasste werden.
- Aktuell genutztes Board: `nanoESP32‑C6 V1.0 Entwicklungsboard`
- Aktuell genutzter CAN-Bus-Transceiver: `SN65HVD230`. Es ist wichtig, dass es der VP230-Typ ist, andere kommen mit der
  3,3 Volt Versorgungsspannung des ESP32 evtl. nicht zuverlässig genug auf die notwendigen Spannungen des CAN-BUS.
 
