//! The story: five regions of three levels each, climbing from Mossy Hollow at the foot of the map
//! to the top of the Tallest Tree. A region's first two levels end at a clue to where its friend
//! blew, told by a helper who lives there; its third ends with the friend's rescue. Every level has
//! a fixed seed, so it's the same climb each time and the tests can check it can be climbed.

use crate::level::{Difficulty, LevelSpec};
use crate::look::{self, Look};
use crate::story::{Friend, Threat};
use crate::world::{NestGoal, Setup};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum RegionId {
    MossyHollow,
    WhisperingPines,
    CraggyPeaks,
    WillowCreek,
    TallestTree,
}

/// From the foot of the map to the top.
pub const REGION_IDS: [RegionId; 5] =
    [RegionId::MossyHollow, RegionId::WhisperingPines, RegionId::CraggyPeaks, RegionId::WillowCreek, RegionId::TallestTree];

impl RegionId {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn info(self) -> &'static Region {
        &REGIONS[self.index()]
    }

    pub fn look(self) -> &'static Look {
        &look::REGIONS[self.index()]
    }

    /// Its three levels, in order.
    pub fn levels(self) -> impl Iterator<Item = LevelId> {
        LevelId::all().filter(move |id| id.def().region == self)
    }

    /// The two things its friend left behind, in the order they're found.
    pub fn clue_objects(self) -> [ClueObject; 2] {
        let mut objects = self.levels().filter_map(|id| id.def().clue().map(|clue| clue.object));
        [objects.next().unwrap(), objects.next().unwrap()]
    }
}

/// A critter who lives in a region and saw where its friend went.
pub struct Helper {
    pub name: &'static str,
    pub full_name: &'static str,
    /// Frame name prefix in assets/helpers, once there's art ("robin" -> robin_idle_0, robin_talk_0..).
    pub sprite: &'static str,
}

pub struct Region {
    pub id: RegionId,
    pub name: &'static str,
    /// A line about it for the map.
    pub blurb: &'static str,
    pub friend: Friend,
    pub helper: Helper,
}

pub const REGIONS: [Region; 5] = [
    Region {
        id: RegionId::MossyHollow,
        name: "Mossy Hollow",
        blurb: "A sunny oak wood, soft with moss.",
        friend: Friend::Pip,
        helper: Helper { name: "Rosie", full_name: "Rosie the robin", sprite: "robin" },
    },
    Region {
        id: RegionId::WhisperingPines,
        name: "Whispering Pines",
        blurb: "Dark pines that whisper in the wind.",
        friend: Friend::Hazel,
        helper: Helper { name: "Professor Hoot", full_name: "Professor Hoot the owl", sprite: "owl" },
    },
    Region {
        id: RegionId::CraggyPeaks,
        name: "Craggy Peaks",
        blurb: "Windy rocks high above the trees.",
        friend: Friend::Mochi,
        helper: Helper { name: "Pebble", full_name: "Pebble the pika", sprite: "pika" },
    },
    Region {
        id: RegionId::WillowCreek,
        name: "Willow Creek",
        blurb: "Willows bending over a sparkling creek.",
        friend: Friend::Sunny,
        helper: Helper { name: "Flick", full_name: "Flick the kingfisher", sprite: "kingfisher" },
    },
    Region {
        id: RegionId::TallestTree,
        name: "The Tallest Tree",
        blurb: "An ancient tree that reaches the clouds.",
        friend: Friend::Luna,
        helper: Helper { name: "Dusk", full_name: "Dusk the bat", sprite: "bat" },
    },
];

/// A level, as an index into `LEVELS`. Saves use its key instead, which never changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct LevelId(pub usize);

impl LevelId {
    pub fn all() -> impl Iterator<Item = LevelId> {
        (0..LEVELS.len()).map(LevelId)
    }

