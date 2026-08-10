// ============================================================================
//  Typoena — 3D-printed enclosure  ·  "typewriter body"  ·  rev v0 (concept)
// ----------------------------------------------------------------------------
//  A shallow sage wedge. The e-paper strip sits on a reclined deck where a
//  typewriter's sheet of paper would be; the keyboard you bring rests in front.
//  No platen part (keeps the print simple) — the rounded back-top edge is a
//  subtle roll that nods to one for free.
//
//  Everything here is PARAMETRIC. Numbers that come from a datasheet are noted;
//  numbers marked  << MEASURE >>  are best-guesses you must confirm against the
//  real board / breakout before printing a final.
//
//  Units: millimetres.   Render:  see hardware/case/README.md
//
//  Parts (set `show` below):
//    "assembled"   – everything in place, coloured (screen ghosted in)
//    "body"        – the shell only (print deck-up or on its back)
//    "bracket"     – the screen retaining frame (print flat)
//    "baseplate"   – the chassis / bottom cover (print flat)
//    "feet"        – the four stick-on feet, laid flat for printing
//    "print_plate" – all printed parts laid out side by side
//    "section"     – vertical cross-section: how the screen is trapped
//    "plan"        – exploded horizontal section: deck lifted off the cavity
//    "plan_up"     – just the top half (deck / screen / bracket)
//    "plan_down"   – just the bottom half (cavity: standoffs, bosses, ports)
//    "io_coupon"   – TEST PRINT: a flat slice of the back wall with only the
//                    I/O openings (2x USB-C, µSD, power button) — dry-fit check
// ============================================================================

show = "assembled";
$fn = 20;

// ---- printer compensation -------------------------------------------------
// The machine over-extrudes: holes come out ~0.5 mm small across, outer features
// ~0.5 mm large. That error is the machine's, not the parts', so it lives HERE
// once and every fit below derives from it — recalibrate the extruder and this
// is a one-line change instead of a sweep. XY only: Z is layer height, and the
// first-layer squish is the slicer's elephant-foot setting, not this.
// The value is still an estimate off the jammed first coupon. The I/O coupon
// measures it exactly — caliper its openings against nominal and set this to
// the delta before the body print (see README, "Printer offset").
print_bloat = 0.5;
// The real gap wanted at a panel opening, before the machine eats into it.
panel_slip  = 0.7;

// ---- body envelope --------------------------------------------------------
W        = 176;   // width  (X)  — screen 150.9 + bezel + walls
D        = 104;   // depth  (Y)  — front (keyboard) .. back (ports)
// The two heights MOVE AS A PAIR: theta is their difference over the pillar span,
// so shifting both by the same amount translates the whole deck plane vertically
// and leaves the recline, deck_L, screen_cy and the entire screen clamp untouched.
// That is what the +2 over the original 24/58 bought — headroom over PCB 1's front
// edge, which turning the board on end had cut to 0.75. See standoff_h.
// In the kb variant Hf is also the bay/cavity SHARED WALL, so Hk and kb_post_h
// must move with it or the top keycap row sinks into the wall — see README-kb.md.
Hf       = 26;    // height at the FRONT edge
Hb       = 60;    // height at the BACK edge  (Hf<Hb makes the reclined deck)
wall     = 2.4;   // side/back wall thickness
top_wall = 2.6;   // deck thickness (before the bezel lip is cut into it)
corner_r = 8;     // rounded vertical + top-edge radius (the "machined" look)

// deck slope, derived from the pillar centres (this is the *true* top plane)
theta    = atan((Hb - Hf) / (D - 2*corner_r));   // ~21 deg with the defaults
// >> THE ergonomics dial. Raise Hb for a more vertical, easier-to-read screen;
//    lower it for a flatter, more typewriter-like deck. 18-22 deg = shallow,
//    28-35 deg reads better when you're sitting close.

// ---- e-paper panel : GDEY0579T93 (datasheet) ------------------------------
G_w  = 150.92;  G_h = 56.94;  G_t = 1.0;   // glass outline W x H x thickness
A_w  = 139.00;  A_h = 47.74;               // active area (must stay uncovered)
// This panel's flex (FPC) leaves the LEFT short edge — the user's left as they
// face the screen, i.e. the low-X side (world x < W/2).
// Where the active area sits on the glass, as an offset from the glass centre
// (+x = toward the right, away from the FPC edge). Measured off the panel: border
// widths glass-edge→active of 9.0 left / 2.0 right / 4.0 top / 4.0 bottom, so
// off_x = (left - right)/2 = +3.5 and off_y = (top - bottom)/2 = 0. The wide
// border on the FPC side is the usual COG-on-flex layout, not a measuring error.
active_off_x = 3.5;
active_off_y = 0;
// The GLASS carries that offset, never the window: it shifts the opposite way so
// the aperture stays centred on the deck and the image lands on the machine's
// centreline. Letting the window carry it instead leaves a 21 mm bezel against a
// 14 mm one — rejected on sight, it reads as crooked. The glass sitting
// off-centre costs nothing: it lives under the bezel where no one sees it.
// Do NOT "simplify" this back to a centred glass. It only fits because the
// bracket's left arm and its boss pair were pulled inboard (br_ml, boss_x_l) —
// at the symmetric layout the bracket overshoots the left wall by 2.6 mm.
glass_dx = -active_off_x;
glass_dy = -active_off_y;

