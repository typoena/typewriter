// ============================================================================
//  Typoena — enclosure variant "kb"  ·  integrated-keyboard body  ·  rev v0
// ----------------------------------------------------------------------------
//  The one-piece writer: same reclined e-paper deck as typoena-case.scad, with
//  a keyboard tray grafted onto the front (AlphaSmart / Freewrite silhouette).
//  The keyboard is an off-the-shelf QMK PCB in a standard tray mount:
//
//    kb = "60"  — GH60-footprint PCB (DZ60/YMDK…), 285 x 94.6
//    kb = "40"  — Planck-footprint ortho PCB, 226.5 x 73.5, MIT (47 keys, 2u
//                 space). The Planck itself is discontinued; buy a clone that
//                 advertises Planck-case compatibility (JJ40, Niu Mini).
//
//  It connects over USB *internally*: a short cable runs from the keyboard PCB
//  through a slot in the shared wall into the wedge cavity. PCB 2's keyboard
//  USB-C faces out the back wall, so it can't take an internal plug — instead
//  PCB 2 grows a 4-pin header (VBUS/D-/D+/GND) wired in parallel with that
//  connector, and this model FILLS the old keyboard port cutout: the back wall
//  shows only charge, µSD and the power switch.
//
//  This file `include`s typoena-case.scad and overrides W (bay width wins over
//  screen width) — the whole wedge (screen clamp, PCB 1/2, battery, baseplate,
//  ports) carries over verbatim, translated back by the bay depth. `show` here
//  uses kb_* names so the parent file's own show-chain stays dormant.
//
//  Units: millimetres.   Render: see README-kb.md
//
//  Parts (set `show` below, or -D from the justfile):
//    "kb_assembled" – everything in place, coloured (keyboard ghosted in)
//    "kb_body"      – the one-piece shell (bay floor + tray posts integral)
//    "kb_baseplate" – the wedge chassis, re-widened (drops in from below)
//    "kb_section"   – vertical cut: tray stack in front, screen clamp behind
//    "kb_print"     – printable parts laid out (body, baseplate, bracket)
// ============================================================================

include <typoena-case.scad>

show = "kb_assembled";
kb   = "60";                 // "60" (GH60 tray) | "40" (Planck)

// ---- keyboard PCB + plate (datasheet / ecosystem-standard numbers) ---------
kb_pcb_w   = kb=="60" ? 285.00 : 226.50;
kb_pcb_d   = kb=="60" ?  94.60 :  73.50;
kb_plate_w = kb=="60" ? 285.75 : 226.50;   // universal 60% plate is the widest part
kb_plate_d = kb=="60" ?  95.25 :  73.50;   // sandwich plate follows the PCB outline
// tray-mount posts, (x from PCB LEFT edge, y from PCB TOP/back edge).
// The 40 set is the Planck's five M2 (Ø2.2) holes off OLKB's own CAD part
// (olkb_parts/planck/planck-pcb-module.kicad_mod, origin = board centre); the
// ten Ø2.9 holes there are acrylic-sandwich standoffs, unused here.
// << 60 set still VERIFY against a DZ60 drawing before printing >>
kb_holes   = kb=="60"
    ? [[25.2,27.9],[128.2,47.8],[259.8,27.9],[190.5,85.2]]      // GH60 std
    : [[18.25,17.75],[208.25,17.75],[18.25,55.75],[208.25,55.75],[113.25,36.75]];
// USB-C centre from PCB left edge. Same CAD part: the Planck's is a 13 mm notch
// in the back edge, rear-LEFT — not centred, so the passthrough slot is off-axis.
kb_usb_x   = kb=="60" ? 19 : 37.25;

// ---- bay geometry -----------------------------------------------------------
kb_int_w  = kb_plate_w + 0.75;   // plate floats on the switches; 0.75 wiggle
kb_int_d  = kb_plate_d + 0.75;
kb_d      = wall + kb_int_d;     // bay depth; the wedge's own front wall is the
                                 // SHARED wall between bay and cavity
kb_floor  = 3.0;                 // integral bay floor (no baseplate up front)
// CONTRACT: these two track Hf. Hf is the bay/cavity shared wall, so the wall
// behind the TOP KEY ROW rises with it while the keyboard stack does not — at the
// base file's +2 and these left at 8.0 / 22 the caps drop from 3.1 mm proud of
// that wall to 1.1 and the back row sits in a well. Both carry the same +2, which
// restores 3.1 proud of the wall and 5.1 of the front rim exactly.
kb_post_h = 10.0;                // floor -> PCB bottom << VERIFY vs USB slot >>
Hk        = 24;                  // bay rim: hides the plate edge, caps sit proud
DT        = kb_d + D;            // total body depth

// width now comes from the keyboard, not the screen — this override reflows the
// whole included wedge (screen stays centred, PCB 2 + ports track the new right
// wall, posts and baseplate re-derive).
// MUST be a literal: an include-override is evaluated at the base file's first
// W assignment, before any variant variable exists (expressions land undef).
// Keep the kb/W pair in sync — the assert below refuses a mismatch.
//   kb="60" -> W = 291.30      kb="40" -> W = 232.05   ( = kb_int_w + 2*wall )
W = 291.30;
assert(abs(W - (kb_int_w + 2*wall)) < 0.01,
       "W is out of sync with kb — see the table above this assert");

