# Enclosure — integrated-keyboard variant (concept)

The one-piece [**Typoena**](../../README.md): the same reclined e-paper deck as
the [bring-your-own-keyboard body](README.md), with a keyboard tray grafted onto
the front — AlphaSmart / Freewrite silhouette, typewriter proportions. Two
sizes, both built on off-the-shelf QMK PCBs so "custom" means switches,
caps and keymap, not PCB design:

| `kb`   | keyboard                          | body (W × D)     | print               |
| ------ | --------------------------------- | ---------------- | ------------------- |
| `"60"` | GH60 footprint (DZ60, YMDK…)      | 291.3 × 202.4 mm | monolithic, 300 bed |
| `"40"` | Planck footprint, ortho, MIT (2u) | 232.1 × 180.7 mm | monolithic          |

> **Status: v0 concept, not yet printed.** The wedge half is inherited verbatim
> from the proven base model. The `"40"` bay is dimensioned from OLKB's published
> CAD part (see below); the `"60"` bay's tray-mount numbers are still best-guesses
> marked `<< VERIFY >>` until checked against a real DZ60 drawing.

**The Planck is discontinued** — Drop's store closed 31 March 2026 and olkb.com no
longer lists one. Buy a clone that advertises Planck-case compatibility (JJ40,
Niu Mini): those keep the footprint the `"40"` numbers come from, and both plates
cut MIT as well as grid. A clone that doesn't claim compatibility (Pabile P48,
Boardsource Equals 48) may fit the bay but will not land on these posts — caliper
it and re-derive `kb_holes` before printing.

| 60% (GH60)                                                                                         | 40% (Planck)                                                              |
| -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| ![60% variant assembled — keyboard tray in front, reclined deck behind](renders/kb60-assembled.png) | ![40% Planck variant — same silhouette, 59 mm narrower](renders/kb40-assembled.png) |

## How it composes

[`typoena-case-kb.scad`](typoena-case-kb.scad) `include`s the base model and
overrides only `W`: the whole wedge — screen clamp, PCB 1 back-left, PCB 2
back-right, front battery, baseplate, ports — re-derives at the new width and is
translated back by the bay depth. The bay is additive:

- **Tray mount.** The keyboard PCB screws to integral posts on the bay floor
  (standard GH60 tray positions for the 60%; for the 40, the Planck's five M2
  holes read off [`olkb_parts`](https://github.com/olkb/olkb_parts/blob/master/planck/planck-pcb-module.kicad_mod) —
  the same part gives the 226.5 × 73.5 outline and puts the USB notch rear-left,
  37.25 mm in, not centred).
  Plate floats on the switches, walls locate it with 0.75 mm wiggle. The rim
  (`Hk = 24` at the front) hides the plate edge, caps sit proud; toward the
  deck it rises to meet the wedge's front-top edge flush, so bay and deck
  join without a seam.

  **`Hk` and `kb_post_h` track the base model's `Hf`.** `Hf` isn't only the
  wedge's front height here — it *is* the bay/cavity shared wall, the wall behind
  the top key row. Raise it and that wall climbs while the keyboard stack stays
  put: at the base model's +2 with these left alone, the caps drop from **3.1 mm
  proud of the wall to 1.1 mm** and the back row sits in a well. Both carry the
  same +2 (`Hk` 22→24, `kb_post_h` 8→10), which restores 3.1 mm over the wall and
  5.1 mm over the front rim exactly, and the plate stays hidden. `kb_cable_cut`
  derives from `kb_pcb_z`, so the USB passthrough follows the taller posts on its
  own — no third number to chase.
- **Internal USB.** The keyboard stays a stock QMK device. Its cable leaves
  through a slot in the shared wall into the wedge cavity. PCB 2's keyboard
  USB-C faces **out** the back wall, so it can't take an internal plug — PCB 2
  instead grows a **4-pin header (VBUS/D−/D+/GND) wired in parallel** with that
  connector, and the model fills the old port cutout: the back wall shows only
  charge, µSD and the power switch. (Direct matrix-on-GPIO — dropping the MT3608
  and USB host for wake-on-key — is a possible v2; it costs 19 pins plus matrix
  scan and a keymap in firmware.)
- **Floor + feet.** The bay has an integral 3 mm floor and its own front feet;
  the wedge keeps its drop-in baseplate for service access.

![60% body — tray posts, USB passthrough slot, screen recess behind](renders/kb60-body.png)
![Section — tray stack in front, battery and boards in the wedge](renders/kb60-section.png)
![Back wall — just charge, µSD and the power switch now](renders/kb60-back.png)

## Render / export

From `hardware/`:

```sh
just render-kb   # regenerate case/renders/kb*.png (both sizes)
just stl-kb      # kb60/kb40 body + baseplate STLs (bracket comes from `just stl`)
```

`show` accepts `kb_assembled` · `kb_body` · `kb_baseplate` · `kb_section` ·
`kb_print`. **`kb` and `W` move as a pair** (`"60"` → 291.30, `"40"` → 232.05);
an include-override must be a literal, so `W` can't follow `kb` by itself — the
model asserts if they drift.

## Verify before printing

- [ ] **Tray post positions** against the actual PCB drawing (`kb_holes`) — the
      GH60 set is the commonly-cited standard; the 40 set is OLKB's own CAD, so
      it only holds for a Planck-compatible clone.
- [ ] **USB position + slot** (`kb_usb_x`, `kb_post_h`): the plug head must pass
      the shared-wall slot and clear PCB 2's cavity side. The slot is rear-left
      now, so re-check it against PCB 2's position rather than assuming centre.
- [ ] **MIT stabiliser clearance** — a 2u space wants a PCB-mount stab hanging
      below the PCB; confirm `kb_post_h = 10` still leaves room over the bay floor.
      It now sits the PCB 13 mm up off a 3 mm floor, so there is 10 mm under the
      board — the +2 that came with the body's height helps here.
- [ ] **PCB 2 header** for the internal keyboard cable (4-pin, parallel to the
      keyboard USB-C) — decide JST-PH vs soldered pigtail.
- [ ] Everything on the base model's own list (battery dims, active-area offset).