// ---- screen retention (glueless) ------------------------------------------
lip_over  = 4.0;   // how far the front bezel lip overlaps the glass border
lip_t     = 1.4;   // deck material left in FRONT of the glass (the visible lip)
glass_gap = 0.5 + print_bloat;   // clearance around the glass in its pocket. The
                   // glass is rigid and brittle: a pocket that prints 0.5 under
                   // doesn't take it at all, which costs more than the 0.25 mm
                   // per side of registration the compensation gives away.
foam_t    = 3.0;   // non-adhesive closed-cell foam gasket behind the glass, FREE
                   // thickness. It also buys the bosses their thread depth: the
                   // seat sits foam_c behind the glass, so a thin gasket leaves
                   // nowhere to tap. See br_seat.
foam_c    = 2.6;   // ...and its thickness once the bracket bottoms on the boss
                   // seats. The squash (foam_t - foam_c) IS the clamp preload —
                   // set by geometry, not by how hard the screw is turned. The
                   // glass is brittle and there is no torque spec.
bracket_t = 2.6;   // printed retaining frame thickness
fpc_w     = 34 + 2 + print_bloat;   // ribbon-slot span along the LEFT short edge
                   // (the FPC side) — measured ribbon 34 mm, + 2 mm clearance.
fpc_slot_x = 10;   // how far the slot reaches ACROSS the glass edge (X). Was 14;
                   // the glass now sits 3.5 mm left (glass_dx), where 14 would
                   // leave only ~1.8 mm of deck before the left wall. 10 keeps
                   // ~3.8 mm and is still ample for the flex's U-turn.

// ---- deck nameplate (engraved, faces the user) ----------------------------
name_text  = "TYPOENA";
name_size  = 6.5;             // cap height in mm
name_depth = 0.8;             // engrave depth — raise for a bolder, deeper cut
name_font  = "Monaspace Krypton";   // install once — see README (Nameplate font)

// through-aperture (a hair bigger than active, still smaller than glass minus
// 2*lip). Compensated: the aperture must never encroach on the active area, and
// A_h+1 leaves only 0.5 mm a side to give away.
A_ap_w = A_w + 2 + print_bloat;
A_ap_h = A_h + 1 + print_bloat;
P_w    = G_w + glass_gap;          // glass pocket (locates the glass in X/Y)
P_h    = G_h + glass_gap;

// screen placed centred on the deck (measured up the slope)
deck_L    = (D - 2*corner_r) / cos(theta);   // deck length along the slope
screen_cy = deck_L/2;                        // centre it
boss_r    = 3.4;                             // M2 self-tap boss for the bracket
boss_pilot = (2.0 + print_bloat)/2;          // its Ø2 pilot
br_screw_r = (2.9 + print_bloat)/2;          // M2 clearance through the bracket
// Bracket fixing points, in GLASS-local X/Y (the bracket is placed on the glass,
// so these are its hole positions and the bosses' positions both).
// Both pairs already clear the glass pocket in Y, so their X is free to slide —
// which is what makes the centred window possible. The LEFT pair is pulled in off
// the corner grid: mirrored at -(P_w/2+5) it would land 3.4 mm from the side wall
// once the glass shifts left, and drag the bracket's arm through it.
boss_x_r = P_w/2 + 5;      // right pair, unchanged
boss_x_l = -77;            // left pair, inboard (mirror would be -80.71)
boss_y   = P_h/2 + 5;
boss_xy  = [[boss_x_l, -boss_y], [boss_x_l, boss_y],
            [boss_x_r, -boss_y], [boss_x_r, boss_y]];
// bracket frame margins beyond the glass pocket — asymmetric so the left arm
// clears the side wall the glass has been shifted toward
br_ml = 5.5;   // LEFT margin
br_m  = 9;     // the other three

// ---- mounting, boards & battery (defined here: the ports below depend on it)
bp_t           = 2.6;    // baseplate thickness
standoff_h     = 3;      // board standoff height. PCB 1's FRONT edge is the tight
                         // spot in the whole cavity: the ceiling is 30.35 there
                         // (see pcb1_y0), so the 22 mm stack on 3 mm standoffs
                         // clears by 2.75 — and only because Hf/Hb carry +2 for
                         // exactly this. At the original 24/58 it was 0.75.
standoff_pilot = (1.6 + print_bloat)/2;   // pilot Ø1.6 for an M2 self-tapper
                         // (PCB holes are Ø2) — see post_pilot on the compensation
