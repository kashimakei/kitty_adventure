//! Menu gestures and buttons, driven headless with playfield positions and timestamps the way
//! `input::Input` replays them each frame.

use kitty_mobile::input::menu::{MenuInput, MenuKeys, MenuRecogniser, MenuThresholds, Nav, Tap};
use kitty_mobile::input::touch::{Phase, PAUSE_ZONE_X, PAUSE_ZONE_Y, MOUSE_ID};
use kitty_mobile::input::Recorder;
use kitty_mobile::ui;
use macroquad::math::{vec2, Rect, Vec2};
use macroquad::miniquad::{EventHandler, KeyCode, MouseButton};

/// 5 playfield units per mm: slop 20.
fn recogniser() -> MenuRecogniser {
    MenuRecogniser::new(MenuThresholds::new(5.0))
}

const FRAME: f64 = 1.0 / 60.0;
const AT: Vec2 = Vec2::from_array([300.0, 600.0]);

#[test]
fn slop_scales_with_density_within_bounds() {
    assert_eq!(MenuThresholds::new(5.0).slop, 20.0);
    assert_eq!(MenuThresholds::new(100.0).slop, 40.0);
    assert_eq!(MenuThresholds::new(0.0).slop, 12.0);
    assert_eq!(MenuThresholds::new(f32::NAN).slop, 12.0);
}

#[test]
fn a_tap_is_down_and_up_close_together() {
    let mut r = recogniser();
    r.event(1, Phase::Down, AT, 0.0);
    let held = r.take();
    assert_eq!((held.tap, held.held), (None, Some(AT)), "nothing until it lifts");
    r.event(1, Phase::Move, AT + vec2(6.0, 5.0), FRAME);
    r.event(1, Phase::Up, AT + vec2(8.0, 6.0), 2.0 * FRAME);
    let g = r.take();
    assert_eq!(g.tap, Some(Tap { down: AT, up: AT + vec2(8.0, 6.0) }));
    assert_eq!((g.drag_dy, g.fling, g.held), (0.0, None, None));
    assert_eq!(r.take(), MenuInput::default(), "a tap counts once");
    // Down and up in the same frame
    r.event(2, Phase::Down, AT, 3.0 * FRAME);
    r.event(2, Phase::Up, AT, 3.0 * FRAME);
    assert!(r.take().tap.is_some());
}

#[test]
fn a_drag_scrolls_and_never_taps() {
    let mut r = recogniser();
    r.event(1, Phase::Down, AT, 0.0);
    let mut dragged = 0.0;
    for k in 1..=10 {
        r.event(1, Phase::Move, AT - vec2(0.0, 12.0 * k as f32), k as f64 * FRAME);
        let g = r.take();
        assert_eq!(g.tap, None);
        dragged += g.drag_dy;
    }
    r.event(1, Phase::Up, AT - vec2(0.0, 132.0), 11.0 * FRAME);
    let g = r.take();
    dragged += g.drag_dy;
    assert_eq!(g.tap, None);
    assert!((dragged + 132.0).abs() < 1e-3, "the drags add up to the finger's travel: {dragged}");
    // Moving 12 units a frame, up: about 720 units a second
    let fling = g.fling.expect("let go while moving");
    assert!((fling + 720.0).abs() < 1.0, "{fling}");
}

#[test]
fn a_drag_that_stops_before_lifting_doesnt_fling() {
    let mut r = recogniser();
    r.event(1, Phase::Down, AT, 0.0);
    r.event(1, Phase::Move, AT - vec2(0.0, 100.0), FRAME);
    // Held still for a while, then lifted
    r.event(1, Phase::Move, AT - vec2(0.0, 100.0), 30.0 * FRAME);
    r.event(1, Phase::Up, AT - vec2(0.0, 100.0), 40.0 * FRAME);
    let g = r.take();
    assert!(g.fling.is_none_or(|v| v.abs() < 1.0), "{:?}", g.fling);
}

#[test]
fn a_finger_down_when_the_screen_changes_does_nothing() {
    let mut r = recogniser();
    r.event(1, Phase::Down, AT, 0.0);
    r.ignore_current();
    r.event(1, Phase::Up, AT, FRAME);
    assert_eq!(r.take(), MenuInput::default());
    // The next finger works
    r.event(2, Phase::Down, AT, 2.0 * FRAME);
    r.event(2, Phase::Up, AT, 3.0 * FRAME);
    assert!(r.take().tap.is_some());
}

#[test]
fn only_the_first_finger_counts() {
    let mut r = recogniser();
    r.event(1, Phase::Down, AT, 0.0);
    r.event(2, Phase::Down, AT + vec2(100.0, 0.0), FRAME);
    r.event(2, Phase::Up, AT + vec2(100.0, 0.0), 2.0 * FRAME);
    assert_eq!(r.take().tap, None);
    r.event(1, Phase::Up, AT, 3.0 * FRAME);
    assert_eq!(r.take().tap.map(|t| t.up), Some(AT));
}

#[test]
fn a_cancelled_finger_doesnt_tap() {
    let mut r = recogniser();
    r.event(1, Phase::Down, AT, 0.0);
    r.event(1, Phase::Cancel, AT, FRAME);
    assert_eq!(r.take(), MenuInput::default());
}

