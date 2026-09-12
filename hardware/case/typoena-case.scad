// ============================================================================
//  Typoena — 3D-printed enclosure  ·  "typewriter body"
// ----------------------------------------------------------------------------
//  A shallow sage wedge. The e-paper strip sits on a reclined deck where a
//  typewriter's sheet of paper would be; the keyboard you bring rests in front.
//  No platen part (keeps the print simple) — the rounded back-top edge is a
//  subtle roll that nods to one for free.
//
//  Everything here is PARAMETRIC. Every number is off a datasheet or measured on
//  the part; the source is noted where it matters.
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

// ---- what this model is ---------------------------------------------------
// NOMINAL GEOMETRY ONLY. Every dimension below is the part as it must END UP,
// and every clearance is the functional gap wanted on the finished assembly.
// The machine's own error — XY over-extrusion — is NOT modelled here and must
// not be: it belongs to the process, not to the part, and it is corrected in the
// slicer. The setting, its measured value and how to re-measure it are in
// MANUFACTURING.md, which is required reading before any print. A part sliced
// without it will not assemble.

// ---- fasteners ------------------------------------------------------------
// ONE family for everything that screws into the BODY: a ruthex RX-6-32x3.8
// brass heat-set insert (#6-32, Ø4.7 body / 3.8 long) taking a #6-32 pan screw,
// 7 mm of thread under a 1 mm x Ø6.5 head. Both are measured parts, not
// catalogue numbers.
// The bore is a PLAIN cylinder: no relief is modelled for the insert's head end,
// the iron sinks it. HAZARD: nothing in the part then says when to stop pressing
// — the bore is deliberately deeper than the insert, so flush is by eye (or by a
// depth stop on the iron), and the boss's free end is a seating face on both
// joints. See the assembly order in the README.
// WHY inserts and not the self-tapped PLA of v0: these are the two joints that
// get OPENED — the baseplate every time the machine is serviced, the bracket
// every time the glass comes out. A self-tapped thread in PLA is good for a
// handful of cycles and then it is a stripped hole in a 10-hour print.
// The BOARD is in this family too: its four Ø3.7 holes are a #6-32 clearance and
// it screws down into an insert in each standoff, so nothing in the machine is a
// self-tapped thread and nothing on the baseplate has to be drilled.
ins_hole_d  = 4.8;   // hole the datasheet asks for. The insert melts its own seat,
                     // so this is not a clearance fit — the brass must grip what it
                     // is pressed into.
ins_min_h   = 4.8;   // ...and its minimum DEPTH. Same number as the diameter by
                     // coincidence only — never fold the two together. Going
                     // deeper is free and every bore here does; the floor is what
                     // matters.
ins_len     = 3.8;   // insert length — the thread the screw actually gets
ins_wall    = 1.8;   // datasheet minimum material around the hole. Heat-setting
                     // pushes melt outward, so every boss below is sized to beat
                     // this, and asserts hold the line if a radius drifts.
scr_thread  = 7.0;   // thread under the head (measured) — this is the one that
                     // sizes ENGAGEMENT, i.e. how much brass the screw actually
                     // grips, and the asserts hold it over 2.5 mm.
scr_thread_max = 8.0;   // ...and the one that sizes DEPTH, deliberately 1 mm over:
                     // the datum is a screw laid on a tape measure. Over-budgeting
                     // buys hole depth (free); under-budgeting bottoms the screw
                     // and jacks the joint back open, or punches the deck.
scr_head_d  = 6.5;   // head Ø  (measured)
scr_head_h  = 1.0;   // head thickness (measured)
scr_clear_d = 3.9;   // clearance hole: #6-32 majors at Ø3.5
// depth of a blind insert bore: everything the thread can still bring past `over`
// (the insert plus whatever tip is left over), floored at the datasheet's 4.8.
// `over` is the material in front of the bore (plate crossed, bracket crossed)
// that the thread spends before it arrives. Overshooting costs only boss height,
// which the roof assert holds — bottoming a screw jacks the joint open.
function ins_bore_h(over) = max(ins_min_h, scr_thread_max - over);
// ...and how much brass the screw grips once it has crossed `over`. Capped at the
// insert: thread past the far end grips nothing.
function ins_grip(over) = min(ins_len, scr_thread - over);

// ---- body envelope --------------------------------------------------------
W        = 176;   // width  (X)  — screen 150.9 + bezel + walls
D        = 104;   // depth  (Y)  — front (keyboard) .. back (ports)
// The two heights MOVE AS A PAIR: theta is their difference over the pillar span,
// so shifting both by the same amount translates the whole deck plane vertically
// and leaves the recline, deck_L, screen_cy and the entire screen clamp untouched.
// Raising Hf alone flattens the deck by ~1 mm per degree. What they have to buy
// is ceiling over the board's front edge, which is asserted at pcb_ceiling — the
// board stack clears by 19 mm here, so this pair is free to come down if a
// shorter machine is ever wanted.
// In the kb variant Hf is also the bay/cavity SHARED WALL, so Hk and kb_post_h
// must move with it or the top keycap row sinks into the wall — see README-kb.md.
Hf       = 28;    // height at the FRONT edge
Hb       = 62;    // height at the BACK edge  (Hf<Hb makes the reclined deck)
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
lip_t     = 2.4;   // deck material left in FRONT of the glass (the visible lip).
                   // Was 1.4. It is also HALF THE DEPTH BUDGET for the bracket's
                   // heat-set inserts — the deck over that bore is the face the
                   // user looks at. See br_seat.