pcb_t          = 1.6;    // PCB thickness (for port-height maths)
// PCB 1 = ESP32 devkit + e-ink driver + MT3608 boost. 50(X) x 70(Y), back-LEFT,
// long axis running FRONT-BACK. Standing it up out of the old 70(X) x 50(Y) is
// what buys the screen ribbon its run: the board's FPC end now lands at the
// front-left, directly under the FRONT end of the deck slot (world y 27..61), and
// the front-left volume ahead of it — which the battery used to cross — is the
// flex's plenum. The header/Dupont side comes to rest on the BACK edge, so the
// 22 mm vertical F-F jumpers stay under the tall rear of the wedge and the ribbon
// to PCB 2 stays short. Rigid board is only 10 mm; 22 mm is the jumpers.
// Its own USB-C is reached by opening the case — no wall cutout.
pcb1_x0 = 4;             pcb1_x1 = pcb1_x0 + 50;   // X  4 .. 54
// Y 26 .. 96. Both ends are hard against something: the back edge keeps 5.6 mm
// for the ribbon to leave, and the front edge is where the wedge's ceiling
// (30.35) comes closest to the 22 mm stack — see standoff_h.
pcb1_y0 = 26;            pcb1_y1 = pcb1_y0 + 70;
pcb1_h  = 22;            // tallest point (rigid stack + vertical Dupont)
// PCB 2 = µSD + 2x USB-C + TP4056. 80(X) x 20(Y), along the BACK wall, right end;
// connectors overhang its back edge by 8 mm to meet the wall.
pcb2_x1 = W - wall - 6;  pcb2_x0 = pcb2_x1 - 80;   // X ~87.6 .. 167.6
pcb2_y1 = D - wall - 8;  pcb2_y0 = pcb2_y1 - 20;   // back edge 8 mm off the wall
pcb2_h  = 8;             // tallest point (µSD cage / USB-C shells) — the power
                         // button barrel has to clear this, see pwr_z
// corner holes, centres 2 mm in from each edge (Ø2 hole, 1 mm pad)
pcb1_holes = [[pcb1_x0+2,pcb1_y0+2],[pcb1_x1-2,pcb1_y0+2],
              [pcb1_x0+2,pcb1_y1-2],[pcb1_x1-2,pcb1_y1-2]];
pcb2_holes = [[pcb2_x0+2,pcb2_y0+2],[pcb2_x1-2,pcb2_y0+2],
              [pcb2_x0+2,pcb2_y1-2],[pcb2_x1-2,pcb2_y1-2]];
// LiPo 3700 mAh (96 x 33.5 x 10.3), flat in the FRONT-RIGHT — the void inside the
// L the two boards make (PCB 1 down the left, PCB 2 across the back). Shallow
// wedge space the board stack can't use, CG low + forward, leads out to the
// charger on PCB 2. Cell measured.
// NOT centred on the case any more: the front-LEFT corner it used to cross is the
// screen ribbon's plenum, and that is the whole point of the layout. X is boxed in
// on both sides — PCB 1's edge at 54, the front-right screw boss's free face at
// 160.5 — so 106.5 mm of band holds a 96 mm cell and the 6 / 4.5 either side is
// all the slack there is.
bat_w = 96;  bat_d = 33.5;  bat_h = 10.3;
bat_x0 = pcb1_x1 + 6;                          // X 60 .. 156, right of PCB 1
bat_x1 = bat_x0 + bat_w;
bat_y0 = wall + 4;                             // front edge just off the front wall

// ---- ports on the back wall  (I/O board = PCB 2) --------------------------
// PCB 2 lies flat at the back-right; its connectors overhang the board's back
// edge by 8 mm and face out through the BACK wall (horizontal insertion). The
// µSD/power end faces the case's RIGHT wall, so from the +X (right) end inward
// the order is: power switch, µSD, keyboard, charge.
// Port openings. These were the PLUG envelope off the USB-C spec sheet, not the
// parts: usbc_h 2.5 against shells that caliper 2.75-3.0, and it jammed the first
// coupon print. A coupon is a 45-minute print, so the openings are now sized to
// clear on the next one rather than to look tight — never take a connector
// opening from a datasheet again.
usbc_w   = 8.5;  usbc_h = 3.0;               // USB-C shell (W x H), both measured
                                             // off the part (9.0 was a guess at a
                                             // typical shell, 8.0 the plug)
sd_w     = 14.0; sd_h   = 2.0;               // microSD cage (W x H). W measured
                                             // (13.0 was the same datasheet guess
                                             // that jammed the USB-C); H is not
port_fit = panel_slip + print_bloat;         // slack on every port opening: the
                                             // plug has to pass with room, and the
                                             // connectors are held by PCB 2, not
                                             // by the panel
usbc_cz  = 3.5;                              // USB-C opening centre, above PCB top
// The µSD is pinned to the USB-C by a MEASURED step, bottom edge to bottom edge,
// not by a guess at its centre (sd_cz was 3.0, which put both bottoms level).
sd_rise  = 1.0;                              // cage bottom, ABOVE the shell bottoms
sd_cz    = usbc_cz - usbc_h/2 + sd_rise + sd_h/2;   // -> 4.0
pcb2_z   = bp_t + standoff_h + pcb_t;        // PCB 2 top face height off the floor
// per-port centre heights off the floor       [charge, keyboard, µSD]
port_z   = [pcb2_z+usbc_cz, pcb2_z+usbc_cz, pcb2_z+sd_cz];
// PCB 2 is flipped vs how you view it: the charge end sits inward (low X), the
// µSD/power end faces the RIGHT wall.
// Spacing is MEASURED centre to centre, which is what a caliper can actually
// reach on a populated board — edge gaps can't be got at once the shells are on.
// This is also what caught the µSD: the old 8/7/5 edge-gap chain reproduced the
// USB-C pitch exactly (15.5, so usbc_w = 8.5 and its 7 mm gap are both right),
// but its 5 mm µSD gap gave 16.25 against a measured 15.0 — the slot was 1.25 mm
// too far right, twice the slack that would have covered it.
port_pitch = [15.5, 15.0];                   // charge->keyboard, keyboard->µSD
// Where the cluster sits on the board: measured edge to edge, PCB 2's left edge
// to the charge shell (was 8 off the old chain). This one slides all three ports
// together, so it is the reading the whole block hangs off.
chg_gap  = 7;
chg_cx   = chg_gap + usbc_w/2;                           // -> 11.25
port_x   = [pcb2_x0 + chg_cx,                                        // -> 12.25
            pcb2_x0 + chg_cx + port_pitch[0],                        // -> 27.75
            pcb2_x0 + chg_cx + port_pitch[0] + port_pitch[1]];       // -> 42.75

