//! How a region looks: its sky, how its scenery and leaves are tinted, how many leaves have turned,
//! its wood and its ground. Only colours: a look never changes where anything is.

use macroquad::prelude::*;

use crate::consts::*;

/// Bark and wood, from the trunks' texture to the pale wood where a limb snaps.
#[derive(Clone, Copy, Debug)]
pub struct WoodColors {
    pub trunk: Color,
    /// Outlines, furrows and cracks.
    pub outline: Color,
    /// Sturdy limbs: sunlit top edge, body and underside.
    pub light: Color,
    pub fill: Color,
    pub shade: Color,
    /// Thin limbs that can snap: young wood, paler.
    pub young_light: Color,
    pub young: Color,
    pub young_shade: Color,
    pub splinter: Color,
}

#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// Sky at the top of the screen and at the horizon, low down, then high up the trees.
    pub sky: [Color; 4],
    /// Multiplies the painted scenery and clouds, and the leaves on the limbs and trees.
    pub scenery_tint: Color,
    pub foliage_tint: Color,
    /// Share of leaves that have turned.
    pub autumn: f32,
    pub wood: WoodColors,
    /// Grass, the grass's dark edge, and soil.
    pub ground: [Color; 3],
    /// The region's band on the map, top to bottom.
    pub band: (Color, Color),
}

const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

/// Today's wood: a warm brown oak.
pub const OAK: WoodColors = WoodColors {
    trunk: TRUNK_COLOR,
    outline: BARK_DARK,
    light: BARK_LIGHT,
    fill: BARK,
    shade: BARK_SHADE,
    young_light: BARK_YOUNG_LIGHT,
    young: BARK_YOUNG,
    young_shade: BARK_YOUNG_SHADE,
    splinter: WOOD_SPLINTER,
};

const SKY: [Color; 4] = [rgb(0.45, 0.72, 0.95), rgb(0.80, 0.92, 0.98), rgb(0.26, 0.47, 0.82), rgb(0.62, 0.80, 0.96)];

/// Mossy Hollow: a sunny oak wood, the woods as they've always looked, barely turning.
pub const MOSSY_HOLLOW: Look = Look {
    sky: SKY,
    scenery_tint: WHITE,
    foliage_tint: WHITE,
    autumn: 0.15,
    wood: OAK,
    ground: [GRASS, GRASS_DARK, SOIL],
    band: (rgb(0.55, 0.75, 0.40), rgb(0.40, 0.62, 0.30)),
};

/// Whispering Pines: cooler and darker, blue-green, with dark red-brown wood and no autumn.
pub const WHISPERING_PINES: Look = Look {
    sky: [rgb(0.30, 0.45, 0.70), rgb(0.62, 0.74, 0.82), rgb(0.20, 0.33, 0.58), rgb(0.50, 0.64, 0.78)],
    scenery_tint: rgb(0.62, 0.72, 0.78),
    foliage_tint: rgb(0.55, 0.75, 0.62),
    autumn: 0.0,
    wood: WoodColors {
        trunk: rgb(0.28, 0.17, 0.11),
        outline: rgb(0.18, 0.10, 0.06),
        light: rgb(0.50, 0.32, 0.20),
        fill: rgb(0.38, 0.23, 0.14),
        shade: rgb(0.27, 0.16, 0.10),
        young_light: rgb(0.66, 0.55, 0.36),
        young: rgb(0.54, 0.40, 0.24),
        young_shade: rgb(0.40, 0.28, 0.16),
        splinter: rgb(0.85, 0.72, 0.52),
    },
    ground: [rgb(0.22, 0.40, 0.24), rgb(0.16, 0.30, 0.18), rgb(0.25, 0.18, 0.12)],
    band: (rgb(0.24, 0.42, 0.36), rgb(0.16, 0.30, 0.28)),
};