glass_gap = 0.5;   // clearance around the glass in its pocket, 0.25 a side. The
                   // glass is rigid and brittle: a pocket that comes out under
                   // doesn't take it at all, which costs more than the quarter
                   // millimetre of registration this gives away.
foam_t    = 5.0;   // non-adhesive closed-cell foam gasket behind the glass, FREE
                   // thickness. It also buys the bosses their thread depth: the
                   // seat sits foam_c behind the glass, so a thin gasket leaves
                   // nowhere to tap. See br_seat.
foam_c    = 3.5;   // ...and its thickness once the bracket bottoms on the boss
                   // seats. The squash (foam_t - foam_c) IS the clamp preload —
                   // set by geometry, not by how hard the screw is turned. The
                   // glass is brittle and there is no torque spec.
                   // CONTRACT: keep the squash in the 25-40% band. Below ~15% the
                   // print tolerance on br_seat eats the whole preload and the
                   // glass rattles; past ~45% a closed-cell foam densifies and the
                   // clamp force runs away on a brittle part. foam_t never moves
                   // without foam_c moving with it.
bracket_t = 3.6;   // printed retaining frame thickness. Was 2.6; the other half of
                   // the insert depth budget — every mm here is a mm of screw
                   // thread that never reaches the deck. See br_seat.
fpc_w     = 34 + 2;   // ribbon-slot span along the LEFT short edge (the FPC
                   // side) — measured ribbon 34 mm, + 2 mm clearance.
fpc_slot_x = 10;   // how far the slot reaches ACROSS the glass edge (X). Was 14;
                   // the glass now sits 3.5 mm left (glass_dx), where 14 would
                   // leave only ~1.8 mm of deck before the left wall. 10 keeps
                   // ~3.8 mm and is still ample for the flex's U-turn.

// ---- deck nameplate (engraved, faces the user) ----------------------------
name_on    = false;           // OFF: the deck ships blank. The geometry stays so
                              // the engrave is one flag away, and because the
                              // band it sits in is the only free deck area left
name_text  = "TYPOENA";
name_size  = 6.5;             // cap height in mm
name_depth = 0.8;             // engrave depth — raise for a bolder, deeper cut
name_font  = "Monaspace Krypton";   // install once — see README (Nameplate font)

// through-aperture (a hair bigger than active, still smaller than glass minus
// 2*lip). The aperture must never encroach on the active area, and A_h+1 leaves
// only 0.5 mm a side to give away.
A_ap_w = A_w + 2;
A_ap_h = A_h + 1;
P_w    = G_w + glass_gap;          // glass pocket (locates the glass in X/Y)
P_h    = G_h + glass_gap;

// screen placed centred on the deck (measured up the slope)
deck_L    = (D - 2*corner_r) / cos(theta);   // deck length along the slope
screen_cy = deck_L/2;                        // centre it
// Bracket boss, now a heat-set insert instead of an M3 self-tapper: Ø8.9 against
// the old Ø6.8. The insert is the reason this boss got fat — see the fastener
// block, and boss_r below for why it stayed at Ø8.9.
boss_bore  = ins_hole_d/2;                    // Ø4.8 insert bore
boss_r     = 4.45;   // Ø8.9. More wall than the datasheet minimum, and kept that
                     // way: the extra is free, and the layout around this
                     // diameter (boss_x_l, the bracket arm's coverage) is already
                     // solved. The assert below is what holds the datasheet
                     // minimum.
br_screw_r = scr_clear_d/2;   // #6-32 clearance through the bracket: a screw that
                     // meets a tight hole is turned through it.
// Bracket fixing points, in GLASS-local X/Y (the bracket is placed on the glass,
// so these are its hole positions and the bosses' positions both).
// Both pairs already clear the glass pocket in Y, so their X is free to slide —
// which is what makes the centred window possible. The LEFT pair is pulled in off
// the corner grid: mirrored at -(P_w/2+5) it would land 3.4 mm from the side wall
// once the glass shifts left, and drag the bracket's arm through it.
boss_x_r = P_w/2 + 5;      // right pair, unchanged
boss_x_l = -76.5;          // left pair, inboard (mirror would be -80.71). Pulled
                           // 0.5 further in when boss_r grew for the insert: at
                           // -77 the fat boss's outer edge landed flush with the
                           // bracket arm's own edge, so the seat the screw pulls
                           // against ran off the end of the arm.
boss_y   = P_h/2 + 5;
boss_xy  = [[boss_x_l, -boss_y], [boss_x_l, boss_y],
            [boss_x_r, -boss_y], [boss_x_r, boss_y]];
// bracket frame margins beyond the glass pocket — asymmetric so the left arm
// clears the side wall the glass has been shifted toward
br_ml = 5.5;   // LEFT margin
br_m  = 9;     // the other three

// ---- mounting, board & battery (defined here: the ports below depend on it)
bp_t           = 2.6;    // baseplate thickness
pcb_t          = 1.6;    // PCB thickness (for port-height maths)