// ---- power on/off switch (latching push button, inline in the battery feed) --
// A push-on / push-off (latching) button that makes/breaks the battery-side power
// feed — press once to power up, again to cut it, so the machine is genuinely OFF
// between sessions instead of idling on the LiPo. NOT wired to EN/GND (that would
// be a momentary reset): it sits inline on the power rail. Panel-mounts through the
// back wall, out past the µSD toward the RIGHT wall, so it's never hit while typing.
// No reset/BOOT button is exposed — on the S3 both are recovery-only (auto-download
// handles flashing), so like the ESP32's own USB-C they're reached by opening up.
pwr_btn  = true;             // set false to omit the switch hole entirely
pwr_d    = 13.5;             // switch barrel Ø (the part Julien bought)
pwr_fit  = 0.4;              // panel-hole clearance on the barrel Ø. NOT the ports'
                             // panel_slip + print_bloat: the first printed coupon
                             // dropped the switch in perfectly at this 0.4 (Ø13.9),
                             // and the 1.2 that followed came out loose — Ø14.7 is
                             // wider than pwr_body_d, so nothing bore against the
                             // wall. Measured on the print; leave it alone.
pwr_r    = (pwr_d + pwr_fit) / 2;
pwr_body_d = 14;             // WIDEST thing behind the panel — nut across corners,
                             // body OD, solder lugs. NOT the barrel: this is what
                             // decides the clearance below.  Measured off the part.
pwr_inset= 20;               // button CENTRE, measured in from the RIGHT outer face
pwr_x    = W - pwr_inset;    // 156 — past the µSD (right edge @ 138.0), 4.6 mm of
                             // flat wall left before the back-right corner blend
// The switch is a loose panel-mount part wired back to PCB 2, not mounted on it.
// At Ø13.5 it cannot avoid PCB 2 in plan — the board runs to x=167.6, and the only
// clear X band is the board gap, which turning PCB 1 widened to 33.6 without
// helping: the back-centre baseplate post sits in the middle of it and leaves
// 12.3 mm a side, still under the Ø13.9 hole. So the barrel MUST fly over the
// board, and the clearance below it is the reserve for parts PCB 2 doesn't have:
pwr_clear = 4;               // headroom kept free above PCB 2 for future parts
pwr_z    = bp_t + standoff_h + pcb2_h + pwr_clear + pwr_body_d/2;   // ~26

// ---- baseplate / chassis --------------------------------------------------
// Clearance so the plate drops into the shell. Both mating faces are printed and
// both err inward: the plate's outer grows by print_bloat while the shell cavity
// shrinks by it, so the gap is eaten TWICE — uncompensated 0.5 goes negative and
// the plate simply won't go in.
bp_gap     = 0.5 + 2*print_bloat;
foot_r     = 7;    // round feet (the little typewriter feet)
foot_h     = 3.5;
// "none"     – no feet (current: deferred to a later version)
// "separate" – printed as their own part (show="feet") and stuck on afterwards
// "fused"    – hanging off the baseplate as one piece.  AVOID: the part then
//              rests on four discs and the whole plate becomes a 3.5 mm overhang
//              needing support across its footprint, with the standoffs and
//              battery nibs floating above it. The other two modes print
//              flat-face-down with every feature growing upward off the bed.
feet_mode = "none";
// The two FRONT feet sit over baseplate screw holes, so they carry the clearance
// hole through plus a counterbore for the head — otherwise a glued-on foot either
// blocks the screw or rocks on its head.
// Screw features are all holes a real fastener passes through or bites into, so
// each is written as its true diameter + print_bloat, halved for a radius.
foot_bore_r = (3.6 + print_bloat)/2;   // screw clearance through the foot
foot_cb_r   = (6.0 + print_bloat)/2;   // head counterbore radius
foot_cb_h   = 2.0;   // head counterbore depth (from the foot's ground face) — Z,
                     // so layer height governs it, not print_bloat