    pub fn def(self) -> &'static LevelDef {
        &LEVELS[self.0]
    }

    pub fn next(self) -> Option<LevelId> {
        (self.0 + 1 < LEVELS.len()).then_some(LevelId(self.0 + 1))
    }

    pub fn by_key(key: &str) -> Option<LevelId> {
        Self::all().find(|id| id.def().key == key)
    }

    /// The last level: rescuing Luna, which unlocks the endless climb.
    pub fn last() -> LevelId {
        LevelId(LEVELS.len() - 1)
    }
}

pub struct Dogs {
    pub max: usize,
    /// Seconds between dogs trotting in (each wait 0.5x to 1.5x this).
    pub interval: f32,
    pub speed: f32,
}

pub struct Crows {
    pub max: usize,
    pub interval: (f32, f32),
}

/// Eagles come one at a time.
pub struct Eagles {
    pub per_level: usize,
    pub interval: (f32, f32),
    pub speed: f32,
}

/// Everything that makes a level easy or hard.
pub struct Tuning {
    pub path_len: usize,
    pub gap_extra: f32,
    pub stiffness: f32,
    pub wind: f32,
    pub hearts: usize,
    pub squirrels: usize,
    pub snakes: usize,
    pub pace: f32,
    pub dogs: Option<Dogs>,
    pub crows: Option<Crows>,
    pub eagles: Option<Eagles>,
}

impl Tuning {
    pub fn spec(&self) -> LevelSpec {
        let dogs = self.dogs.as_ref();
        let crows = self.crows.as_ref();
        let eagles = self.eagles.as_ref();
        LevelSpec {
            path_len: self.path_len,
            hearts: self.hearts,
            difficulty: Difficulty {
                gap_extra: self.gap_extra,
                stiffness: self.stiffness,
                wind: self.wind,
                squirrels: self.squirrels,
                snakes: self.snakes,
                dogs: dogs.is_some(),
                max_dogs: dogs.map_or(0, |d| d.max),
                dog_interval: dogs.map_or(5.0, |d| d.interval),
                dog_speed: dogs.map_or(1.0, |d| d.speed),
                crows: crows.is_some(),
                max_crows: crows.map_or(0, |c| c.max),
                crow_interval: crows.map_or((5.0, 10.0), |c| c.interval),
                eagles: eagles.is_some(),
                max_eagles: 1,
                eagles_per_level: eagles.map_or(0, |e| e.per_level),
                eagle_interval: eagles.map_or((5.0, 10.0), |e| e.interval),
                eagle_speed: eagles.map_or(1.0, |e| e.speed),
                pace: self.pace,
            },
        }
    }
}

/// Something a friend left behind on the wind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClueObject {
    Carrot,
    Fur,
    Quill,
    Berry,
    Seeds,
    Pouch,
    Feather,
    Crust,
    Bell,
    Ribbon,
}

impl ClueObject {
    /// Frame name prefix in assets/clues, once there's art ("carrot" -> carrot_0).
    pub fn sprite(self) -> &'static str {
        match self {
            ClueObject::Carrot => "carrot",
            ClueObject::Fur => "fur",
            ClueObject::Quill => "quill",
            ClueObject::Berry => "berry",
            ClueObject::Seeds => "seeds",
            ClueObject::Pouch => "pouch",
            ClueObject::Feather => "feather",
            ClueObject::Crust => "crust",
            ClueObject::Bell => "bell",
            ClueObject::Ribbon => "ribbon",
        }
    }
}

pub struct Clue {
    pub object: ClueObject,
    /// What Kitty says on finding it.
    pub found: &'static str,
    /// What the region's helper tells her.
    pub helper_line: &'static str,
}

pub enum Goal {
    Clue(Clue),
    /// The region's friend, in their nest.
    Rescue,
}

pub struct LevelDef {
    /// Its name in saves. Never renumbered.
    pub key: &'static str,
    /// Bumped when the level's layout changes, so stale best times and scores are dropped.
    pub layout_rev: u8,
    pub region: RegionId,
    /// 1..=3 within its region.
    pub stage: usize,
    pub title: &'static str,
    pub seed: u64,
    pub tuning: Tuning,
    /// The critter it brings in for the first time, if any.
    pub introduces: Option<Threat>,
    /// The critter its intro warns about, and in other words than its usual tip, if need be.
    pub watch_out: Threat,
    pub watch_tip: Option<&'static str>,
    /// Kitty's thought on a clue level; the friend's call from the nest on a rescue.
    pub intro: &'static str,
    pub goal: Goal,
    /// Reaching the top within this many seconds wins the time star.
    pub star_time: f32,
}