// ---- the mainboard --------------------------------------------------------
// ONE board carries the machine. Its outline, hole grid and connector positions
// are read off hardware/pcb/devboard/typoena-devboard.kicad_pcb: every number
// below that describes the board is the board's own, and the case follows it.
// The four Ø3.7 holes are a #6-32 clearance, so the board joins the case's one
// fastener family — it screws DOWN into a heat-set insert in each standoff.
// HAZARD: the board goes in TURNED. KiCad's canvas has Y pointing DOWN, so the
// port edge is drawn at the BOTTOM; laying the board in with its ports at the
// BACK is a 180° turn in plan, and every X then reads from the other end. Every
// board-local X below is therefore `u = 116 - kicad_x`, measured from the
// board's LEFT edge AS INSTALLED — take one straight off the canvas and the
// whole cluster comes out mirrored. Y needs no such care: `kicad_y - 40` is the
// distance back from the board's front edge either way.
pcb_w       = 99;   pcb_d = 45;   pcb_r = 3;   // outline, corner radius
pcb_hole_in = 5;    // hole centres, in from both edges at all four corners
// The ESP32-S3-DevKitC-1 plugs into the two 1x22 rows and lies flat over the
// board's right half. It is the tallest thing in the cavity and the only part of
// the stack the PCB file does not carry — caliper it on the assembled board.
hdr_mate    = 8.5;  // 2.54 socket: board face -> devkit underside
devkit_t    = 1.6;
devkit_top  = 3.4;  // the WROOM-1 can, and the devkit's own USB shells under it
pcb_h       = hdr_mate + devkit_t + devkit_top;   // 13.5 over the board face
devkit_w    = 63;   devkit_d = 25.5;              // the devkit's own outline,
devkit_cu   = 69.83; devkit_cy = 22.53;           // centred on its header rows
                    // (board-local). Its antenna end overhangs the board's RIGHT
                    // edge by 2.33 mm — the widest the assembly ever gets, and
                    // what the power switch has to clear.

// Placement in Y is set by the ports: the USB-C shells stand usbc_proud past the
// board's back edge and must reach INTO the wall opening, so the board is pushed
// back until only pcb_gap_wall is left.
pcb_gap_wall = 0.8;
pcb_y1 = D - wall - pcb_gap_wall;   // back (port) edge  -> 100.8
pcb_y0 = pcb_y1 - pcb_d;            // front edge        -> 55.8
// X is as far LEFT as the back-left baseplate boss allows (asserted). J4 — the
// FFC that takes the panel — sits at the board's LEFT end (u 2…7.3) and the
// panel's flex drops through the deck slot at x 3.8…13.8: the two want to be as
// near each other as the boss lets them.
pcb_x0 = 18;
pcb_x1 = pcb_x0 + pcb_w;            // 117
pcb_holes = [[pcb_x0+pcb_hole_in, pcb_y0+pcb_hole_in],
             [pcb_x1-pcb_hole_in, pcb_y0+pcb_hole_in],
             [pcb_x0+pcb_hole_in, pcb_y1-pcb_hole_in],
             [pcb_x1-pcb_hole_in, pcb_y1-pcb_hole_in]];

// ---- standoffs ------------------------------------------------------------
// The board's four feet, on the baseplate. They carry a heat-set insert like
// every other joint that gets opened, so their height is set by the BORE and not
// by the bay: the screw crosses the board and the rest of its thread has to land
// in brass. The bore bottoms on the plate, which is the floor under it.
standoff_r      = 4.45;               // Ø8.9 — the same wall over a Ø4.8 bore the
                                      // bracket boss keeps; the assert holds it
standoff_bore   = ins_hole_d/2;
standoff_bore_h = ins_bore_h(pcb_t);  // 6.4, blind
standoff_h      = 7;   // ...and the WIRING BAY under the board: the panel's FPC
                       // extension crosses it west to east on its way to J4, and
                       // the battery and button pigtails share it.
pcb_z = bp_t + standoff_h + pcb_t;    // board TOP face, off the plate's
                                      // UNDERSIDE — z=0 for the whole model

// ---- battery --------------------------------------------------------------
// LiPo 3700 mAh (94 x 32 x 10.3), flat across the FRONT — the shallow end of the
// wedge, which the board stack cannot use anyway, and the heaviest part keeps
// the centre of gravity low and forward. Cell measured. Its leads reach J1 on
// the board's own front edge, ~25 mm away.
// The cage is NOT symmetric. It is a WALL at the cell's left end and two NIBS at
// its mid-length: the cell goes in from the right, passes between the nibs —
// which only hold it in Y — and slides left until it stops on the wall. The nibs
// sit at mid-length because that is where a pouch cell bows, and it is the one
// spot from which neither end of the cell can lever itself out.
bat_w = 94;  bat_d = 32;  bat_h = 10.3;
// The WALL is the datum, and what bounds it on the left is the flex plenum: the
// panel's FPC drops through the deck slot into the front-left floor and its
// slack coils there. On the right there is only the front-right screw boss, and
// a pouch cell must never be asked to take a rigid printed corner — asserted.
plenum_x1    = 56;    // front-left floor kept clear for the FPC and its slack
bat_wall_t   = 2;     // cage wall, grown LEFT (away from the cell), so the cell's
                      // own position never moves when this does
bat_x0 = plenum_x1 + bat_wall_t;   // 58 — the wall's face IS the cell's left end
bat_x1 = bat_x0 + bat_w;           // 152
bat_y0 = wall + 4;                 // front edge just off the front wall
bat_nib_x = (bat_x0 + bat_x1)/2;   // 105
bat_nib_h = 5;        // wall and nibs share it: half the cell's height, the rest
                      // is the foam/VHB tape's job