// Baseplate screw bosses. Rectangular pads FUSED INTO THE WALLS they sit against,
// not free-standing posts: the box overshoots the shell by post_out and the
// intersection with body_outer() trims it flush, so "touching the wall" is a
// property of the construction instead of a number that drifts when wall or
// corner_r moves. The v0 round posts stood 4.4 mm clear of every wall and 3.3 mm
// short of the deck — they hung off the bracket bosses, whose M2 pilots they
// plugged, and they overlapped the baseplate over their whole bottom 2.6 mm.
post_pad   = 4.5;  // material from the screw axis out to the boss's FREE faces
post_out   = 8;    // how far the box is driven THROUGH the wall before
                   // body_outer() trims it — any value past the wall works
post_pilot = (2.3 + print_bloat)/2;   // Ø2.3 pilot. A self-tapper wants a tight
                   // pilot, but Ø2.3 printing as Ø1.8 is past tight and splits
                   // the boss — compensation protects the part, not the fit.
post_h     = 10;   // boss height above bp_t. The WALLS carry the boss, so height
                   // is only ever about thread engagement — 8 mm of M2.5 in PLA
                   // is >3x its Ø, well past the 2-2.5x a self-tapper wants.
                   // Anything above the pilot is inert: the load path is
                   // screw -> boss -> wall and never leaves this band.
post_pilot_h = post_h - 2;   // BLIND, with a 2 mm roof. A through pilot would let
                   // an over-long screw push its tip into the cavity, and at the
                   // front corners that lands in the bracket boss overhead.

// ---- colours (for the assembled render) -----------------------------------
C_body   = "#B6CEB4";
C_plate  = "#C9C3B2";
C_bracket= "#2B2B2B";
C_screen = "#F7F4EA";
C_foam   = "#8a8f94";

// ---- cutaway sections -----------------------------------------------------
plan_z       = 22;   // height of the horizontal "plan" cut
plan_explode = 62;   // gap between the halves in the exploded "plan" view

// ===========================================================================
//  helpers
// ===========================================================================
module rrect(w, d, r) {                       // 2D rounded rectangle, centred
    hull() for (mx=[-1,1], my=[-1,1])
        translate([mx*(w/2-r), my*(d/2-r)]) circle(r=r);
}

// place children onto the reclined deck plane. Origin at the FRONT-TOP edge
// (world y=0, z=Hf) — where the true hull top surface actually begins; anchor
// it at the pillar centre instead and everything lands ~3mm below the surface.
// local frame: X = width, Y = up the slope, Z = out of the deck (normal).
module on_deck() {
    translate([W/2, 0, Hf]) rotate([theta, 0, 0]) children();
}

// ===========================================================================
//  body
// ===========================================================================
module body_outer() {
    hull() {
        translate([corner_r,     corner_r,     0]) cylinder(h=Hf, r=corner_r);
        translate([W-corner_r,    corner_r,     0]) cylinder(h=Hf, r=corner_r);
        translate([corner_r,      D-corner_r,   0]) cylinder(h=Hb, r=corner_r);
        translate([W-corner_r,    D-corner_r,   0]) cylinder(h=Hb, r=corner_r);
    }
}

module body_cavity() {
    ri = corner_r - wall;
    hull() {
        translate([corner_r,   corner_r,   -3]) cylinder(h=Hf-top_wall+3, r=ri);
        translate([W-corner_r, corner_r,   -3]) cylinder(h=Hf-top_wall+3, r=ri);
        translate([corner_r,   D-corner_r, -3]) cylinder(h=Hb-top_wall+3, r=ri);
        translate([W-corner_r, D-corner_r, -3]) cylinder(h=Hb-top_wall+3, r=ri);
    }
}

// baseplate screw bosses: two at the FRONT corners + one at the BACK centre.
// The back corners are taken by the PCB 1 / PCB 2 standoffs, so a corner boss
// there would clash — the third drops into the gap between the two boards.
// The back screw stays 6 mm off the wall's inner face: the baseplate's own edge
// is at D-wall-bp_gap/2, so any closer and the plate keeps under 3 mm of rim
// outboard of the clearance hole for the head to pull against.
post_xy = [[corner_r+3,          corner_r+3],     // front-left  corner
           [W-corner_r-3,        corner_r+3],     // front-right corner
           [(pcb1_x1+pcb2_x0)/2, D-wall-6]];      // back-centre, in the board gap
// Boss footprints [x0, x1, y0, y1]. A face driven past the shell is a FUSED face:
// the two front boxes run out through both corner walls, the back one through the
// back wall. Their free faces sit post_pad from the screw axis.
post_box = [[-post_out,              post_xy[0][0]+post_pad,
             -post_out,              post_xy[0][1]+post_pad],
            [post_xy[1][0]-post_pad, W+post_out,
             -post_out,              post_xy[1][1]+post_pad],
            [post_xy[2][0]-post_pad, post_xy[2][0]+post_pad,
             post_xy[2][1]-post_pad, D+post_out]];
// Foot centres. The two FRONT feet are concentric with the front screw posts on
// purpose: the screw then lands dead centre in the disc, so the head counterbore
// keeps a full 4 mm of wall all round. Offset even 3 mm and that wall drops to
// ~0.2 mm and will not print. The back pair is decorative (the only back post is
// centre-back, in the board gap) and stays on the corner grid.
// [x, y, takes a screw?]
foot_pos = [[post_xy[0][0], post_xy[0][1], true ],
            [post_xy[1][0], post_xy[1][1], true ],
            [corner_r+6,    D-corner_r-6,  false],
            [W-corner_r-6,  D-corner_r-6,  false]];
