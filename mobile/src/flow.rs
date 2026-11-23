//! The rules of progress, apart from any drawing so the tests can check them: what's unlocked,
//! what's been found, what a cleared level earns and how it's scored. Everything is worked out
//! from the save's facts (which levels are cleared, with what), so nothing can disagree.

use crate::campaign::{Goal, LevelId, RegionId, LEVELS};
use crate::consts::*;
use crate::save::{LevelRecord, SaveData, Stars};
use crate::story::{Friend, Tally};

/// Open to play: the first level, the one after each cleared level, or every one in playtesting.
pub fn unlocked(save: &SaveData, id: LevelId) -> bool {
    save.unlock_all || id.0 == 0 || save.cleared(LEVELS[id.0 - 1].key)
}

/// Where Kitty sits on the map: the first open level not yet cleared, or the last level once
/// they all are.
pub fn newest(save: &SaveData) -> LevelId {
    LevelId::all().find(|&id| unlocked(save, id) && !save.cleared(id.def().key)).unwrap_or(LevelId::last())
}

/// Clues found in `region` (cleared clue levels).
pub fn clues_found(save: &SaveData, region: RegionId) -> usize {
    region.levels().filter(|id| matches!(id.def().goal, Goal::Clue(_)) && save.cleared(id.def().key)).count()
}

/// Whether `friend` is safe home (their region's last level cleared).
pub fn rescued(save: &SaveData, friend: Friend) -> bool {
    LevelId::all().any(|id| matches!(id.def().goal, Goal::Rescue) && id.def().friend() == friend && save.cleared(id.def().key))
}

/// The endless climb opens once Luna is home (or for playtesting).
pub fn endless_unlocked(save: &SaveData) -> bool {
    save.unlock_all || save.cleared(LevelId::last().def().key)
}

/// Stars won across every level, and how many there are to win.
pub fn total_stars(save: &SaveData) -> (usize, usize) {
    let won = LEVELS.iter().map(|def| save.level(def.key).stars.count()).sum();
    (won, LEVELS.len() * 3)
}

/// How an attempt went, from the World.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttemptResult {
    /// Climbing, treats and bops.
    pub points: u32,
    /// The treats' share of that.
    pub collected: u32,
    /// Seconds of play to the top (pauses and the goal's moment left out).
    pub elapsed: f32,
    pub hits_taken: u32,
}

/// The level-clear tally: the goal (a friend or a clue) is worth 1000, each second under `par`
/// 10 more, and no bumps 500, on top of the climbing, treats and bops. `so_far` is what earlier
/// levels of an endless run scored.
pub fn tally(par: f32, result: &AttemptResult, so_far: u32) -> Tally {
    let time_bonus = ((par - result.elapsed).max(0.0) as u32) * SCORE_PER_SECOND_LEFT;
    let no_damage = if result.hits_taken == 0 { SCORE_NO_DAMAGE } else { 0 };
    let level_total = result.points + SCORE_RESCUE + time_bonus + no_damage;
    Tally { rescue: SCORE_RESCUE, time_bonus, no_damage, collected: result.collected, level_total, score: so_far + level_total }
}

/// A campaign level's par for the time bonus: twice its star time (90 s on the first level, as
/// the game always had).
pub fn par(id: LevelId) -> f32 {
    id.def().star_time * 2.0
}

/// The stars an attempt that reached the top earned.
pub fn stars_for(id: LevelId, result: &AttemptResult) -> Stars {
    Stars { top: true, no_bumps: result.hits_taken == 0, quick: result.elapsed <= id.def().star_time }
}

/// What clearing a level changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClearOutcome {
    /// The stars this attempt earned, and which of them weren't held before.
    pub stars: Stars,
    pub new_stars: Stars,
    pub new_best: bool,
    pub first_clear: bool,
    /// The level it opened, if any.
    pub unlocked: Option<LevelId>,
    /// The friend it brought home, the first time.
    pub rescued: Option<Friend>,
    /// Whether it opened the endless climb.
    pub endless_unlocked: bool,
}

/// Record a cleared level in `save`: cleared, its stars, and its best score and time.
pub fn record_clear(save: &mut SaveData, id: LevelId, result: &AttemptResult, score: u32) -> ClearOutcome {
    let def = id.def();
    let stars = stars_for(id, result);
    let endless_before = endless_unlocked(save);
    let next_was_open = id.next().is_some_and(|next| unlocked(save, next));
    let before = save.level(def.key);
    let fresh = before.layout_rev != def.layout_rev;
    let record = LevelRecord {
        cleared: true,
        best_score: if fresh { score } else { before.best_score.max(score) },
        best_time: match (before.best_time, fresh) {
            (Some(best), false) => Some(best.min(result.elapsed)),
            _ => Some(result.elapsed),
        },
        stars: before.stars.union(stars),
        layout_rev: def.layout_rev,
    };
    save.levels.insert(def.key.to_string(), record);
    let held = before.stars;
    ClearOutcome {
        stars,
        new_stars: Stars { top: stars.top && !held.top, no_bumps: stars.no_bumps && !held.no_bumps, quick: stars.quick && !held.quick },
        new_best: before.cleared && score > before.best_score && !fresh,
        first_clear: !before.cleared,
        unlocked: id.next().filter(|_| !next_was_open),
        rescued: (matches!(def.goal, Goal::Rescue) && !before.cleared).then(|| def.friend()),
        endless_unlocked: !endless_before && endless_unlocked(save),
    }
}

