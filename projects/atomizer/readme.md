# Atomizer

Controls an ultrasonic mist maker board from an ESP32.

The board has a separate power input and a push-button trigger.
Each press cycles the board through modes: **Off → On → Timed → Off**.
The board doesn't report which mode it is in, so the firmware works it out:

- **Turning on:** power the board (it always boots in Off), wait for it to settle, then press the trigger 1 time for On or 2 times for Timed.
- **Turning off:** cut the power. This is instant and always leaves the board in a known state.
- **Water low:** cut the power and latch a fault. Running the disc dry destroys it.
  The atomizer stays off until the water is refilled **and** the mode button is pressed to acknowledge.

All of this logic lives in [`crates/atomizer-logic`](../../crates/atomizer-logic/src/lib.rs) and is unit-tested on the host (`mise run test`).

## Controls

- **Mode button:** steps the target mode Off → On → Timed → Off. While a fault is latched, it acknowledges the fault instead.
- **Serial log:** shows `Target:`, `Board:`, `Water:` and fault messages.

## Wiring

| ESP32 | Goes to | Notes |
|---|---|---|
| GPIO26 | MOSFET gate (through 100Ω) | Logic-level N-MOSFET (e.g. AO3400) switches the board's **ground**. Add a 10k resistor from gate to GND so the board stays off while the ESP32 boots. |
| GPIO27 | Optocoupler LED anode (through 330Ω) | PC817 or similar. Cathode to GND. The optocoupler's transistor goes across the board's trigger pads. |
| GPIO19 | Mode button, other side to GND | Internal pull-up. |
| GPIO18 | Water float switch, other side to GND | Internal pull-up. Closed = water present. If your float switch works the other way, flip the reading in `main.rs`. |
| GND | Atomizer supply GND | Common ground with the 5V atomizer supply. |

### Before wiring

1. **Measure the trigger pads.** With the board powered, measure the voltage across the button pads and note which pad is +.
   The optocoupler's collector goes to the + pad and its emitter to the other.
   If the transistor is in backwards, presses won't register.
2. **Power the atomizer separately.** It draws about 300–500 mA at 5V.
   Give it its own 5V supply and never power it from the ESP32's 3.3V pin.
3. **Check the timing.** If presses get missed, increase `press_frames` / `gap_frames` in `main.rs`.
   If the first press after power-on is missed, increase `settle_frames`.
4. **Find out what "Timed" does.** Run the board in mode 2 with a stopwatch.
   Usually it is either an on/off interval or an auto-off timer.
5. **Test dry first.** Check the power and trigger switching with the disc out of the water, using LEDs or a multimeter, before adding water.

Keep the electronics above the water line, and use drip loops on the cables.

## Simulation

`diagram.json` uses stand-in parts: a blue LED for board power, a yellow LED for the trigger, a green mode button and a slide switch for the water float.
Slide the switch right to simulate water running low.

- **Interactive (click the button, flip the switch):** open the project in VS Code, `mise run build atomizer`, open `diagram.json`, then run **Wokwi: Start Simulator**.
- **Headless serial output:** `mise run sim atomizer`
- **Scenario test (mode cycle):** `mise run test-sim atomizer`

To check the MOSFET and optocoupler circuit itself, sketch it in [Falstad CircuitJS](https://www.falstad.com/circuit/).
Wokwi simulates logic levels, not currents.

## Next steps

- Humidity sensor (DHT22 or SHT31) and a humidistat: on below one level, off above a higher one.
  `Controller::request` already accepts a target mode from any source.
- MQTT control, as in the stoplight project.
- A custom Wokwi chip that behaves like the real board (counts trigger presses, resets when unpowered), replacing the stand-in LEDs.