// one foot, sitting on the ground plane (ground face at z=0, top face at foot_h)
module foot(screwed) {
    difference() {
        cylinder(h=foot_h, r=foot_r);
        if (screwed) {
            translate([0,0,-1])       cylinder(h=foot_h+2,  r=foot_bore_r);
            translate([0,0,-0.01])    cylinder(h=foot_cb_h, r=foot_cb_r);
        }
    }
}
// the four feet placed under the baseplate (renders only — see feet_mode)
module feet_parts() {
    for (f = foot_pos)
        translate([f[0], f[1], -foot_h]) foot(f[2]);
}
// the four feet laid out flat for printing, ground face on the bed
module feet_plate() {
    for (i = [0:len(foot_pos)-1])
        translate([foot_r + i*(2*foot_r + 4), foot_r, 0]) foot(foot_pos[i][2]);
}
// Solid pads only — the pilots are cut in case_body(), see the contract there.
// They start at bp_t so the baseplate seats under them instead of through them,
// and stop at post_h; the intersection is what trims their overshoot flush with
// the walls they are driven through.
module screw_bosses() {
    intersection() {
        body_outer();
        for (b = post_box)
            translate([b[0], b[2], bp_t]) cube([b[1]-b[0], b[3]-b[2], post_h]);
    }
}
module screw_pilots() {
    for (p = post_xy)
        translate([p[0], p[1], bp_t-1]) cylinder(h=post_pilot_h+1, r=post_pilot);
}

// 4 bosses just OUTSIDE the glass pocket for the retaining bracket (M2 self-tap).
// CONTRACT: the boss's free end IS the bracket's seating face. The screw pulls the
// bracket onto it and stops there, so glass clamp and foam squash both follow from
// br_seat instead of from screwdriver feel. A boss that reaches PAST the bracket
// seats nothing — the v0 length (…+ bracket_t + 6) drove a Ø6.8 column straight
// through the bracket's Ø3.4 screw hole, 277 mm³ of interference.
br_seat     = lip_t + G_t + foam_c;   // seat depth below the deck's outer face
br_boss_len = br_seat - lip_t;        // the column itself: pocket floor -> seat
pilot_skin  = 1.0;   // deck left over the BLIND pilot on the face the user sees.
                     // 0.4 mm was two bridged layers over a Ø2.5 hole: a
                     // self-tapper punches through and dimples the visible deck.
module bracket_cols(r, z0, h) {
    on_deck() for (p = boss_xy)
        translate([glass_dx + p[0], screen_cy + glass_dy + p[1], z0])
            cylinder(h=h, r=r);
}
module bracket_bosses() { bracket_cols(boss_r, -br_seat, br_boss_len); }
// runs 1 mm past the seat so the cut is clean, and stops pilot_skin short of the
// deck face — 4 mm of M2 bite, which sizes the screw at M2 x 6.
module bracket_pilots() { bracket_cols(boss_pilot, -br_seat-1, br_seat-pilot_skin+1); }

// deck cuts: through-aperture, glass pocket (leaves the front lip), FPC slot
module screen_cuts() {
    on_deck() translate([0, screen_cy, 0]) {
        // window — always on the ACTIVE area, wherever the glass has been put.
        // glass_dx cancels active_off_x, so this lands on the deck centre; keep
        // the expression rather than hardcoding 0, or the window silently stops
        // tracking the active area and rides onto the pixels.
        translate([glass_dx + active_off_x, glass_dy + active_off_y, -30])
            cube([A_ap_w, A_ap_h, 66], center=true);
        // glass pocket behind the lip — shifted so the ACTIVE area lands centred
        translate([glass_dx, glass_dy, -30-lip_t])
            cube([P_w, P_h, 60], center=true);
        // FPC clearance: an internal notch in the LEFT recess wall, kept BELOW
        // the bezel lip so it stays invisible from outside — the flex passes the
        // glass's left edge and folds back into the cavity, to the breakout
        translate([glass_dx-P_w/2, glass_dy, -30-lip_t])
            cube([fpc_slot_x, fpc_w, 60], center=true);
    }
}

module port_cuts() {
    // USB-C (charge, keyboard) + microSD through the BACK wall (y = D)
    for (i=[0:2]) {
        pw = ((i==2) ? sd_w : usbc_w) + port_fit;
        ph = ((i==2) ? sd_h : usbc_h) + port_fit;
        translate([port_x[i], D-wall-1, port_z[i]])
            rotate([-90,0,0]) linear_extrude(wall+2)
                offset(r=0.8) square([pw-1.6, ph-1.6], center=true);
    }
}

// power switch mounting hole through the back wall (y = D)
module power_cut() {
    if (pwr_btn)
        translate([pwr_x, D-wall-1, pwr_z])
            rotate([-90,0,0]) cylinder(h=wall+2, r=pwr_r);
}

// engraved nameplate on the DECK, in the band between the front edge and the
// screen — faces the user as they write. Sits flat on the reclined deck.
module nameplate() {
    name_y = (screen_cy - P_h/2) / 2;     // centre of the front deck band
    // centred on the deck, like the window above it — not on the shifted glass
    on_deck() translate([0, name_y, -name_depth])
        linear_extrude(name_depth + 0.6)
            text(name_text, size=name_size, halign="center", valign="center",
                 font=name_font, spacing=1.1);
}