// derived tray-stack heights (MX: plate top = PCB top + 5.0)
kb_pcb_z   = kb_floor + kb_post_h;            // PCB bottom face
kb_plate_z = kb_pcb_z + pcb_t + 5.0 - 1.5;    // plate bottom face (1.5 plate)
// PCB placed centred in the bay interior; its TOP edge (USB side) faces the deck
kb_px0 = (W - kb_pcb_w)/2;                    // PCB left edge, world X
kb_py1 = wall + (kb_int_d + kb_pcb_d)/2;      // PCB top/back edge, world Y

// ===========================================================================
//  keyboard bay
// ===========================================================================
// back pillars sit at the wedge's front-pillar XY so the two hulls share one
// continuous side wall and corner radius. They rise to Hf (not Hk): the rim
// top must meet the wedge's front-top edge flush — at Hk it stops 2 mm short
// and the wedge's receding corner leaves an open slot at each junction corner.
module bay_outer() {
    hull() for (x=[corner_r, W-corner_r]) {
        translate([x, corner_r,        0]) cylinder(h=Hk, r=corner_r);
        translate([x, kb_d + corner_r, 0]) cylinder(h=Hf, r=corner_r);
    }
}

module bay_cavity() {
    translate([W/2, wall + kb_int_d/2, kb_floor])
        linear_extrude(Hk) rrect(kb_int_w, kb_int_d, 3);
}

module bay_posts() {
    for (h = kb_holes)
        translate([kb_px0 + h[0], kb_py1 - h[1], kb_floor]) difference() {
            cylinder(h=kb_post_h, r=3);
            translate([0,0,-1]) cylinder(h=kb_post_h+2, r=standoff_pilot);
        }
}

module bay_feet() {
    for (fx=[corner_r+6, W-corner_r-6])
        translate([fx, corner_r+6, -foot_h]) cylinder(h=foot_h+0.1, r=foot_r);
}

// USB passthrough in the shared wall: keyboard PCB -> wedge cavity -> PCB 2's
// internal keyboard header. Sized for a low-profile USB-C plug head.
module kb_cable_cut() {
    translate([kb_px0 + kb_usb_x - 8, kb_d - 2, kb_pcb_z - 3])
        cube([16, wall + 4, 12], center=false);
}

// the old external keyboard USB-C opening, filled: that port turns inward
// (parallel 4-pin header on PCB 2), so the wall stays blank there.
// Local wedge coordinates — union it inside the translated wedge.
module kb_port_patch() {
    translate([port_x[1] - usbc_w/2 - 1, D - wall, port_z[1] - usbc_h/2 - 1])
        cube([usbc_w + 2, wall, usbc_h + 2]);
}

module variant_body() {
    difference() {
        union() {
            translate([0, kb_d, 0]) { case_body(); kb_port_patch(); }
            // bay back pillars reach corner_r past the shared wall — carve the
            // wedge cavity out of the bay solid too, or that band re-fills the
            // cavity's front (battery space) up to Hk
            difference() {
                bay_outer(); bay_cavity();
                translate([0, kb_d, 0]) body_cavity();
            }
            bay_posts();
            bay_feet();
        }
        kb_cable_cut();
    }
}

// ===========================================================================
//  ghosts (assembly renders)
// ===========================================================================
module ghost_keyboard() {
    cy = kb_py1 - kb_pcb_d/2;
    color("#2f6f4f") translate([W/2, cy, kb_pcb_z + pcb_t/2])
        cube([kb_pcb_w, kb_pcb_d, pcb_t], center=true);
    color("#4a4f55") translate([W/2, cy, kb_plate_z + 0.75])
        cube([kb_plate_w, kb_plate_d, 1.5], center=true);
    // keycap field, translucent — OEM-ish cap tops land ~10 mm over the plate
    %translate([W/2, cy, kb_plate_z + 1.5 + 5])
        cube([kb_pcb_w - 3, kb_pcb_d - 3, 9], center=true);
}

module kb_assembly() {
    color(C_body) variant_body();
    translate([0, kb_d, 0]) {
        ghost_screen();
        placed_bracket();
        ghost_boards();
        translate([0,0,-0.01]) color(C_plate) baseplate();
    }
    ghost_keyboard();
}

// ===========================================================================
//  show
// ===========================================================================
if (show == "kb_assembled") {
    kb_assembly();
} else if (show == "kb_body") {
    color(C_body) variant_body();
} else if (show == "kb_baseplate") {
    color(C_plate) baseplate();
} else if (show == "kb_section") {
    // remove the +X half; the kept LEFT half shows the tray stack, the USB
    // passthrough slot, and the screen clamp behind it
    difference() {
        union() {
            kb_assembly();
            translate([0, kb_d, 0]) placed_foam();
        }
        translate([W/2, -30, -70]) cube([W, DT + 60, 220]);
    }
} else if (show == "kb_print") {
    color(C_body)  variant_body();
    translate([0, -(D + 40), 0])          color(C_plate)   baseplate();
    translate([W/2, -(D + 40) - 60, 0])   color(C_bracket) bracket();
}
