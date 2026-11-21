//! The touch gestures, driven headless with playfield positions and timestamps the way
//! `input::Input` replays them each frame.

use kitty_mobile::input::touch::{Gestures, Phase, Recogniser, Thresholds, MOUSE_ID};
use kitty_mobile::input::{Controls, Pending, Raw, Recorder};
use macroquad::math::{vec2, Vec2};
use macroquad::miniquad::{EventHandler, MouseButton};

/// 10 playfield units per mm: dead zone 15, full speed 150, swipe 70.
fn recogniser() -> Recogniser {
    Recogniser::new(Thresholds::new(10.0))
}

const FRAME: f64 = 1.0 / 60.0;
/// Somewhere on the right half (the action finger) and the left half (the slider).
const RIGHT: Vec2 = Vec2::from_array([450.0, 800.0]);
const LEFT: Vec2 = Vec2::from_array([150.0, 900.0]);

fn nothing(g: &Gestures) -> bool {
    !g.press && !g.leap && !g.drop && !g.pause && !g.any_tap
}

#[test]
fn thresholds_scale_with_density_within_bounds() {
    let t = Thresholds::new(10.0);
    assert_eq!((t.dead_zone, t.full_speed, t.swipe), (15.0, 150.0, 70.0));
    // A huge or broken density lands on the bounds instead of making gestures impossible
    let big = Thresholds::new(100.0);
    let tiny = Thresholds::new(0.0);
    let nan = Thresholds::new(f32::NAN);
    assert!(big.full_speed <= 180.0 && big.swipe <= 110.0 && big.dead_zone <= 24.0);
    assert!(tiny.full_speed >= 50.0 && tiny.swipe >= 28.0 && tiny.dead_zone >= 4.0);
    assert_eq!(nan, tiny);
    assert!(Thresholds::new(f32::INFINITY) == big);
}

#[test]
fn a_tap_whose_down_and_up_arrive_together_still_presses_and_leaps() {
    let mut r = recogniser();
    r.event(3, Phase::Down, RIGHT, 0.0);
    r.event(3, Phase::Up, RIGHT, 0.0);
    let g = r.take();
    assert!(g.press && g.leap && g.any_tap, "{g:?}");
    assert!(!g.charge_held && !g.drop);
    assert!(nothing(&r.take()));
}

#[test]
fn a_down_followed_by_a_move_in_the_same_frame_still_presses() {
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.event(0, Phase::Move, RIGHT + vec2(3.0, 2.0), 0.0);
    let g = r.take();
    assert!(g.press && g.charge_held && !g.leap, "{g:?}");
}

#[test]
fn holding_charges_and_releasing_leaps_once() {
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    let g = r.take();
    assert!(g.press && g.charge_held && !g.leap);
    for frame in 1..30 {
        r.event(0, Phase::Move, RIGHT + vec2(0.0, frame as f32 * 0.2), frame as f64 * FRAME);
        let g = r.take();
        assert!(!g.press && g.charge_held && !g.leap && !g.drop, "frame {frame}: {g:?}");
    }
    r.event(0, Phase::Up, RIGHT, 0.5);
    let g = r.take();
    assert!(g.leap && !g.charge_held && !g.press && !g.drop);
    let g = r.take();
    assert!(nothing(&g) && !g.charge_held);
}

#[test]
fn a_flick_down_drops_once_and_never_leaps() {
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    assert!(r.take().press);
    r.event(0, Phase::Move, RIGHT + vec2(2.0, 30.0), FRAME);
    assert!(!r.take().drop, "30 units isn't a swipe yet");
    r.event(0, Phase::Move, RIGHT + vec2(4.0, 75.0), 2.0 * FRAME);
    let g = r.take();
    assert!(g.drop && !g.charge_held && !g.leap, "{g:?}");
    // Carrying on down doesn't drop again, and lifting doesn't leap
    r.event(0, Phase::Move, RIGHT + vec2(4.0, 160.0), 3.0 * FRAME);
    r.event(0, Phase::Up, RIGHT + vec2(4.0, 200.0), 4.0 * FRAME);
    let g = r.take();
    assert!(nothing(&g) && !g.charge_held, "{g:?}");
}