// ---- ports on the back wall -----------------------------------------------
// All three user ports sit on the board's back long edge and face out through
// the BACK wall (horizontal insertion). Their X is the board's own, so the whole
// cluster follows pcb_x0 and nothing else can move it.
// Listed LEFT TO RIGHT across the back wall, which after the turn above is
// µSD, keyboard, charge. Index 0 is the µSD and the cuts below rely on that.
port_lx  = [26.1875, 54.00, 71.89];   // board-local X: µSD, keyboard, charge
port_x   = [for (lx = port_lx) pcb_x0 + lx];
// USB-C: HRO TYPE-C-31-M-12. The shell sits ON the board face, and its mouth
// stands usbc_proud past the board's edge — which is what pushes the board back
// against the wall, since the mouth has to end up inside the opening and not
// behind it.
usbc_w   = 8.94;  usbc_h = 3.26;
usbc_proud = 1.3;
usbc_cz  = usbc_h/2;                  // shell centre over the board's top face
// microSD: Molex 1040310811, push-pull, 1.42 tall, and its mouth sits sd_mouth
// BEHIND the board's edge. HAZARD: the card reaches only sd_card_out past that
// mouth, which is less than the wall is thick — so however the board is placed
// the card stops short of the outer face. The finger pocket below is not
// cosmetic, it is the only thing that makes the card removable. Asserted.
sd_cage_h = 1.42;  sd_mouth = 1.92;  sd_card_out = 4.0;  sd_card_w = 11.0;
sd_cz     = sd_cage_h/2;              // card plane over the board's top face
sd_slot_w = 18;  sd_slot_h = 3.5;     // the wall opening — far wider and taller
                                      // than the card, so a nail can reach down
                                      // either side of it and pinch
port_fit = 1.0;                       // slack on the USB-C openings, 0.5 a side.
                                      // The shells stand INSIDE the openings, so
                                      // this is no longer only the plug's
                                      // clearance: it is how far the board may
                                      // sit off in X and Z before the wall bears
                                      // on a shell and works its joints. It
                                      // costs nothing to give — the opening
                                      // lives at the bottom of a 13 mm pocket
                                      // and is not seen from outside.
port_z   = [pcb_z + sd_cz, pcb_z + usbc_cz, pcb_z + usbc_cz];

// ---- pockets in the wall's outer face -------------------------------------
// One rule for all three: cut to port_pocket_d and leave 0.8 mm of floor, which
// is all a 2.4 mm wall has to give and every 0.1 mm left there is 0.1 not
// bought. At the two USB-C it buys insertion depth — the receptacle mouths sit
// inside the wall, so a cable's overmould would otherwise land on the outer face
// before the plug is home. At the µSD it is the finger recess that reaches the
// card.
port_pocket_d = wall - 0.8;                  // 1.6, floor 0.8
usbc_boot_w = 13.0;  usbc_boot_h = 7.6;  usbc_boot_r = 1.5;
sd_pocket_w = 26.0;  sd_pocket_h = 11.0; sd_pocket_r = 2.5;

// ---- power on/off switch (latching push button, inline in the battery feed) --
// A push-on / push-off (latching) button that makes/breaks the battery-side
// power feed — press once to power up, again to cut it, so the machine is
// genuinely OFF between sessions instead of idling on the LiPo. NOT wired to
// EN/GND (that would be a momentary reset): it sits inline on the power rail.
// Panel-mounts through the back wall, a loose part wired back to J2 on the
// board, with its lamp on J3.
// No reset/BOOT button is exposed — on the S3 both are recovery-only
// (auto-download handles flashing), so like the devkit's own USB-C they are
// reached by taking the baseplate off.
pwr_btn  = true;             // set false to omit the switch hole entirely
pwr_d    = 13.5;             // switch barrel Ø (the part Julien bought)
pwr_fit  = 0.4;              // panel-hole clearance on the barrel Ø, far tighter
                             // than the ports': this switch IS retained by the
                             // panel, its nut bearing on the wall, so it wants a
                             // close hole and not port_fit's slack. At Ø14.7 a
                             // coupon came out wider than pwr_body_d and nothing
                             // bore against the wall at all.
                             // OPEN: at 0.4 the hole is Ø13.9 and the bearing
                             // against pwr_body_d is only 0.05 a side — and that
                             // bearing is what retains the switch. Judge it on
                             // the coupon; if it is loose, this wants ~0.05.
pwr_r    = (pwr_d + pwr_fit) / 2;
pwr_body_d = 14;             // WIDEST thing behind the panel — nut across
                             // corners, body OD, solder lugs. NOT the barrel:
                             // this is what decides the clearances below.
                             // Measured off the part.
// Nothing stands behind the button, so its placement has only three rules: clear
// of the board assembly, clear of the back-right baseplate boss, and on the USB-C
// centreline — which is where the back elevation wants it. All three asserted.
// What it clears is the DEVKIT, not the PCB: the antenna end overhangs the
// board's right edge and is the rightmost thing in the cavity.
devkit_x1 = pcb_x0 + devkit_cu + devkit_w/2;   // 119.33
pwr_gap  = 6;                                  // air from there to the nut
pwr_x    = max(pcb_x1, devkit_x1) + pwr_gap + pwr_body_d/2;   // 132.3
pwr_z    = pcb_z + usbc_cz;