module case_body() {
    difference() {
        union() {
            difference() { body_outer(); body_cavity(); }
            screw_bosses();
            bracket_bosses();
        }
        // CONTRACT: every pilot is cut AFTER the bosses are unioned. Cut one
        // inside its own boss and a neighbour that grows into it fills it back
        // in silently — which is what the v0 corner posts did to both front
        // bracket pilots, blinding them 6 mm into a 12 mm boss.
        screw_pilots();
        bracket_pilots();
        screen_cuts();
        port_cuts();
        power_cut();
        nameplate();                 // engrave (comment out for a blank face)
    }
}

// ===========================================================================
//  screen retaining bracket  (printed flat, screwed to the 4 bosses)
// ===========================================================================
module bracket() {
    // asymmetric frame: the left arm is trimmed (br_ml < br_m) because the glass
    // is shifted that way to centre the window. br_cx is the frame's own centre,
    // offset from the glass centre the bracket is placed on.
    // Frame size comes off the GLASS, not the pocket: the margins exist to overlap
    // the glass border, and pocket slack is clearance, not frame. Deriving it from
    // P_* coupled the frame to glass_gap, so compensating the pocket grew the arm
    // straight into the left wall (0.89 mm was the whole margin there).
    ow = G_w + br_ml + br_m;  oh = G_h + 2*br_m;
    br_cx = (br_m - br_ml)/2;
    // FPC U-turn clearance: a gap in the LEFT frame member. The flex leaves the
    // glass's back plane and folds ~180° to dive into the cavity toward the
    // breakout; a safe bend radius (~1.5-2 mm) makes that loop ~4 mm deep, too
    // deep for the foam gap, so it fouls this rigid frame unless relieved
    // here. Lines up with the body's FPC slot (screen_cuts) and the foam relief.
    difference() {
        // The bracket is placed on the GLASS centre, but its window has to clear
        // the ACTIVE area — which sits active_off_* away from that centre.
        linear_extrude(bracket_t)
            difference() {
                translate([br_cx, 0]) rrect(ow, oh, 4);
                translate([active_off_x, active_off_y])
                    rrect(A_ap_w+2, A_ap_h+2, 2);
                // relief from outside the left frame edge in to the window edge
                translate([br_cx - ow/2 - 2, -fpc_w/2])
                    square([(active_off_x - (A_ap_w+2)/2 + 2) - (br_cx - ow/2 - 2),
                            fpc_w]);
            }
        for (p = boss_xy)
            translate([p[0], p[1], -1]) cylinder(h=bracket_t+2, r=br_screw_r);
    }
}

// ===========================================================================
//  baseplate / chassis
// ===========================================================================
module baseplate() {
    iw = W - 2*wall - bp_gap;
    id = D - 2*wall - bp_gap;
    difference() {
        union() {
            // plate (centred on the footprint)
            translate([W/2, D/2, 0]) linear_extrude(bp_t) rrect(iw, id, corner_r-wall);
            // round feet underneath — only in "fused" mode, see feet_mode
            if (feet_mode == "fused") feet_parts();
            // board standoffs on top (PCB 1 back-left + PCB 2 back-right)
            for (h = concat(pcb1_holes, pcb2_holes))
                translate([h[0], h[1], bp_t]) cylinder(h=standoff_h, r=3);
            // battery cage nibs (front-right LiPo; foam/VHB tape does the rest)
            for (cx=[bat_x0-1, bat_x1+1], cy=[bat_y0-1, bat_y0+bat_d+1])
                translate([cx, cy, bp_t]) cylinder(h=5, r=1.6);
        }
        // screw clearance up into the body bosses (2 front corners + 1 back centre)
        for (p = post_xy)
            translate([p[0], p[1], -foot_h-1])
                cylinder(h=bp_t+foot_h+2, r=(3.2 + print_bloat)/2);   // M2.5 clear
        // standoff pilot holes
        for (h = concat(pcb1_holes, pcb2_holes))
            translate([h[0], h[1], bp_t-1]) cylinder(h=standoff_h+2, r=standoff_pilot);
        // cable / connector relief at the back
        translate([W/2, D-wall-3, -1]) cube([30, 8, bp_t+2], center=false);
    }
}

// ===========================================================================
//  I/O fit coupon  (test print — the back wall's openings, nothing else)
// ---------------------------------------------------------------------------
//  A slice of the REAL back wall, taken by intersecting case_body() with a box,
//  so the wall thickness, the opening shapes and their spacing are the shipping
//  geometry rather than a re-derivation. Dry-fit the two USB-C shells, a µSD
//  card and the power button in this before committing to a 10-hour body print.
//  Kept inside x <= W-corner_r so the slab is perfectly flat and lays on the bed.
// ===========================================================================
io_x0 = pcb2_x0 - 6;                 // just left of the charge port
io_x1 = W - corner_r;                // back wall stays flat up to the corner tangent
io_z0 = bp_t;                        // floor level
io_z1 = pwr_z + pwr_r + 6;           // 6 mm of margin above the button hole

