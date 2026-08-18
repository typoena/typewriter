# HID usage IDs and boot-report bytes

`keymap/src/lib.rs` matches on raw USB HID Keyboard/Keypad usage IDs, and
`firmware/src/drivers/keyboard_usb.rs` fills raw control-transfer fields. This
is what those numbers are.

## Boot report

Eight bytes: `[modifiers, reserved, key1..key6]`. A `0` slot means no key. A
report shorter than 3 bytes is ignored; extra bytes past the six slots are
processed, never indexed out of range.

## Modifier bits

| Bit | Key |
|---|---|
| `0x01` | Left Ctrl |
| `0x02` | Left Shift |
| `0x08` | Left GUI (Cmd) |
| `0x10` | Right Ctrl |
| `0x20` | Right Shift |
| `0x80` | Right GUI (Cmd) |

The decoder tests both sides at once: Shift is `mods & 0x22`, Cmd is
`mods & 0x88`, Ctrl is `mods & 0x11`.

Caps Lock (usage `0x39`) is a key slot, not a modifier bit, so it is
edge-tracked by hand: held it acts as Ctrl, tapped alone it emits Escape.

## Usage IDs the chord table matches

| Usage | Key | Chord |
|---|---|---|
| `0x04` | A | reserved — Ctrl+A and Cmd+A insert nothing |
| `0x06` | C | Ctrl+C — continue the focus break |
| `0x07` | D | Ctrl+D — half page down |
| `0x11` | N | Ctrl+N — move down (vim CTRL-N); Cmd+N reserved for `:enew` |
| `0x13` | P | Ctrl+P — move up (vim CTRL-P); Cmd+P — file palette; Cmd+Shift+P — command palette |
| `0x14` | Q | Ctrl+Q — quit the focus session |
| `0x15` | R | Ctrl+R — redo |
| `0x16` | S | Cmd+S — save |
| `0x18` | U | Ctrl+U — half page up |
| `0x1A` | W | Ctrl+W — delete word, readline-style |
| `0x2A` | Backspace | Cmd — delete line; Ctrl — delete word; bare — one char |
| `0x2B` | Tab | Ctrl+Tab — MRU note switch; releasing Ctrl commits the walk |
| `0x39` | Caps Lock | held = Ctrl, tapped = Escape |

Letter usage IDs run `0x04` = A through `0x1D` = Z, so any ID in that range is
`0x04 + (letter - 'a')`.

## Control transfer fields

`keyboard_usb.rs` sets these on the raw esp-idf transfer struct:

- `bEndpointAddress = 0` — the control endpoint, EP0.
- `num_bytes = 8` for a setup packet with no data stage.
- `num_bytes = BOOT_REPORT_LEN` for the interrupt-in report, which must be a
  multiple of `wMaxPacketSize` (8).