impl LevelDef {
    pub fn friend(&self) -> Friend {
        self.region.info().friend
    }

    pub fn spec(&self) -> LevelSpec {
        self.tuning.spec()
    }

    pub fn clue(&self) -> Option<&Clue> {
        match &self.goal {
            Goal::Clue(clue) => Some(clue),
            Goal::Rescue => None,
        }
    }

    /// What's waiting at the top.
    pub fn nest(&self) -> NestGoal {
        match &self.goal {
            Goal::Clue(clue) => NestGoal::Clue { friend: self.friend(), object: clue.object },
            Goal::Rescue => NestGoal::Rescue(self.friend()),
        }
    }

    /// Everything a World needs to play it.
    pub fn setup(&self) -> Setup {
        Setup { spec: self.spec(), seed: self.seed, look: *self.region.look(), nest: self.nest() }
    }
}

/// Whether `difficulty` brings `threat` into the level at all.
pub fn in_roster(threat: Threat, difficulty: &Difficulty) -> bool {
    match threat {
        Threat::Squirrels => difficulty.squirrels > 0,
        Threat::Dogs => difficulty.dogs,
        Threat::Snakes => difficulty.snakes > 0,
        Threat::Crows => difficulty.crows,
        Threat::Eagles => difficulty.eagles,
    }
}

pub const ALL_THREATS: [Threat; 5] = [Threat::Squirrels, Threat::Dogs, Threat::Snakes, Threat::Crows, Threat::Eagles];