/// Craggy Peaks: thin bright mountain air, grey-violet scenery, weathered grey wood, stony ground.
pub const CRAGGY_PEAKS: Look = Look {
    sky: [rgb(0.40, 0.62, 0.90), rgb(0.85, 0.90, 0.95), rgb(0.30, 0.50, 0.85), rgb(0.75, 0.84, 0.95)],
    scenery_tint: rgb(0.85, 0.82, 0.90),
    foliage_tint: rgb(0.85, 0.90, 0.75),
    autumn: 0.55,
    wood: WoodColors {
        trunk: rgb(0.40, 0.34, 0.29),
        outline: rgb(0.24, 0.20, 0.17),
        light: rgb(0.62, 0.55, 0.47),
        fill: rgb(0.48, 0.41, 0.35),
        shade: rgb(0.36, 0.30, 0.26),
        young_light: rgb(0.74, 0.67, 0.52),
        young: rgb(0.62, 0.54, 0.42),
        young_shade: rgb(0.48, 0.40, 0.30),
        splinter: rgb(0.88, 0.80, 0.66),
    },
    ground: [rgb(0.50, 0.52, 0.40), rgb(0.40, 0.42, 0.32), rgb(0.40, 0.38, 0.38)],
    band: (rgb(0.62, 0.60, 0.66), rgb(0.48, 0.46, 0.52)),
};

/// Willow Creek: fresh and green by the water, soft willow-grey wood.
pub const WILLOW_CREEK: Look = Look {
    sky: [rgb(0.50, 0.76, 0.92), rgb(0.88, 0.94, 0.86), rgb(0.36, 0.60, 0.86), rgb(0.74, 0.86, 0.90)],
    scenery_tint: rgb(0.90, 1.0, 0.92),
    foliage_tint: rgb(0.85, 1.0, 0.70),
    autumn: 0.1,
    wood: WoodColors {
        trunk: rgb(0.38, 0.30, 0.22),
        outline: rgb(0.22, 0.17, 0.11),
        light: rgb(0.60, 0.50, 0.36),
        fill: rgb(0.47, 0.37, 0.25),
        shade: rgb(0.34, 0.26, 0.17),
        young_light: rgb(0.76, 0.70, 0.48),
        young: rgb(0.63, 0.54, 0.33),
        young_shade: rgb(0.48, 0.39, 0.22),
        splinter: rgb(0.90, 0.82, 0.60),
    },
    ground: [rgb(0.38, 0.62, 0.30), rgb(0.28, 0.48, 0.22), rgb(0.30, 0.24, 0.18)],
    band: (rgb(0.45, 0.70, 0.62), rgb(0.32, 0.56, 0.55)),
};

/// The Tallest Tree: warm light high in the sky, ancient dark wood, a canopy deep in autumn.
pub const TALLEST_TREE: Look = Look {
    sky: [rgb(0.26, 0.47, 0.82), rgb(0.75, 0.85, 0.98), rgb(0.18, 0.36, 0.74), rgb(0.62, 0.78, 0.98)],
    scenery_tint: rgb(1.0, 0.95, 0.88),
    foliage_tint: WHITE,
    autumn: 0.7,
    wood: WoodColors {
        trunk: rgb(0.30, 0.19, 0.11),
        outline: rgb(0.18, 0.11, 0.06),
        light: rgb(0.52, 0.34, 0.20),
        fill: rgb(0.38, 0.25, 0.16),
        shade: rgb(0.28, 0.18, 0.10),
        young_light: rgb(0.72, 0.60, 0.38),
        young: rgb(0.58, 0.43, 0.25),
        young_shade: rgb(0.44, 0.31, 0.17),
        splinter: rgb(0.88, 0.75, 0.52),
    },
    ground: [GRASS, GRASS_DARK, SOIL],
    band: (rgb(0.55, 0.68, 0.88), rgb(0.40, 0.55, 0.80)),
};

/// The regions' looks, from the bottom of the map to the top.
pub const REGIONS: [Look; 5] = [MOSSY_HOLLOW, WHISPERING_PINES, CRAGGY_PEAKS, WILLOW_CREEK, TALLEST_TREE];

/// The endless climb's `level` (1..=5) takes the look of the region in the same place.
pub fn endless(level: usize) -> &'static Look {
    &REGIONS[(level.max(1) - 1) % REGIONS.len()]
}

/// `color` multiplied by `tint`, keeping its alpha.
pub fn tinted(color: Color, tint: Color) -> Color {
    Color::new(color.r * tint.r, color.g * tint.g, color.b * tint.b, color.a)
}