#[test]
fn a_flick_that_arrives_in_one_batch_drops_instead_of_leaping() {
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.event(0, Phase::Up, RIGHT + vec2(0.0, 90.0), 0.0);
    let g = r.take();
    assert!(g.press && g.drop && !g.leap, "{g:?}");
}

#[test]
fn a_flick_after_holding_still_drops() {
    // No moves while the finger rests, then a quick flick: the resting place still counts as
    // where the swipe started
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.event(0, Phase::Move, RIGHT + vec2(0.0, 80.0), 1.0);
    let g = r.take();
    assert!(g.drop, "{g:?}");
}

#[test]
fn a_flick_split_across_slow_frames_still_drops() {
    // Events only carry the time of the frame that replays them. With frames 0.38 s apart (a
    // hitch, or a slow browser) a quick flick lands in three frames
    let mut r = recogniser();
    let frame = |r: &mut Recogniser, n: usize, y: f32, phase: Phase| {
        let now = n as f64 * 0.38;
        r.frame(now);
        r.event(0, phase, RIGHT + vec2(0.0, y), now);
        r.take()
    };
    assert!(frame(&mut r, 0, 0.0, Phase::Down).press);
    assert!(!frame(&mut r, 1, 37.0, Phase::Move).drop);
    let g = frame(&mut r, 2, 74.0, Phase::Move);
    assert!(g.drop && !g.charge_held, "{g:?}");
    assert!(!frame(&mut r, 3, 110.0, Phase::Up).leap);
}

#[test]
fn a_slow_drag_at_60_frames_a_second_never_drops() {
    // The same as the slow drag below, with the frame bookkeeping Input does
    let mut r = recogniser();
    for n in 0..=60 {
        let now = n as f64 * FRAME;
        r.frame(now);
        let phase = if n == 0 { Phase::Down } else { Phase::Move };
        r.event(0, phase, RIGHT + vec2(0.0, n as f32 * 2.0), now);
        assert!(!r.take().drop, "frame {n}");
    }
}

#[test]
fn slow_or_sideways_drags_never_drop() {
    // 120 units down, but over a second
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    for frame in 1..=60 {
        r.event(0, Phase::Move, RIGHT + vec2(0.0, frame as f32 * 2.0), frame as f64 * FRAME);
    }
    let g = r.take();
    assert!(!g.drop && g.charge_held);
    r.event(0, Phase::Up, RIGHT + vec2(0.0, 120.0), 61.0 * FRAME);
    assert!(r.take().leap, "a slow drag is still a release");

    // Fast, but more sideways than down
    let mut r = recogniser();
    r.event(1, Phase::Down, RIGHT, 0.0);
    r.event(1, Phase::Move, RIGHT + vec2(60.0, 40.0), FRAME);
    r.event(1, Phase::Move, RIGHT + vec2(110.0, 80.0), 2.0 * FRAME);
    r.event(1, Phase::Up, RIGHT + vec2(110.0, 80.0), 3.0 * FRAME);
    let g = r.take();
    assert!(!g.drop && g.leap, "{g:?}");

    // Fast and straight up
    let mut r = recogniser();
    r.event(2, Phase::Down, RIGHT, 0.0);
    r.event(2, Phase::Move, RIGHT - vec2(0.0, 120.0), FRAME);
    assert!(!r.take().drop);
}

#[test]
fn the_slider_has_a_dead_zone_and_reaches_full_speed() {
    let mut r = recogniser();
    r.event(0, Phase::Down, LEFT, 0.0);
    let g = r.take();
    assert_eq!(g.move_x, 0.0);
    assert!(!g.press && !g.charge_held && g.any_tap, "the slider never jumps");
    r.event(0, Phase::Move, LEFT + vec2(14.0, 0.0), FRAME);
    assert_eq!(r.take().move_x, 0.0, "inside the dead zone");
    r.event(0, Phase::Move, LEFT + vec2(16.0, 0.0), 2.0 * FRAME);
    let slow = r.take().move_x;
    assert!((0.2..0.25).contains(&slow), "just past the dead zone: {slow}");
    r.event(0, Phase::Move, LEFT + vec2(80.0, 0.0), 3.0 * FRAME);
    let mid = r.take().move_x;
    assert!(mid > slow && mid < 1.0, "{mid}");
    r.event(0, Phase::Move, LEFT + vec2(145.0, 0.0), 4.0 * FRAME);
    assert_eq!(r.take().move_x, 1.0, "close to full snaps to full");
    r.event(0, Phase::Move, LEFT + vec2(-150.0, 30.0), 5.0 * FRAME);
    assert_eq!(r.take().move_x, -1.0);
    // Up and down don't matter
    r.event(0, Phase::Move, LEFT + vec2(0.0, -200.0), 6.0 * FRAME);
    assert_eq!(r.take().move_x, 0.0);
}