/// The fifteen levels, from the foot of the map to the top. Difficulty climbs steadily all the
/// way: on the old five-level scale (1 for its first level, 9 for its fifth) Acorn Way is 1 and The
/// Very Top 11.5, with the gaps 20 px wider, the wood 0.035 bendier and the wind 0.6 stronger
/// for each step.
pub const LEVELS: [LevelDef; 15] = [
    // Mossy Hollow: squirrels, then dogs, then crows
    LevelDef {
        key: "mossy-1",
        layout_rev: 1,
        region: RegionId::MossyHollow,
        stage: 1,
        title: "Acorn Way",
        seed: 1000,
        tuning: Tuning {
            path_len: 10, gap_extra: 20.0, stiffness: 0.95, wind: 3.0, hearts: 1, squirrels: 2, snakes: 0, pace: 1.0,
            dogs: None, crows: None, eagles: None,
        },
        introduces: Some(Threat::Squirrels),
        watch_out: Threat::Squirrels,
        watch_tip: None,
        intro: "Something's caught on the very top branch. Could it be a sign of Pip?",
        goal: Goal::Clue(Clue {
            object: ClueObject::Carrot,
            found: "Pip's half-eaten carrot!",
            helper_line: "Tweet! A little grey bunny tumbled past on the wind, still holding her carrot. She went that way, toward the brambles!",
        }),
        star_time: 45.0,
    },
    LevelDef {
        key: "mossy-2",
        layout_rev: 1,
        region: RegionId::MossyHollow,
        stage: 2,
        title: "Bramble Bend",
        seed: 2004,
        tuning: Tuning {
            path_len: 11, gap_extra: 30.0, stiffness: 0.93, wind: 3.3, hearts: 1, squirrels: 2, snakes: 0, pace: 1.0,
            dogs: Some(Dogs { max: 1, interval: 6.0, speed: 1.0 }), crows: None, eagles: None,
        },
        introduces: Some(Threat::Dogs),
        watch_out: Threat::Dogs,
        watch_tip: None,
        intro: "The trail leads into the brambles. Pip must have come this way!",
        goal: Goal::Clue(Clue {
            object: ClueObject::Fur,
            found: "A tuft of Pip's grey fur!",
            helper_line: "Tweet tweet! The wind dropped her in the Old Oak, the biggest tree in the hollow. Hurry, Kitty!",
        }),
        star_time: 50.0,
    },
    LevelDef {
        key: "mossy-3",
        layout_rev: 1,
        region: RegionId::MossyHollow,
        stage: 3,
        title: "The Old Oak",
        seed: 3000,
        tuning: Tuning {
            path_len: 12, gap_extra: 40.0, stiffness: 0.915, wind: 3.6, hearts: 2, squirrels: 3, snakes: 0, pace: 1.05,
            dogs: Some(Dogs { max: 1, interval: 5.0, speed: 1.05 }), crows: Some(Crows { max: 1, interval: (7.0, 12.0) }),
            eagles: None,
        },
        introduces: Some(Threat::Crows),
        watch_out: Threat::Crows,
        watch_tip: None,
        intro: "Kitty! Up here! The wind blew me sooo high, and my ears are all in a tangle!",
        goal: Goal::Rescue,
        star_time: 60.0,
    },
    // Whispering Pines: eagles
    LevelDef {
        key: "pines-1",
        layout_rev: 1,
        region: RegionId::WhisperingPines,
        stage: 1,
        title: "Pine Needle Path",
        seed: 4001,
        tuning: Tuning {
            path_len: 12, gap_extra: 55.0, stiffness: 0.89, wind: 4.05, hearts: 2, squirrels: 3, snakes: 0, pace: 1.14,
            dogs: Some(Dogs { max: 1, interval: 4.4, speed: 1.14 }), crows: Some(Crows { max: 1, interval: (5.0, 9.0) }),
            eagles: Some(Eagles { per_level: 2, interval: (8.0, 14.0), speed: 1.0 }),
        },
        introduces: Some(Threat::Eagles),
        watch_out: Threat::Eagles,
        watch_tip: None,
        intro: "It's so dark and whispery in these pines... Hazel, where are you?",
        goal: Goal::Clue(Clue {
            object: ClueObject::Quill,
            found: "One of Hazel's quills!",
            helper_line: "Hoo-hoo! A prickly little hedgehog rolled past my hollow like a pinecone. Deeper into the pines, I'd say.",
        }),
        star_time: 60.0,
    },
    LevelDef {
        key: "pines-2",
        layout_rev: 1,
        region: RegionId::WhisperingPines,
        stage: 2,
        title: "Hush Hollow",
        seed: 5000,
        tuning: Tuning {
            path_len: 12, gap_extra: 70.0, stiffness: 0.86, wind: 4.5, hearts: 2, squirrels: 3, snakes: 0, pace: 1.2,
            dogs: Some(Dogs { max: 2, interval: 4.1, speed: 1.2 }), crows: Some(Crows { max: 1, interval: (5.0, 9.0) }),
            eagles: Some(Eagles { per_level: 3, interval: (6.0, 12.0), speed: 1.0 }),
        },
        introduces: None,
        watch_out: Threat::Eagles,
        watch_tip: None,
        intro: "Shh... the pines are so quiet here. Something's glinting at the top!",
        goal: Goal::Clue(Clue {
            object: ClueObject::Berry,
            found: "A nibbled blackberry. Hazel's favourite!",
            helper_line: "Hoo! She was nibbling berries by moonlight. Look for the tallest pine, where the moon shines through.",
        }),
        star_time: 65.0,
    },
    LevelDef {
        key: "pines-3",
        layout_rev: 1,
        region: RegionId::WhisperingPines,
        stage: 3,
        title: "Moonlit Pines",
        seed: 6003,
        tuning: Tuning {
            path_len: 13, gap_extra: 85.0, stiffness: 0.84, wind: 4.95, hearts: 2, squirrels: 4, snakes: 0, pace: 1.26,
            dogs: Some(Dogs { max: 2, interval: 3.9, speed: 1.26 }), crows: Some(Crows { max: 1, interval: (4.5, 8.5) }),
            eagles: Some(Eagles { per_level: 4, interval: (5.0, 10.0), speed: 1.02 }),
        },
        introduces: None,
        watch_out: Threat::Dogs,
        watch_tip: None,
        intro: "Hello down there! I'm a bit too prickly to hug, but I would really love a rescue!",
        goal: Goal::Rescue,
        star_time: 75.0,
    },
    // Craggy Peaks: snakes
    LevelDef {
        key: "crags-1",
        layout_rev: 1,
        region: RegionId::CraggyPeaks,
        stage: 1,
        title: "Pebble Rise",
        seed: 7003,
        tuning: Tuning {
            path_len: 13, gap_extra: 100.0, stiffness: 0.81, wind: 5.4, hearts: 2, squirrels: 2, snakes: 1, pace: 1.32,
            dogs: Some(Dogs { max: 1, interval: 4.0, speed: 1.32 }), crows: Some(Crows { max: 1, interval: (4.0, 8.0) }),
            eagles: Some(Eagles { per_level: 3, interval: (4.6, 9.2), speed: 1.06 }),
        },
        introduces: Some(Threat::Snakes),
        watch_out: Threat::Snakes,
        watch_tip: None,
        intro: "Brrr, it's windy up in the peaks! Mochi could be anywhere on these rocks.",
        goal: Goal::Clue(Clue {
            object: ClueObject::Seeds,
            found: "A trail of sunflower seeds. Mochi's been here!",
            helper_line: "Eep! A round little hamster bounced up the rocks, dropping seeds everywhere. Up toward Goat Ledge!",
        }),
        star_time: 75.0,
    },
    LevelDef {
        key: "crags-2",
        layout_rev: 1,
        region: RegionId::CraggyPeaks,
        stage: 2,
        title: "Goat Ledge",
        seed: 8001,
        tuning: Tuning {
            path_len: 13, gap_extra: 115.0, stiffness: 0.785, wind: 5.85, hearts: 2, squirrels: 3, snakes: 2, pace: 1.38,
            dogs: Some(Dogs { max: 1, interval: 3.8, speed: 1.38 }), crows: Some(Crows { max: 1, interval: (4.0, 8.0) }),
            eagles: Some(Eagles { per_level: 4, interval: (4.3, 8.6), speed: 1.1 }),
        },
        introduces: None,
        watch_out: Threat::Snakes,
        watch_tip: None,
        intro: "More seeds on the ledges... Mochi's trail keeps going up!",
        goal: Goal::Clue(Clue {
            object: ClueObject::Pouch,
            found: "Mochi's seed pouch, all empty!",
            helper_line: "Eep eep! She's stuck on the Windy Crag, right at the top. Mind the snakes sunning on the ledges!",
        }),
        star_time: 80.0,
    },
    LevelDef {
        key: "crags-3",
        layout_rev: 1,
        region: RegionId::CraggyPeaks,
        stage: 3,
        title: "Windy Crag",
        seed: 9002,
        tuning: Tuning {
            path_len: 14, gap_extra: 130.0, stiffness: 0.76, wind: 6.3, hearts: 3, squirrels: 3, snakes: 3, pace: 1.44,
            dogs: Some(Dogs { max: 1, interval: 3.6, speed: 1.44 }), crows: Some(Crows { max: 1, interval: (4.0, 8.0) }),
            eagles: Some(Eagles { per_level: 4, interval: (4.0, 8.0), speed: 1.15 }),
        },
        introduces: None,
        watch_out: Threat::Eagles,
        watch_tip: None,
        intro: "I packed my cheeks with seeds for the trip... I just didn't plan on the trip being up a mountain!",
        goal: Goal::Rescue,
        star_time: 90.0,
    },
    // Willow Creek: crows in pairs
    LevelDef {
        key: "creek-1",
        layout_rev: 1,
        region: RegionId::WillowCreek,
        stage: 1,
        title: "Reed Bank",
        seed: 10003,
        tuning: Tuning {
            path_len: 14, gap_extra: 145.0, stiffness: 0.73, wind: 6.75, hearts: 2, squirrels: 4, snakes: 2, pace: 1.5,
            dogs: Some(Dogs { max: 2, interval: 3.0, speed: 1.5 }), crows: Some(Crows { max: 2, interval: (3.5, 7.0) }),
            eagles: Some(Eagles { per_level: 3, interval: (4.0, 8.0), speed: 1.15 }),
        },
        introduces: None,
        watch_out: Threat::Crows,
        watch_tip: Some("Crows fly in pairs along the creek! Step aside when one hovers, or bop it from above."),
        intro: "Willows, reeds and a sparkling creek. Sunny would love it here!",
        goal: Goal::Clue(Clue {
            object: ClueObject::Feather,
            found: "A fluffy yellow feather. Sunny's!",
            helper_line: "Chip-chip! A duckling floated downstream on a leaf. Follow the creek round the bend!",
        }),
        star_time: 90.0,
    },
    LevelDef {
        key: "creek-2",
        layout_rev: 1,
        region: RegionId::WillowCreek,
        stage: 2,
        title: "Lily Bend",
        seed: 11003,
        tuning: Tuning {
            path_len: 14, gap_extra: 160.0, stiffness: 0.705, wind: 7.2, hearts: 3, squirrels: 4, snakes: 3, pace: 1.56,
            dogs: Some(Dogs { max: 2, interval: 2.7, speed: 1.56 }), crows: Some(Crows { max: 2, interval: (3.0, 6.0) }),
            eagles: Some(Eagles { per_level: 3, interval: (4.0, 8.0), speed: 1.2 }),
        },
        introduces: None,
        watch_out: Threat::Dogs,
        watch_tip: None,
        intro: "Round the bend, past the lilies. I can hear a little peep somewhere!",
        goal: Goal::Clue(Clue {
            object: ClueObject::Crust,
            found: "A bread crust, nibbled by a little beak!",
            helper_line: "Chip! She's perched up in the great willow by the falls. Quick, before she tries to swim!",
        }),
        star_time: 95.0,
    },
    LevelDef {
        key: "creek-3",
        layout_rev: 1,
        region: RegionId::WillowCreek,
        stage: 3,
        title: "Willow Falls",
        seed: 12001,
        tuning: Tuning {
            path_len: 15, gap_extra: 175.0, stiffness: 0.68, wind: 7.65, hearts: 3, squirrels: 5, snakes: 3, pace: 1.62,
            dogs: Some(Dogs { max: 2, interval: 2.5, speed: 1.62 }), crows: Some(Crows { max: 2, interval: (2.8, 5.6) }),
            eagles: Some(Eagles { per_level: 4, interval: (3.6, 7.2), speed: 1.25 }),
        },
        introduces: None,
        watch_out: Threat::Snakes,
        watch_tip: None,
        intro: "Peep peep! My wings are still too fluffy to fly. Can you come and get me, Kitty?",
        goal: Goal::Rescue,
        star_time: 105.0,
    },
    // The Tallest Tree: everything
    LevelDef {
        key: "tree-1",
        layout_rev: 1,
        region: RegionId::TallestTree,
        stage: 1,
        title: "Root Hall",
        seed: 13003,
        tuning: Tuning {
            path_len: 15, gap_extra: 190.0, stiffness: 0.65, wind: 8.1, hearts: 3, squirrels: 5, snakes: 3, pace: 1.68,
            dogs: Some(Dogs { max: 3, interval: 2.2, speed: 1.68 }), crows: Some(Crows { max: 2, interval: (2.6, 5.2) }),
            eagles: Some(Eagles { per_level: 5, interval: (2.8, 5.6), speed: 1.33 }),
        },
        introduces: None,
        watch_out: Threat::Eagles,
        watch_tip: None,
        intro: "The Tallest Tree! Its roots are as big as houses. Luna must be up there somewhere.",
        goal: Goal::Clue(Clue {
            object: ClueObject::Bell,
            found: "Luna's little bell!",
            helper_line: "Squeak... a kitten's bell jingled past me in the night. She's high, high up, above the clouds.",
        }),
        star_time: 105.0,
    },
    LevelDef {
        key: "tree-2",
        layout_rev: 1,
        region: RegionId::TallestTree,
        stage: 2,
        title: "Cloud Branch",
        seed: 14001,
        tuning: Tuning {
            path_len: 15, gap_extra: 210.0, stiffness: 0.62, wind: 8.7, hearts: 3, squirrels: 6, snakes: 4, pace: 1.76,
            dogs: Some(Dogs { max: 3, interval: 2.0, speed: 1.76 }), crows: Some(Crows { max: 2, interval: (2.4, 4.8) }),
            eagles: Some(Eagles { per_level: 5, interval: (2.4, 4.8), speed: 1.39 }),
        },
        introduces: None,
        watch_out: Threat::Crows,
        watch_tip: None,
        intro: "Up among the clouds now. Don't look down, Kitty!",
        goal: Goal::Clue(Clue {
            object: ClueObject::Ribbon,
            found: "Luna's pink ribbon!",
            helper_line: "Squeak! I heard a tiny meow from the very top. Climb, Kitty, climb!",
        }),
        star_time: 110.0,
    },
    LevelDef {
        key: "tree-3",
        layout_rev: 1,
        region: RegionId::TallestTree,
        stage: 3,
        title: "The Very Top",
        seed: 15003,
        tuning: Tuning {
            path_len: 16, gap_extra: 230.0, stiffness: 0.585, wind: 9.3, hearts: 3, squirrels: 6, snakes: 4, pace: 1.84,
            dogs: Some(Dogs { max: 3, interval: 2.0, speed: 1.84 }), crows: Some(Crows { max: 2, interval: (2.2, 4.4) }),
            eagles: Some(Eagles { per_level: 5, interval: (2.0, 4.0), speed: 1.45 }),
        },
        introduces: None,
        watch_out: Threat::Eagles,
        watch_tip: None,
        intro: "Kitty, is that you? I can see the whole forest from up here... and it's a long way down!",
        goal: Goal::Rescue,
        star_time: 120.0,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level;
    use crate::story::FRIENDS;

    #[test]
    fn five_regions_of_three_levels_each_end_in_a_rescue() {
        assert_eq!(LEVELS.len(), 15);
        for (i, def) in LEVELS.iter().enumerate() {
            assert_eq!(def.region, REGION_IDS[i / 3], "{}", def.key);
            assert_eq!(def.stage, i % 3 + 1, "{}", def.key);
            assert_eq!(matches!(def.goal, Goal::Rescue), def.stage == 3, "{}", def.key);
            assert_eq!(def.region.info().id, def.region);
        }
        for (region, friend) in REGION_IDS.iter().zip(FRIENDS) {
            assert_eq!(region.info().friend, friend);
            assert_eq!(region.levels().count(), 3);
        }
        let keys: std::collections::HashSet<_> = LEVELS.iter().map(|d| d.key).collect();
        assert_eq!(keys.len(), LEVELS.len(), "keys are unique");
        let objects: Vec<ClueObject> = LEVELS.iter().filter_map(|d| d.clue().map(|c| c.object)).collect();
        assert_eq!(objects.len(), 10);
        assert!(objects.iter().enumerate().all(|(i, o)| !objects[..i].contains(o)), "clue objects are unique");
        assert_eq!(LevelId::by_key("crags-2"), Some(LevelId(7)));
        assert_eq!(LevelId::last().next(), None);
    }

    /// Each level brings in at most one new critter, says which, and warns about one it has; the
    /// pines bring eagles and the peaks snakes.
    #[test]
    fn critters_arrive_one_at_a_time() {
        let mut seen: Vec<Threat> = Vec::new();
        for def in &LEVELS {
            let d = def.spec().difficulty;
            let new: Vec<Threat> = ALL_THREATS.iter().copied().filter(|&t| in_roster(t, &d) && !seen.contains(&t)).collect();
            assert!(new.len() <= 1, "{} brings in {new:?}", def.key);
            assert_eq!(def.introduces, new.first().copied(), "{}", def.key);
            assert!(in_roster(def.watch_out, &d), "{} warns about {:?}, which isn't there", def.key, def.watch_out);
            seen.extend(new);
        }
        assert_eq!(LEVELS[3].introduces, Some(Threat::Eagles));
        assert_eq!(LEVELS[6].introduces, Some(Threat::Snakes));
        assert_eq!(seen.len(), ALL_THREATS.len());
    }

    /// Difficulty only ever climbs: longer paths, wider gaps, bendier wood and stronger wind.
    #[test]
    fn difficulty_climbs_steadily() {
        for pair in LEVELS.windows(2) {
            let (a, b) = (&pair[0].tuning, &pair[1].tuning);
            let what = format!("{} to {}", pair[0].key, pair[1].key);
            assert!(b.path_len >= a.path_len && b.gap_extra > a.gap_extra, "{what}");
            assert!(b.stiffness < a.stiffness && b.wind > a.wind, "{what}");
            assert!(b.pace >= a.pace, "{what}");
        }
        for def in &LEVELS {
            let t = &def.tuning;
            assert!(t.stiffness >= 0.5 && t.hearts >= 1 && def.star_time > 0.0, "{}", def.key);
            assert!(t.eagles.as_ref().is_none_or(|e| e.per_level >= 1), "{}", def.key);
        }
    }

    /// Each level is laid out exactly as when its seed was picked and checked (bump its
    /// `layout_rev`, re-pick its seed and update this when its tuning changes on purpose), and
    /// finds room for all its critters and hearts.
    #[test]
    fn layouts_are_frozen() {
        let expected = [
            ("mossy-1", 0xe61132ed296b1f19),
            ("mossy-2", 0xcaa4697eac265337),
            ("mossy-3", 0x6d965c05fe722964),
            ("pines-1", 0x310f3a5d0013a5d7),
            ("pines-2", 0xbd3b888ad3097948),
            ("pines-3", 0xbc3617f82918dc1f),
            ("crags-1", 0x5bf53a4d87974810),
            ("crags-2", 0x573ac5d61cda6d09),
            ("crags-3", 0xb800fe0a23c5ba52),
            ("creek-1", 0xbc874e0452d6e768),
            ("creek-2", 0x31508efe701d2eca),
            ("creek-3", 0x37ab911b9d7accea),
            ("tree-1", 0xc7f85d3b2cc436df),
            ("tree-2", 0x0af3da8cf73a48e0),
            ("tree-3", 0xaec267e31e63c22d),
        ];
        for (def, (key, fingerprint)) in LEVELS.iter().zip(expected) {
            assert_eq!(def.key, key);
            let spec = def.spec();
            let plan = level::generate(&spec, def.seed);
            assert_eq!(level::fingerprint(&plan), fingerprint, "{key}'s layout changed");
            let hearts = plan.items.iter().filter(|i| i.kind == level::ItemKind::Heart).count();
            assert_eq!((plan.squirrel_spots.len(), plan.snake_spots.len(), hearts),
                       (spec.difficulty.squirrels, spec.difficulty.snakes, spec.hearts), "{key}");
        }
    }

    /// Every clue has its picture in assets/clues (sliced from assets/source/clues_sheet.jpg).
    #[test]
    fn every_clue_has_its_picture() {
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string("../assets/clues/anchors.json").unwrap()).unwrap();
        for def in &LEVELS {
            if let Some(clue) = def.clue() {
                let file = format!("{}_0.png", clue.object.sprite());
                assert!(manifest["anchors"].get(&file).is_some(), "{}: no {file}", def.key);
            }
        }
        for region in REGION_IDS {
            assert_eq!(region.clue_objects().len(), 2);
        }
    }

    /// Every level's plan is sound on many seeds, not just its own, so a tuning that needs lucky
    /// seeds shows up here.
    #[test]
    fn tunings_lay_out_sound_levels() {
        for def in &LEVELS {
            let spec = def.spec();
            for seed in 0..60 {
                if let Err(problem) = level::check_plan(&level::generate(&spec, seed)) {
                    panic!("{} seed {seed}: {problem}", def.key);
                }
            }
        }
    }
}