#[test]
fn tapped_needs_both_ends_inside() {
    let button = Rect::new(100.0, 100.0, 200.0, 100.0);
    let tap = |down: Vec2, up: Vec2| MenuInput { tap: Some(Tap { down, up }), ..MenuInput::default() };
    assert!(tap(vec2(150.0, 150.0), vec2(160.0, 155.0)).tapped(button));
    assert!(!tap(vec2(150.0, 150.0), vec2(350.0, 150.0)).tapped(button), "slid off");
    assert!(!tap(vec2(50.0, 150.0), vec2(150.0, 150.0)).tapped(button), "slid on");
    assert!(!MenuInput::default().tapped(button));
}

/// The mouse plays a finger for taps, and with no button down it only hovers: play never sees it.
#[test]
fn the_mouse_taps_hovers_and_scrolls() {
    let mut recorder = Recorder::default();
    recorder.mouse_motion_event(120.0, 340.0);
    recorder.mouse_wheel_event(0.0, 2.0);
    recorder.mouse_wheel_event(0.0, -0.5);
    assert!(recorder.events.is_empty(), "hovering and the wheel aren't play events");
    assert_eq!(recorder.hover, Some(vec2(120.0, 340.0)));
    assert_eq!(recorder.wheel, 1.5);
    recorder.mouse_button_down_event(MouseButton::Left, 120.0, 340.0);
    recorder.mouse_button_up_event(MouseButton::Left, 122.0, 341.0);
    let mut r = recogniser();
    for (i, event) in recorder.events.drain(..).enumerate() {
        if let kitty_mobile::input::Raw::Touch { id, phase, x, y } = event {
            assert_eq!(id, MOUSE_ID);
            r.event(id, phase, vec2(x, y), i as f64 * FRAME);
        }
    }
    assert!(r.take().tap.is_some());
}

#[test]
fn keys_move_the_focus_and_press_it() {
    let mut keys = MenuKeys::default();
    let mut press = |code: KeyCode, repeat: bool| {
        keys.event(code, true, repeat);
        keys.take()
    };
    assert_eq!(press(KeyCode::Down, false), Some(Nav::Down));
    assert_eq!(press(KeyCode::Down, true), Some(Nav::Down), "held keys repeat");
    assert_eq!(press(KeyCode::Tab, false), Some(Nav::Down));
    assert_eq!(press(KeyCode::W, false), Some(Nav::Up));
    assert_eq!(press(KeyCode::A, false), Some(Nav::Left));
    assert_eq!(press(KeyCode::Enter, false), Some(Nav::Activate));
    assert_eq!(press(KeyCode::Space, false), Some(Nav::Activate));
    assert_eq!(press(KeyCode::Escape, false), Some(Nav::Back));
    assert_eq!(press(KeyCode::Back, false), Some(Nav::Back));
    assert_eq!(press(KeyCode::PageDown, false), Some(Nav::PageDown));
    assert_eq!(press(KeyCode::Q, false), None);
    keys.event(KeyCode::Down, false, false);
    assert_eq!(keys.take(), None, "letting go does nothing");
    assert_eq!(ui::focus_step(Some(Nav::Right), 1, 2), 0);
    assert_eq!(ui::focus_step(Some(Nav::Left), 0, 3), 2);
    assert_eq!(ui::focus_step(None, 5, 3), 2);
}

/// A running key still held when a menu opens doesn't wander the focus with its repeats.
#[test]
fn a_key_held_across_a_screen_change_does_nothing_until_let_go() {
    let mut keys = MenuKeys::default();
    keys.event(KeyCode::Right, true, false);
    keys.take();
    keys.ignore_current();
    keys.event(KeyCode::Right, true, true);
    assert_eq!(keys.take(), None, "its repeats are ignored");
    keys.event(KeyCode::Down, true, false);
    assert_eq!(keys.take(), Some(Nav::Down), "other keys work");
    keys.event(KeyCode::Right, false, false);
    keys.event(KeyCode::Right, true, false);
    assert_eq!(keys.take(), Some(Nav::Right), "pressed afresh, it works again");
}

#[test]
fn buttons_never_overlap_and_keep_clear_of_the_pause_button() {
    let pause = Rect::new(PAUSE_ZONE_X, 0.0, 600.0 - PAUSE_ZONE_X, PAUSE_ZONE_Y);
    let layouts: Vec<Vec<Rect>> = vec![
        ui::pause_buttons().to_vec(),
        ui::try_again_buttons().to_vec(),
        ui::clear_buttons().to_vec(),
        ui::endless_over_buttons().to_vec(),
    ];
    for buttons in layouts {
        for (i, a) in buttons.iter().enumerate() {
            assert!(a.x >= 20.0 && a.right() <= 580.0 && a.y >= 0.0 && a.bottom() <= 1200.0, "{a:?} on the playfield");
            assert!(a.h >= ui::BUTTON_H, "{a:?} tall enough for a thumb");
            assert!(!a.overlaps(&pause), "{a:?} clear of the pause button");
            for b in &buttons[i + 1..] {
                assert!(!a.overlaps(b), "{a:?} and {b:?} overlap");
            }
        }
    }
}