// ---- baseplate / chassis --------------------------------------------------
// Clearance so the plate drops into the shell, 0.25 a side. This is the one fit
// where BOTH faces are printed and both err inward, so whatever the process gives
// away it gives away twice here — the joint that goes tight first, and the reason
// MANUFACTURING.md calls it the check to make on every new filament.
bp_gap     = 0.5;
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
// ---- the four baseplate screws (#6-32 into body inserts) ------------------
// NOT MODELLED. The plate prints SOLID at the four post_xy and both features are
// DRILLED after the print. The printed lamage came out poor on the first baseplate:
// it is a flat-bottomed pocket in the FIRST layers, so the plate has to bridge from
// Ø7.4 back in to Ø3.9 over open air, and the seat the head pulls against ends up
// being whatever that bridge sagged to. A drill gives a clean flat seat instead —
// see README, "Drilling the baseplate", for transferring the positions.
// The three numbers below are the DRILLING SPEC — a bit cuts the size it is, so
// they owe nothing to the process. They stay in the model because the geometry
// that meets the drilled hole derives from them — post_bore_h budgets the insert against
// the plate thickness the screw crosses once the lamage exists, and the two grip
// asserts check that budget.
bp_screw_r = scr_clear_d/2;      // Ø3.9 shank clearance, drilled through (Ø4 bit)
bp_head_r  = (scr_head_d + 0.4)/2;  // Ø6.9 lamage for the Ø6.5 head, 0.4 of slop
bp_head_h  = scr_head_h + 0.2;   // 1.2 deep — 0.2 past the head so it can only ever
                     // end up BELOW the plate's underside, never level with it.
                     // With feet_mode="none" that face is the machine's contact
                     // patch: a proud head makes it rock on three of four points.
// The two FRONT feet sit over those holes, so they need a bore that clears the
// DRIVER, not just the screw: the head is recessed bp_head_h up inside the plate,
// and a glued-on foot with a shank-sized hole puts the head out of reach. One bore
// at the lamage Ø does both jobs (the v0 foot had a bore + its own head counterbore,
// which the lamage made redundant).
foot_bore_r = bp_head_r;
// Baseplate screw bosses. Rectangular pads FUSED INTO THE WALLS they sit against,
// not free-standing posts: the box overshoots the shell by post_out and the
// intersection with body_outer() trims it flush, so "touching the wall" is a
// property of the construction instead of a number that drifts when wall or
// corner_r moves. The v0 round posts stood 4.4 mm clear of every wall and 3.3 mm
// short of the deck — they hung off the bracket bosses, whose M2 pilots they
// plugged, and they overlapped the baseplate over their whole bottom 2.6 mm.
post_pad   = 5.0;  // material from the screw axis out to the boss's FREE faces.
                   // Was 4.5, which left 2.1 mm around the Ø4.8 insert bore — over
                   // ins_wall, but only just, on a joint that gets melted. The
                   // 0.5 comes out of the battery band (106.5 -> 106.0 for a 96 mm
                   // cell); an insert that bulges its boss costs a body reprint.
post_out   = 8;    // how far the box is driven THROUGH the wall before
                   // body_outer() trims it — any value past the wall works
post_bore   = ins_hole_d/2;                    // Ø4.8 insert bore
post_h     = 10;   // boss height above bp_t. The WALLS carry the boss, so height
                   // is only ever about clearing the bore — the load path is
                   // screw -> insert -> boss -> wall and never leaves that band.
// Insert goes in from BELOW, into the boss's bottom face at z=bp_t (the face the
// baseplate seats against), so the iron reaches it straight down through the open
// bottom of the shell before any board is fitted.
// over = the plate the thread crosses first: bp_t less the head's lamage. The
// lamage is drilled rather than printed, but it is there by the time a screw is
// turned, so the budget is the same.
post_bore_h = ins_bore_h(bp_t - bp_head_h);   // BLIND, ~3 mm of roof left under
                   // post_h. A through bore would let an over-long screw push its
                   // tip into the cavity, and at the front corners that lands in
                   // the bracket boss overhead.

// ---- colours (for the assembled render) -----------------------------------
C_body   = "#B6CEB4";
C_plate  = "#C9C3B2";
C_bracket= "#2B2B2B";
C_screen = "#F7F4EA";
C_foam   = "#8a8f94";

// ---- cutaway sections -----------------------------------------------------
plan_z       = 26;   // height of the horizontal "plan" cut — clears the board
                    // stack, so the whole cavity stays in the bottom half
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

// baseplate screw bosses: one at each of the four corners.
// One per corner, on the same 3 mm inset off the corner tangent all round. Each
// keeps ~4.9 mm of plate rim outboard of its Ø6.9 LAMAGE for the head to pull
// against — the rim also absorbs the wander of a lamage that is drilled by hand.
// CONTRACT: no boss may stand under the board. They run to post_h above bp_t,
// i.e. 3 mm past the board's underside, so the back pair is what sets how far
// left the board can go (asserted at pcb_x0).
post_xy = [[corner_r+3,   corner_r+3],     // front-left
           [W-corner_r-3, corner_r+3],     // front-right
           [corner_r+3,   D-corner_r-3],   // back-left
           [W-corner_r-3, D-corner_r-3]];  // back-right
