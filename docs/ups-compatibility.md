# UPS compatibility

`upsbeep` needs a UPS that beeps **periodically while running on battery**, with a steady tone the microphone can pick up. Beep patterns differ between models, so please report what you find.

## Reported UPSs

| Model | Tone | Pattern on battery | Result | Reported by |
|---|---|---|---|---|
| _none yet, be the first!_ | | | | |

One data point from the maintainer's UPS (model not yet recorded): a tone near 1820 Hz; after about 60 s on battery it gives two quick beeps, then one beep every 60 s. Detection and a real shutdown were verified end to end.

## How to report

1. Put the microphone next to the UPS and open `upsbeep-ui` (Calibrate tab), or run `upsbeepd --analyze 130`.
2. Make the UPS run on battery (unplug it from the wall) and record for long enough to catch several beeps.
3. Open a [UPS compatibility report](https://github.com/guptaamannn/ups-beep-monitor/issues/new?template=ups_report.yml) with the model, the detected tone, the beep pattern (including any change as the battery drains), and whether detection worked.

## Things that make a UPS hard to detect

- **Very quiet or low-pitched buzzers.** Move the microphone closer or raise its input gain.
- **Beeps that sweep across frequencies.** `upsbeep` looks for one steady tone.
- **No periodic beep.** Some UPSs only beep at low battery, which leaves too little time to shut down safely.