#[test]
fn the_slider_centre_trails_a_thumb_that_goes_too_far() {
    let mut r = recogniser();
    r.event(0, Phase::Down, LEFT, 0.0);
    // Well past full speed (the slider keeps the finger even across the middle)
    r.event(0, Phase::Move, LEFT + vec2(450.0, 0.0), FRAME);
    assert_eq!(r.take().move_x, 1.0);
    let slider = r.slider().unwrap();
    assert_eq!(slider.finger.x - slider.origin.x, 180.0, "the centre trails at 1.2x full speed");
    // So coming back 180 stops her, instead of having to come back 450
    r.event(0, Phase::Move, LEFT + vec2(270.0, 0.0), 2.0 * FRAME);
    assert_eq!(r.take().move_x, 0.0);
    r.event(0, Phase::Move, LEFT + vec2(120.0, 0.0), 3.0 * FRAME);
    assert_eq!(r.take().move_x, -1.0);
}

#[test]
fn the_newest_thumb_takes_over_the_slider() {
    let mut r = recogniser();
    r.event(0, Phase::Down, LEFT, 0.0);
    r.event(0, Phase::Move, LEFT + vec2(150.0, 0.0), FRAME);
    assert_eq!(r.take().move_x, 1.0);
    // A second thumb lands: it's the slider now, centred where it landed
    let other = vec2(60.0, 1000.0);
    r.event(1, Phase::Down, other, 2.0 * FRAME);
    assert_eq!(r.take().move_x, 0.0);
    r.event(1, Phase::Move, other - vec2(150.0, 0.0), 3.0 * FRAME);
    assert_eq!(r.take().move_x, -1.0);
    // The first thumb no longer does anything, even when it lifts
    r.event(0, Phase::Move, LEFT + vec2(300.0, 0.0), 4.0 * FRAME);
    r.event(0, Phase::Up, LEFT + vec2(300.0, 0.0), 5.0 * FRAME);
    let g = r.take();
    assert_eq!(g.move_x, -1.0);
    assert!(!g.leap);
    r.event(1, Phase::Up, other, 6.0 * FRAME);
    assert_eq!(r.take().move_x, 0.0);
}

#[test]
fn lifting_or_cancelling_the_slider_stops() {
    for phase in [Phase::Up, Phase::Cancel] {
        let mut r = recogniser();
        r.event(0, Phase::Down, LEFT, 0.0);
        r.event(0, Phase::Move, LEFT + vec2(150.0, 0.0), FRAME);
        assert_eq!(r.take().move_x, 1.0);
        r.event(0, phase, LEFT + vec2(150.0, 0.0), 2.0 * FRAME);
        let g = r.take();
        assert_eq!(g.move_x, 0.0);
        assert!(r.slider().is_none());
        assert!(!g.leap);
    }
}

#[test]
fn the_pause_corner_pauses_and_never_jumps() {
    // On the button, and out in the letterbox beyond the corner
    for at in [vec2(572.0, 92.0), vec2(545.0, 125.0), vec2(640.0, -30.0)] {
        let mut r = recogniser();
        r.event(0, Phase::Down, at, 0.0);
        let g = r.take();
        assert!(g.pause && !g.press && !g.charge_held, "{at}: {g:?}");
        r.event(0, Phase::Move, at + vec2(-300.0, 400.0), FRAME);
        r.event(0, Phase::Up, at + vec2(-300.0, 400.0), 2.0 * FRAME);
        let g = r.take();
        assert!(nothing(&g) && g.move_x == 0.0, "{at}: {g:?}");
    }
    // Just below the corner, and the bottom-right letterbox, are action fingers
    for at in [vec2(572.0, 140.0), vec2(650.0, 1250.0)] {
        let mut r = recogniser();
        r.event(0, Phase::Down, at, 0.0);
        let g = r.take();
        assert!(g.press && !g.pause, "{at}: {g:?}");
    }
}