// Boss footprints [x0, x1, y0, y1]. A face driven past the shell is a FUSED face:
// every box runs out through both of the corner walls it sits in. Their free
// faces sit post_pad from the screw axis.
post_box = [[-post_out,              post_xy[0][0]+post_pad,
             -post_out,              post_xy[0][1]+post_pad],
            [post_xy[1][0]-post_pad, W+post_out,
             -post_out,              post_xy[1][1]+post_pad],
            [-post_out,              post_xy[2][0]+post_pad,
             post_xy[2][1]-post_pad, D+post_out],
            [post_xy[3][0]-post_pad, W+post_out,
             post_xy[3][1]-post_pad, D+post_out]];
// Foot centres — concentric with the four screw posts on purpose: the screw then
// lands dead centre in the disc, so the driver bore keeps 3.3 mm of wall all
// round. Offset even 3 mm and that wall drops under 1 mm and will not print.
// [x, y, takes a screw?]
foot_pos = [for (p = post_xy) [p[0], p[1], true]];
// one foot, sitting on the ground plane (ground face at z=0, top face at foot_h)
module foot(screwed) {
    difference() {
        cylinder(h=foot_h, r=foot_r);
        if (screwed)
            translate([0,0,-1]) cylinder(h=foot_h+2, r=foot_bore_r);
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
module screw_inserts() {
    for (p = post_xy)
        translate([p[0], p[1], bp_t-1])
            cylinder(h=post_bore_h + 1, r=post_bore);   // run 1 mm past the face
}

// 4 bosses just OUTSIDE the glass pocket for the retaining bracket (heat-set
// insert — see the fastener block).
// CONTRACT: the boss's free end IS the bracket's seating face. The screw pulls the
// bracket onto it and stops there, so glass clamp and foam squash both follow from
// br_seat instead of from screwdriver feel. A boss that reaches PAST the bracket
// seats nothing — the v0 length (…+ bracket_t + 6) drove a Ø6.8 column straight
// through the bracket's Ø3.4 screw hole, 277 mm³ of interference.
br_seat     = lip_t + G_t + foam_c;   // seat depth below the deck's outer face
br_boss_len = br_seat - lip_t;        // the column itself: pocket floor -> seat
// DEPTH BUDGET for the bracket insert, and the reason lip_t and bracket_t both
// grew. Everything between the boss's seat and the deck's OUTER face — the face
// the user looks at — is br_seat, and the insert's bore eats into it from below:
//     br_seat 6.9  =  bore 4.8  +  skin 2.1
// The bore has to cover the datasheet's 4.8 mm minimum AND a screw tip: 8 mm of
// budgeted thread less the 3.6 it spends crossing the bracket leaves 4.4 arriving,
// so here the datasheet floor is the binding one and the tip has 1 mm of room past
// the insert. At v0's lip_t 1.4 / bracket_t 2.6 the same sum came to 5.4 against a
// br_seat of 5.9 — 0.5 mm of skin on the machine's showpiece face, with the screw
// tip arriving under it and the iron melting brass into it. There is no way to buy
// that skin back except from lip_t (visible: a deeper bezel well) or bracket_t
// (invisible), so it came half from each, and the glass ends up 1 mm deeper in a
// stiffer clamp.
br_bore_h   = ins_bore_h(bracket_t);
pilot_skin  = br_seat - br_bore_h;   // deck left over the BLIND bore. Asserted
                     // against ins_wall below: it is the one skin on the part
                     // where a failure is both structural AND cosmetic.

// ---- fastener sanity checks ------------------------------------------------
// Every insert joint in the model, checked against the datasheet and against the
// screw. These are here because the numbers they guard are spread across three
// sections and interact: lip_t and bracket_t set the deck skin, post_pad and
// boss_r set the walls, and a plausible-looking edit to any of them silently
// buys a stripped joint or a hole through the deck. Failing the render is cheap;
// finding out after a 10-hour print is not.
assert(boss_r  - boss_bore  >= ins_wall,  "bracket boss: too little wall for the insert");
assert(post_pad - post_bore >= ins_wall,  "baseplate boss: too little wall for the insert");
assert(pilot_skin >= ins_wall,            "deck skin over the bracket insert too thin");
assert(post_h - post_bore_h >= 1.5,       "baseplate boss: no roof left over the bore");
assert(bp_t - bp_head_h     >= 1.2,       "baseplate: drilled lamage leaves too little plate");
assert(ins_grip(bracket_t)        >= 2.5, "bracket screw: not enough thread in the insert");
assert(ins_grip(bp_t - bp_head_h) >= 2.5, "baseplate screw: not enough thread in the insert");
// the bracket has to cover the boss it seats on, and the boss has to stay out of
// the glass pocket — both got tighter when boss_r grew for the insert
assert(P_w/2 + br_ml >= -boss_x_l + boss_r, "bracket arm no longer covers the left boss");
assert(boss_y - boss_r >= P_h/2,            "bracket boss has grown into the glass pocket");
// The board's own joints. The screw crosses the PCB and everything left of its
// thread has to land in brass, and the bore under it has to stop on the plate
// rather than open into it — held here because standoff_h and pcb_t are set 200
// lines apart and either alone renders clean and fails in plastic.
assert(standoff_r - standoff_bore >= ins_wall,
       "standoff: too little wall for the insert");
assert(standoff_h + bp_t - standoff_bore_h >= 1.5,
       "standoff: no floor left under the bore");
assert(ins_grip(pcb_t) >= 2.5, "board screw: not enough thread in the insert");
// The board's FRONT edge against the deck underside above it: the whole stack
// stands at the back of the wedge, where the ceiling is highest, so this is slack
// rather than a squeeze — but it is the check that fails if the heights are ever
// dropped or the devkit stack grows.
pcb_ceiling = Hf + (pcb_y0 - corner_r)*tan(theta) - top_wall;
assert(pcb_ceiling - (pcb_z + pcb_h) >= 2.5,
       "board front edge: not enough ceiling over the devkit stack");
// ...and the back-LEFT screw boss against the board's left edge. This is what
// sets pcb_x0, and pcb_x0 is what puts J4 within reach of the deck slot, so the
// two are worth seeing together.
assert(pcb_x0 - (post_xy[2][0] + post_pad) >= 1,
       "the back-left baseplate boss has grown under the board");
// The devkit's antenna end, against the right wall it overhangs toward.
assert(W - wall - devkit_x1 >= 2, "the devkit has backed into the right wall");
// The cell's X band, boxed by the flex plenum on one side and the front-right
// screw boss on the other. The boss one is the load-bearing check: it is a rigid
// printed corner at the cell's own height, and a pouch cell must never be asked
// to take it. Caught here because bat_x0 is set 300 lines from post_xy and a
// plausible edit to either closes the gap silently.
assert(post_xy[1][0] - post_pad - bat_x1 >= 2,
       "battery: the cell runs into the front-right screw boss");
assert(bat_y0 + bat_d <= pcb_y0 - 8,
       "battery: the cell has closed the corridor in front of the board");
// The I/O block. Its X comes straight off the PCB file, so what is worth holding
// is not the pitches — they cannot drift — but that the openings stay on the flat
// part of the back face and keep a wall between them.
wall_x1 = W - wall;                                 // shell inner right face -> 173.6
port_env = [sd_pocket_w/2, usbc_boot_w/2, usbc_boot_w/2];   // outer-face pockets
assert(port_x[0] - port_env[0] >= corner_r &&
       port_x[2] + port_env[2] <= W - corner_r,
       "I/O block: an opening has run off the flat part of the back face");
port_web = min([for (i = [0:1]) (port_x[i+1] - port_env[i+1])
                               - (port_x[i] + port_env[i])]);
assert(port_web >= 1.0,
       "I/O block: an outer-face pocket leaves under 1 mm of wall to its neighbour");
// The µSD card against the pocket that has to reach it. The card stops short of
// the wall's outer face by construction (see sd_mouth), so the pocket floor is
// what a finger arrives at: if the card does not at least reach that floor there
// is nothing to pull on and the slot is one-way.
sd_card_y = pcb_y1 - sd_mouth + sd_card_out;        // where the card ends -> 102.88
assert(sd_card_y >= D - port_pocket_d,
       "microSD: the card no longer reaches the floor of its finger pocket");
assert(sd_slot_w - sd_card_w >= 5 && sd_slot_h > sd_cage_h + 1,
       "microSD: the opening leaves no room for a nail beside the card");
// The two USB-C mouths have to end up INSIDE the wall opening, not behind the
// wall: what the plug loses is the gap between the pocket floor and the mouth.
assert(pcb_y1 + usbc_proud >= D - wall,
       "USB-C: the shell mouths sit behind the wall's inner face");
// The button. Nothing stands behind it, so these are the only three rules it has.
assert(pwr_x - pwr_body_d/2 >= max(pcb_x1, devkit_x1) + 2,
       "power switch: the nut has moved over the board or the devkit");
assert(post_xy[3][0] - post_pad - (pwr_x + pwr_body_d/2) >= 2,
       "power switch: the nut runs into the back-right screw boss");
assert(pwr_z - pwr_body_d/2 >= bp_t + 2 && pwr_x + pwr_body_d/2 <= W - corner_r,
       "power switch: the nut has no flat seat left, or fouls the baseplate");
module bracket_cols(r, z0, h) {
    on_deck() for (p = boss_xy)
        translate([glass_dx + p[0], screen_cy + glass_dy + p[1], z0])
            cylinder(h=h, r=r);
}
module bracket_bosses() { bracket_cols(boss_r, -br_seat, br_boss_len); }
// Insert bore, run 1 mm past the seat so the cut is clean. The insert goes in from
// the SEAT side, i.e. from inside the case, iron pointing at the deck — and this
// seat is the face the bracket clamps against, so stop the press flush with it
// (see the hazard in the fastener block).
module bracket_inserts() {
    bracket_cols(boss_bore, -br_seat-1, br_bore_h + 1);
}

// deck cuts: through-aperture, glass pocket (leaves the front lip), FPC slot
// HAZARD: these three prisms are driven down the DECK NORMAL, and the deck is
// reclined — so every millimetre of depth also carries them sin(theta) = 0.36 mm
// toward the BACK. They only ever have to pierce top_wall, so they are bounded:
// run them to the floor instead and the pocket's back edge lands at y 92 / z 0
// and takes a wedge out of both back screw bosses, on the very face the
// baseplate seats against. Depth is measured from the deck's OUTER face.
deck_cut_d = 20;   // >> top_wall by a wide margin, and still stops the sweep
                   // ~25 mm above the cavity floor

module screen_cuts() {
    on_deck() translate([0, screen_cy, 0]) {
        // window — always on the ACTIVE area, wherever the glass has been put.
        // glass_dx cancels active_off_x, so this lands on the deck centre; keep
        // the expression rather than hardcoding 0, or the window silently stops
        // tracking the active area and rides onto the pixels.
        translate([glass_dx + active_off_x, glass_dy + active_off_y, -deck_cut_d])
            linear_extrude(deck_cut_d + 3)
                square([A_ap_w, A_ap_h], center=true);
        // glass pocket behind the lip — shifted so the ACTIVE area lands centred.
        // Its top face IS the lip's underside: it must never start above -lip_t.
        translate([glass_dx, glass_dy, -lip_t - deck_cut_d])
            linear_extrude(deck_cut_d) square([P_w, P_h], center=true);
        // FPC clearance: an internal notch in the LEFT recess wall, kept BELOW
        // the bezel lip so it stays invisible from outside — the flex passes the
        // glass's left edge and folds back into the cavity, to the board
        translate([glass_dx-P_w/2, glass_dy, -lip_t - deck_cut_d])
            linear_extrude(deck_cut_d) square([fpc_slot_x, fpc_w], center=true);
    }
}

module port_cuts() {
    // USB-C (charge, keyboard) + microSD through the BACK wall (y = D). The two
    // USB-C openings clear the SHELL, which passes into them; the µSD opening
    // clears the CARD, which is all that ever reaches the wall.
    for (i=[0:2]) {
        pw = (i==0) ? sd_slot_w : usbc_w + port_fit;
        ph = (i==0) ? sd_slot_h : usbc_h + port_fit;
        translate([port_x[i], D-wall-1, port_z[i]])
            rotate([-90,0,0]) linear_extrude(wall+2)
                offset(r=0.8) square([pw-1.6, ph-1.6], center=true);
    }
    // and the pockets they open into, in the outer face
    for (i=[1:2]) port_pocket(i, usbc_boot_w, usbc_boot_h, usbc_boot_r);
    port_pocket(0, sd_pocket_w, sd_pocket_h, sd_pocket_r);
}

module port_pocket(i, w, h, r) {
    translate([port_x[i], D-port_pocket_d, port_z[i]])
        rotate([-90,0,0]) linear_extrude(port_pocket_d+1)
            rrect(w, h, r);
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
    if (name_on)
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
        // CONTRACT: every bore is cut AFTER the bosses are unioned. Cut one
        // inside its own boss and a neighbour that grows into it fills it back
        // in silently — which is what the v0 corner posts did to both front
        // bracket pilots, blinding them 6 mm into a 12 mm boss.
        screw_inserts();
        bracket_inserts();
        screen_cuts();
        port_cuts();
        power_cut();
        nameplate();                 // engrave — no-op while name_on is false
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
    // P_* coupled the frame to glass_gap, so any growth in pocket slack drove the
    // arm straight into the left wall (0.89 mm was the whole margin there).
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
// The only feature the plate does not print is the four body screws at post_xy:
// their through hole and lamage are DRILLED after the print, because a printed
// lamage is a flat-bottomed pocket in the FIRST layers and comes out as whatever
// the bridge under it sagged to. Reasons at bp_screw_r/bp_head_r, procedure in
// README, "Drilling the baseplate". Everything else — the standoff bores
// included — is in the print: a Ø4.8 bore is wide enough for the printer to hold
// it, which is exactly what a Ø1.6 pilot was not.
module baseplate() {
    iw = W - 2*wall - bp_gap;
    id = D - 2*wall - bp_gap;
    union() {
        // plate (centred on the footprint)
        translate([W/2, D/2, 0]) linear_extrude(bp_t) rrect(iw, id, corner_r-wall);
        // round feet underneath — only in "fused" mode, see feet_mode
        if (feet_mode == "fused") feet_parts();
        // the board's four standoffs, bored for their inserts
        difference() {
            for (h = pcb_holes)
                translate([h[0], h[1], bp_t]) cylinder(h=standoff_h, r=standoff_r);
            for (h = pcb_holes)
                translate([h[0], h[1], bp_t + standoff_h - standoff_bore_h])
                    cylinder(h=standoff_bore_h + 1, r=standoff_bore);
        }
        // battery cage (front LiPo; foam/VHB tape does the rest). Wall at the
        // cell's LEFT end — the stop it slides onto — and two nibs at bat_nib_x that
        // only hold Y. The cell's right end is free: it overhangs the nibs.
        translate([bat_x0 - bat_wall_t, bat_y0 - 1, bp_t])
            cube([bat_wall_t, bat_d + 2, bat_nib_h]);
        for (cy = [bat_y0-1, bat_y0+bat_d+1])
            translate([bat_nib_x, cy, bp_t]) cylinder(h=bat_nib_h, r=1.6);
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
io_x0 = port_x[0] - port_env[0] - 6;          // just left of the charge pocket
io_x1 = pwr_x + pwr_body_d/2 + 6;             // just right of the button
io_z0 = bp_t;                                 // floor level
io_z1 = max(pwr_z + pwr_r, port_z[0] + usbc_boot_h/2) + 6;

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
// the mainboard on its standoffs, with the devkit floating over its right half
module ghost_pcb() {
    translate([pcb_x0 + pcb_w/2, pcb_y0 + pcb_d/2, bp_t + standoff_h])
        color("#2f6f4f") linear_extrude(pcb_t) rrect(pcb_w, pcb_d, pcb_r);
    translate([pcb_x0 + devkit_cu, pcb_y0 + devkit_cy, pcb_z + hdr_mate])
        %linear_extrude(devkit_t + devkit_top)
            square([devkit_w, devkit_d], center=true);
}
module ghost_boards() {
    ghost_battery();
    ghost_pcb();
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