module io_coupon() {
    // lay the wall flat, outer face down, front-left corner at the origin
    translate([-io_x0, -io_z0, D]) rotate([-90, 0, 0])
    intersection() {
        case_body();
        translate([io_x0, D-wall-0.01, io_z0])
            cube([io_x1-io_x0, wall+0.02, io_z1-io_z0]);
    }
}

// ===========================================================================
//  assemblies
// ===========================================================================
module ghost_screen() {
    on_deck() translate([glass_dx, screen_cy+glass_dy, -lip_t-G_t/2])
        color(C_screen) cube([G_w, G_h, G_t], center=true);
}
// LiPo lying flat on the baseplate, front-right
module ghost_battery() {
    translate([(bat_x0+bat_x1)/2, bat_y0+bat_d/2, bp_t+bat_h/2])
        color("#3f7d4f") cube([bat_w, bat_d, bat_h], center=true);
}
// a board slab on its standoffs + a translucent envelope for its tall parts
module ghost_pcb(x0, y0, x1, y1, htot) {
    w = x1-x0; d = y1-y0;
    translate([(x0+x1)/2, (y0+y1)/2, bp_t+standoff_h]) color("#2f6f4f") {
        linear_extrude(pcb_t) square([w, d], center=true);
        translate([0,0,pcb_t]) %linear_extrude(htot-pcb_t)
            square([w*0.7, d*0.7], center=true);
    }
}
module ghost_boards() {
    ghost_battery();
    ghost_pcb(pcb1_x0, pcb1_y0, pcb1_x1, pcb1_y1, pcb1_h);   // back-left, tall
    ghost_pcb(pcb2_x0, pcb2_y0, pcb2_x1, pcb2_y1, pcb2_h);   // back-right, low I/O
}
module placed_bracket() {
    on_deck() translate([glass_dx, screen_cy+glass_dy, -br_seat-bracket_t])
        color(C_bracket) bracket();
}
// foam gasket (non-adhesive) — a border frame between glass and bracket, with
// its LEFT border opened over the FPC span so the U-turning flex isn't clamped
module foam(t=foam_t) {
    linear_extrude(t)
        difference() {
            rrect(P_w+4, P_h+4, 3);
            translate([active_off_x, active_off_y]) rrect(A_ap_w, A_ap_h, 2);
            translate([-(P_w+4)/2 - 2, -fpc_w/2])
                square([(active_off_x - A_ap_w/2 + 2) + (P_w+4)/2 + 2, fpc_w]);
        }
}
module placed_foam() {
    on_deck() translate([glass_dx, screen_cy+glass_dy, -br_seat])
        color(C_foam) foam(foam_c);      // drawn squashed, i.e. as assembled
}
// full coloured assembly, reused by the plan sections
module plan_assembly() {
    color(C_body)   case_body();
    ghost_screen();
    placed_foam();
    placed_bracket();
    ghost_boards();
    translate([0,0,-0.01]) color(C_plate) baseplate();
}
// the two halves of the horizontal cut at plan_z
module plan_down() {     // bottom: the cavity (standoffs, posts, ports)
    intersection() {
        plan_assembly();
        translate([-60, -60, plan_z-200]) cube([W+120, D+120, 200]);
    }
}
module plan_up() {       // top: the deck / lid (screen, bracket)
    intersection() {
        plan_assembly();
        translate([-60, -60, plan_z]) cube([W+120, D+120, 200]);
    }
}

if (show == "assembled") {
    color(C_body)   case_body();
    ghost_screen();
    placed_bracket();
    ghost_boards();
    translate([0,0,-0.01]) color(C_plate) baseplate();
    if (feet_mode == "separate") color(C_plate) feet_parts();
} else if (show == "body") {
    color(C_body) case_body();
} else if (show == "bracket") {
    color(C_bracket) bracket();
} else if (show == "baseplate") {
    color(C_plate) baseplate();
} else if (show == "feet") {
    color(C_plate) feet_plate();
} else if (show == "print_plate") {
    color(C_body)    case_body();
    translate([W+30, 0, 0])           color(C_plate)   baseplate();
    translate([W+30, D+30, 0])        color(C_bracket) bracket();
    if (feet_mode != "none") translate([W+30, D+90, 0]) color(C_plate) feet_plate();
} else if (show == "section") {
    // VERTICAL slice (remove +X half): cut face shows the screen clamp, and the
    // retained LEFT half exposes the internal FPC clearance behind the bezel
    difference() {
        union() {
            color(C_body)   case_body();
            ghost_screen();
            placed_foam();
            placed_bracket();
            translate([0,0,-0.01]) color(C_plate) baseplate();
        }
        translate([W/2, -30, -70]) cube([W, D+60, 220]);
    }
} else if (show == "plan") {
    // EXPLODED horizontal section: deck/lid half lifted off the cavity half
    plan_down();
    translate([0, 0, plan_explode]) plan_up();
} else if (show == "plan_up") {
    plan_up();       // just the top half — deck, screen, bracket
} else if (show == "plan_down") {
    plan_down();     // just the bottom half — cavity, standoffs, ports
} else if (show == "io_coupon") {
    color(C_body) io_coupon();
}