#[test]
fn fingers_down_when_the_screen_changes_do_nothing_until_lifted() {
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.event(1, Phase::Down, LEFT, 0.0);
    r.event(1, Phase::Move, LEFT + vec2(150.0, 0.0), FRAME);
    let g = r.take();
    assert!(g.press && g.charge_held && g.move_x == 1.0);
    // The press dismissed a menu; play starts with both fingers still down
    r.ignore_current();
    let g = r.take();
    assert!(nothing(&g) && !g.charge_held && g.move_x == 0.0, "{g:?}");
    r.event(1, Phase::Move, LEFT + vec2(-150.0, 0.0), 2.0 * FRAME);
    r.event(0, Phase::Move, RIGHT + vec2(0.0, 120.0), 2.0 * FRAME);
    r.event(0, Phase::Up, RIGHT + vec2(0.0, 120.0), 3.0 * FRAME);
    let g = r.take();
    assert!(nothing(&g) && !g.charge_held && g.move_x == 0.0, "{g:?}");
    assert!(r.slider().is_none());
    // New fingers work as usual
    r.event(2, Phase::Down, RIGHT, 4.0 * FRAME);
    let g = r.take();
    assert!(g.press && g.charge_held);
    // Edges not yet taken when the screen changes are dropped too
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.event(0, Phase::Up, RIGHT, 0.0);
    r.ignore_current();
    assert!(nothing(&r.take()));
}

#[test]
fn a_cancelled_touch_or_the_app_going_away_releases_without_leaping() {
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.take();
    r.event(0, Phase::Cancel, RIGHT, FRAME);
    let g = r.take();
    assert!(!g.leap && !g.charge_held);

    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.event(1, Phase::Down, LEFT, 0.0);
    r.event(1, Phase::Move, LEFT + vec2(150.0, 0.0), FRAME);
    r.take();
    r.release_all();
    let g = r.take();
    assert!(!g.leap && !g.charge_held && g.move_x == 0.0, "{g:?}");
    // Their lifts, if they ever come, are ignored
    r.event(0, Phase::Up, RIGHT, 1.0);
    r.event(1, Phase::Up, LEFT, 1.0);
    assert!(nothing(&r.take()));
}

#[test]
fn a_lost_lift_doesnt_leave_a_finger_stuck() {
    // The same id landing again means its lift went missing: the new touch counts, the old one
    // doesn't leap
    let mut r = recogniser();
    r.event(5, Phase::Down, RIGHT, 0.0);
    r.take();
    r.event(5, Phase::Down, RIGHT + vec2(20.0, 0.0), 1.0);
    let g = r.take();
    assert!(g.press && g.charge_held && !g.leap);
    r.event(5, Phase::Up, RIGHT, 1.2);
    let g = r.take();
    assert!(g.leap && !g.charge_held);
}

#[test]
fn two_thumbs_walk_and_jump_together() {
    let mut r = recogniser();
    r.event(0, Phase::Down, LEFT, 0.0);
    r.event(0, Phase::Move, LEFT + vec2(150.0, 0.0), FRAME);
    r.event(1, Phase::Down, RIGHT, FRAME);
    let g = r.take();
    assert!(g.move_x == 1.0 && g.press && g.charge_held, "{g:?}");
    // The action thumb flicks down while the slider keeps running
    r.event(1, Phase::Move, RIGHT + vec2(0.0, 90.0), 2.0 * FRAME);
    r.event(0, Phase::Move, LEFT + vec2(155.0, 4.0), 2.0 * FRAME);
    let g = r.take();
    assert!(g.move_x == 1.0 && g.drop && !g.charge_held, "{g:?}");
    r.event(1, Phase::Up, RIGHT + vec2(0.0, 90.0), 3.0 * FRAME);
    // A new jump while still running
    r.event(2, Phase::Down, RIGHT, 4.0 * FRAME);
    r.event(2, Phase::Up, RIGHT, 5.0 * FRAME);
    let g = r.take();
    assert!(g.move_x == 1.0 && g.press && g.leap && !g.drop, "{g:?}");
    // Lifting the slider thumb doesn't jump
    r.event(0, Phase::Up, LEFT, 6.0 * FRAME);
    let g = r.take();
    assert!(g.move_x == 0.0 && nothing(&g));
}