/// Record an endless run's score and how far it climbed; whether the score is a new best.
pub fn record_endless(save: &mut SaveData, score: u32, climb: u32) -> bool {
    let best = score > save.endless.best_score;
    save.endless.best_score = save.endless.best_score.max(score);
    save.endless.best_climb = save.endless.best_climb.max(climb);
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::REGION_IDS;

    fn result(elapsed: f32, hits_taken: u32) -> AttemptResult {
        AttemptResult { points: 200, collected: 50, elapsed, hits_taken }
    }

    #[test]
    fn levels_open_one_at_a_time() {
        let mut save = SaveData::default();
        assert!(unlocked(&save, LevelId(0)));
        assert!(!unlocked(&save, LevelId(1)));
        assert_eq!(newest(&save), LevelId(0));
        let outcome = record_clear(&mut save, LevelId(0), &result(40.0, 0), 1800);
        assert_eq!(outcome.unlocked, Some(LevelId(1)));
        assert!(outcome.first_clear && !outcome.new_best);
        assert!(unlocked(&save, LevelId(1)) && !unlocked(&save, LevelId(2)), "never two ahead");
        assert_eq!(newest(&save), LevelId(1));
        assert_eq!(clues_found(&save, RegionId::MossyHollow), 1);
        // Clearing it again opens nothing new
        let again = record_clear(&mut save, LevelId(0), &result(60.0, 2), 900);
        assert_eq!(again.unlocked, None);
        assert!(!again.first_clear);
        assert!(!endless_unlocked(&save));
    }

    #[test]
    fn stars_and_bests_add_up_over_attempts() {
        let mut save = SaveData::default();
        let first = record_clear(&mut save, LevelId(0), &result(80.0, 1), 1200);
        assert_eq!(first.stars, Stars { top: true, no_bumps: false, quick: false });
        let second = record_clear(&mut save, LevelId(0), &result(30.0, 0), 1500);
        assert_eq!(second.new_stars, Stars { top: false, no_bumps: true, quick: true });
        assert!(second.new_best);
        let third = record_clear(&mut save, LevelId(0), &result(90.0, 3), 400);
        assert_eq!(third.new_stars.count(), 0);
        assert!(!third.new_best);
        let record = save.level(LevelId(0).def().key);
        assert_eq!((record.stars.count(), record.best_score, record.best_time), (3, 1500, Some(30.0)));
        assert_eq!(total_stars(&save), (3, 45));
    }

    #[test]
    fn a_rescue_brings_a_friend_home_and_the_last_opens_endless() {
        let mut save = SaveData::default();
        for i in 0..3 {
            let outcome = record_clear(&mut save, LevelId(i), &result(50.0, 0), 1000);
            assert_eq!(outcome.rescued, (i == 2).then_some(Friend::Pip));
        }
        assert!(rescued(&save, Friend::Pip) && !rescued(&save, Friend::Hazel));
        for i in 3..14 {
            record_clear(&mut save, LevelId(i), &result(50.0, 0), 1000);
        }
        assert!(!endless_unlocked(&save));
        let last = record_clear(&mut save, LevelId::last(), &result(100.0, 0), 1000);
        assert!(last.endless_unlocked && last.unlocked.is_none() && last.rescued == Some(Friend::Luna));
        assert!(!record_clear(&mut save, LevelId::last(), &result(100.0, 0), 1000).endless_unlocked, "only the first time");
        for region in REGION_IDS {
            assert_eq!(clues_found(&save, region), 2);
            assert!(rescued(&save, region.info().friend));
        }
        assert_eq!(newest(&save), LevelId::last());
    }

    #[test]
    fn playtest_unlock_all_opens_everything_but_clears_nothing() {
        let save = SaveData { unlock_all: true, ..SaveData::default() };
        assert!(LevelId::all().all(|id| unlocked(&save, id)));
        assert!(endless_unlocked(&save));
        assert_eq!(newest(&save), LevelId(0));
        assert_eq!(total_stars(&save).0, 0);
    }

    #[test]
    fn a_tally_rewards_speed_and_care() {
        assert_eq!(par(LevelId(0)), 90.0);
        let t = tally(90.0, &result(40.0, 0), 0);
        assert_eq!((t.rescue, t.time_bonus, t.no_damage), (1000, 500, 500));
        assert_eq!(t.level_total, 200 + 1000 + 500 + 500);
        let slow = tally(90.0, &result(120.0, 2), 5000);
        assert_eq!((slow.time_bonus, slow.no_damage, slow.score), (0, 0, 5000 + 1200));
    }

    #[test]
    fn endless_keeps_the_best() {
        let mut save = SaveData::default();
        assert!(record_endless(&mut save, 3000, 4));
        assert!(!record_endless(&mut save, 2000, 7));
        assert_eq!((save.endless.best_score, save.endless.best_climb), (3000, 7));
    }

    /// A best set on an older layout of a level doesn't count against the new one.
    #[test]
    fn a_new_layout_starts_the_bests_afresh() {
        let mut save = SaveData::default();
        let key = LevelId(0).def().key;
        save.levels.insert(key.to_string(), LevelRecord { cleared: true, best_score: 9999, best_time: Some(10.0), stars: Stars::default(), layout_rev: 0 });
        let outcome = record_clear(&mut save, LevelId(0), &result(50.0, 0), 1500);
        assert!(!outcome.new_best && !outcome.first_clear);
        assert_eq!((save.level(key).best_score, save.level(key).best_time), (1500, Some(50.0)));
    }
}