#[test]
fn fingers_keep_their_job_across_the_middle() {
    let mut r = recogniser();
    // An action finger wandering left neither walks nor stops counting as a release
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.event(0, Phase::Move, vec2(80.0, 800.0), 0.5);
    let g = r.take();
    assert!(g.move_x == 0.0 && g.charge_held);
    r.event(0, Phase::Up, vec2(80.0, 800.0), 0.6);
    assert!(r.take().leap);
}

#[test]
fn a_second_action_finger_takes_over_without_a_stray_leap() {
    let mut r = recogniser();
    r.event(0, Phase::Down, RIGHT, 0.0);
    r.take();
    r.event(1, Phase::Down, RIGHT + vec2(60.0, 0.0), 0.2);
    let g = r.take();
    assert!(g.press && g.charge_held);
    r.event(0, Phase::Up, RIGHT, 0.3);
    let g = r.take();
    assert!(!g.leap && g.charge_held, "the first finger was superseded: {g:?}");
    r.event(1, Phase::Up, RIGHT, 0.4);
    assert!(r.take().leap);
}

#[test]
fn the_mouse_plays_a_finger() {
    let mut recorder = Recorder::default();
    recorder.mouse_motion_event(100.0, 100.0); // hovering does nothing
    recorder.mouse_button_down_event(MouseButton::Right, 450.0, 800.0); // nor other buttons
    recorder.mouse_button_down_event(MouseButton::Left, LEFT.x, LEFT.y);
    recorder.mouse_motion_event(LEFT.x + 150.0, LEFT.y);
    let mut r = recogniser();
    let replay = |recorder: &mut Recorder, r: &mut Recogniser, time: f64| {
        for event in recorder.events.drain(..) {
            if let Raw::Touch { id, phase, x, y } = event {
                assert_eq!(id, MOUSE_ID);
                r.event(id, phase, vec2(x, y), time);
            }
        }
        r.take()
    };
    assert_eq!(replay(&mut recorder, &mut r, 0.0).move_x, 1.0);
    recorder.mouse_button_up_event(MouseButton::Left, LEFT.x + 150.0, LEFT.y);
    recorder.mouse_motion_event(LEFT.x, LEFT.y);
    assert_eq!(replay(&mut recorder, &mut r, FRAME).move_x, 0.0);
    // A click on the right half is a quick tap
    recorder.mouse_button_down_event(MouseButton::Left, RIGHT.x, RIGHT.y);
    recorder.mouse_button_up_event(MouseButton::Left, RIGHT.x, RIGHT.y);
    let g = replay(&mut recorder, &mut r, 2.0 * FRAME);
    assert!(g.press && g.leap && g.any_tap);
    assert!(!recorder.touch_seen, "the mouse isn't a touch screen");
    // Minimizing while the button is down: the release that never comes isn't waited for
    recorder.mouse_button_down_event(MouseButton::Left, RIGHT.x, RIGHT.y);
    recorder.window_minimized_event();
    recorder.mouse_button_up_event(MouseButton::Left, RIGHT.x, RIGHT.y);
    assert_eq!(recorder.events.last(), Some(&Raw::Minimized));
}

#[test]
fn edges_wait_for_the_next_physics_step() {
    let mut pending = Pending::default();
    // A quick tap in a frame that runs no step
    let tap = Controls { press: true, leap: true, any_tap: true, ..Controls::default() };
    pending.add(&tap);
    // The next frame: nothing new, but the step sees the tap
    let now = Controls { move_x: 0.5, ..Controls::default() };
    pending.add(&now);
    let step = pending.take(&now);
    assert!(step.press && step.leap && !step.charge_held && step.move_x == 0.5);
    // A second step in the same frame doesn't see it again
    let step = pending.take(&now);
    assert!(!step.press && !step.leap && !step.drop);
    // Held state comes from now, edges from whenever they happened
    pending.add(&Controls { drop: true, ..Controls::default() });
    let held = Controls { charge_held: true, ..Controls::default() };
    let step = pending.take(&held);
    assert!(step.drop && step.charge_held && !step.press);
}
