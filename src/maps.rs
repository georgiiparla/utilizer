//! World-to-radar calibration matched to the vendored, uncropped SVG artwork.
pub struct Calibration {
    pub origin: [f64; 2],
    pub scale: f64,
    pub size: f64,
}

pub const MIRAGE: Calibration = Calibration {
    origin: [-3230.0, 1713.0],
    scale: 5.0,
    size: 1024.0,
};

pub struct Outline {
    pub path: &'static str,
    pub transform: &'static str,
}

pub struct Map {
    pub code: &'static str,
    pub name: &'static str,
    pub viewport: &'static str,
    pub calibration: Calibration,
    pub artwork: Option<&'static str>,
    pub dark_artwork: Option<&'static str>,
    pub outline: Option<Outline>,
}

pub const MAPS: [Map; 7] = [
    Map {
        viewport: "46 7 936 1006",
        code: "de_dust2",
        name: "Dust2",
        calibration: Calibration {
            origin: [-2476.0, 3239.0],
            scale: 4.4,
            size: 1024.0,
        },
        artwork: Some(include_str!("../assets/maps/dust2.svg")),
        dark_artwork: Some(include_str!("../assets/maps/dust2-dark.svg")),
        outline: Some(Outline {
            path: include_str!("../assets/maps/dust2.outline"),
            transform: "translate(52 12.5)",
        }),
    },
    Map {
        viewport: "95 148 858 736",
        code: "de_mirage",
        name: "Mirage",
        calibration: Calibration {
            origin: [-3230.0, 1713.0],
            scale: 5.0,
            size: 1024.0,
        },
        artwork: Some(include_str!("../assets/maps/mirage.svg")),
        dark_artwork: Some(include_str!("../assets/maps/mirage-dark.svg")),
        outline: Some(Outline {
            path: include_str!("../assets/maps/mirage.outline"),
            transform: "",
        }),
    },
    Map {
        viewport: "61 58 921 900",
        code: "de_inferno",
        name: "Inferno",
        calibration: Calibration {
            origin: [-2087.0, 3870.0],
            scale: 4.9,
            size: 1024.0,
        },
        artwork: Some(include_str!("../assets/maps/inferno.svg")),
        dark_artwork: Some(include_str!("../assets/maps/inferno-dark.svg")),
        outline: Some(Outline {
            path: include_str!("../assets/maps/inferno.outline"),
            transform: "translate(69 66)",
        }),
    },
    Map {
        viewport: "52 268 954 511",
        code: "de_nuke",
        name: "Nuke",
        calibration: Calibration {
            origin: [-3453.0, 2887.0],
            scale: 7.0,
            size: 1024.0,
        },
        artwork: Some(include_str!("../assets/maps/nuke.svg")),
        dark_artwork: Some(include_str!("../assets/maps/nuke-dark.svg")),
        outline: Some(Outline {
            path: include_str!("../assets/maps/nuke.outline"),
            transform: "",
        }),
    },
    Map {
        viewport: "118 68 764 879",
        code: "de_ancient",
        name: "Ancient",
        calibration: Calibration {
            origin: [-2953.0, 2164.0],
            scale: 5.0,
            size: 1024.0,
        },
        artwork: Some(include_str!("../assets/maps/ancient.svg")),
        dark_artwork: Some(include_str!("../assets/maps/ancient-dark.svg")),
        outline: Some(Outline {
            path: include_str!("../assets/maps/ancient.outline"),
            transform: "",
        }),
    },
    Map {
        viewport: "20 166 959 718",
        code: "de_cache",
        name: "Cache",
        calibration: Calibration {
            origin: [-2000.0, 3250.0],
            scale: 5.5,
            size: 1024.0,
        },
        artwork: Some(include_str!("../assets/maps/cache.svg")),
        dark_artwork: Some(include_str!("../assets/maps/cache-dark.svg")),
        outline: Some(Outline {
            path: include_str!("../assets/maps/cache.outline"),
            transform: "",
        }),
    },
    Map {
        viewport: "141 17 754 978",
        code: "de_anubis",
        name: "Anubis",
        calibration: Calibration {
            origin: [-2796.0, 3328.0],
            scale: 5.22,
            size: 1024.0,
        },
        artwork: Some(include_str!("../assets/maps/anubis.svg")),
        dark_artwork: Some(include_str!("../assets/maps/anubis-dark.svg")),
        outline: Some(Outline {
            path: include_str!("../assets/maps/anubis.outline"),
            transform: "",
        }),
    },
];
pub fn find(code: &str) -> Option<&'static Map> {
    MAPS.iter().find(|map| map.code == code)
}

pub const ZONE_SIZE: f64 = 32.0;

/// Connected unions of compact squares; do not join separate height bands.
/// This groups starting positions, never grenade coverage or walkable geometry.
pub fn groups(points: &[(usize, [f64; 3])]) -> Vec<Vec<usize>> {
    let mut remaining: Vec<_> = (0..points.len()).collect();
    let mut result = Vec::new();
    while let Some(seed) = remaining.pop() {
        let mut group = vec![seed];
        let mut cursor = 0;
        while cursor < group.len() {
            let a = points[group[cursor]].1;
            let mut i = 0;
            while i < remaining.len() {
                let b = points[remaining[i]].1;
                if (a[0] - b[0]).abs() <= ZONE_SIZE
                    && (a[1] - b[1]).abs() <= ZONE_SIZE
                    && (a[2] - b[2]).abs() <= 64.0
                {
                    group.push(remaining.swap_remove(i));
                } else {
                    i += 1;
                }
            }
            cursor += 1;
        }
        let mut members: Vec<_> = group.into_iter().map(|i| points[i].0).collect();
        members.sort_unstable();
        result.push(members);
    }
    result.sort_by_key(|group| group[0]);
    result
}

impl Calibration {
    /// Ignore altitude for this single-floor prototype. Never clamp invalid points.
    pub fn project(&self, [x, y, z]: [f64; 3]) -> Option<[f64; 2]> {
        if ![
            x,
            y,
            z,
            self.origin[0],
            self.origin[1],
            self.scale,
            self.size,
        ]
        .iter()
        .all(|n| n.is_finite())
            || self.scale <= 0.0
            || self.size <= 0.0
        {
            return None;
        }
        let point = [
            (x - self.origin[0]) / self.scale,
            (self.origin[1] - y) / self.scale,
        ];
        point
            .iter()
            .all(|n| (0.0..=self.size).contains(n))
            .then_some(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_match_the_bundled_base_map_clips_including_translations() {
        for map in &MAPS {
            assert!(map.artwork.is_some() && map.dark_artwork.is_some() && map.outline.is_some());
            for artwork in [map.artwork, map.dark_artwork] {
                match (&artwork, &map.outline) {
                    (Some(artwork), Some(outline)) => {
                        let clip = artwork
                            .split_once("<clipPath id=\"paint-1\">")
                            .unwrap()
                            .1
                            .split_once("</clipPath>")
                            .unwrap()
                            .0;
                        assert!(
                            clip.contains(&format!("d=\"{}\"", outline.path)),
                            "{}",
                            map.code
                        );
                        assert!(clip.contains("clip-rule=\"evenodd\""));
                        if !outline.transform.is_empty() {
                            assert!(clip.contains(&format!("transform=\"{}\"", outline.transform)));
                        } else {
                            assert!(!clip.contains("transform="));
                        }
                    }
                    (None, None) => {}
                    _ => panic!("{} must bundle artwork and its outline together", map.code),
                }
            }
        }
    }

    #[test]
    fn mirage_origin_scale_y_axis_and_edges_match_artwork() {
        assert_eq!(MIRAGE.project([-3230.0, 1713.0, 0.0]), Some([0.0, 0.0]));
        assert_eq!(MIRAGE.project([-730.0, -787.0, 64.0]), Some([500.0, 500.0]));
        assert_eq!(
            MIRAGE.project([1890.0, -3407.0, -100.0]),
            Some([1024.0, 1024.0])
        );
        assert_eq!(MIRAGE.project([0.0, 0.0, 0.0]), Some([646.0, 342.6]));
    }

    #[test]
    fn invalid_and_outside_positions_are_not_misleadingly_clamped() {
        for position in [
            [-3231.0, 1713.0, 0.0],
            [1891.0, 0.0, 0.0],
            [0.0, 1714.0, 0.0],
            [0.0, -3408.0, 0.0],
            [f64::INFINITY, 0.0, 0.0],
            [0.0, f64::NAN, 0.0],
            [0.0, 0.0, f64::NAN],
        ] {
            assert_eq!(MIRAGE.project(position), None);
        }
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;
    #[test]
    fn connected_squares_handle_singletons_duplicates_chains_and_height_separation() {
        assert!(groups(&[]).is_empty());
        assert_eq!(groups(&[(9, [2.0, 2.0, 0.0])]), vec![vec![9]]);
        assert_eq!(
            groups(&[
                (0, [0.0, 0.0, 0.0]),
                (1, [0.0, 0.0, 0.0]),
                (2, [28.0, 0.0, 0.0]),
                (3, [56.0, 0.0, 0.0]),
                (4, [400.0, 400.0, 0.0]),
                (5, [0.0, 0.0, 300.0])
            ]),
            vec![vec![0, 1, 2, 3], vec![4], vec![5]]
        );
        assert_eq!(MAPS.len(), 7);
        assert!(find("cs_office").is_none());
        assert_eq!(
            groups(&[
                (0, [0.0, 0.0, 0.0]),
                (1, [32.0, 0.0, 0.0]),
                (2, [64.01, 0.0, 0.0]),
                (3, [32.0, 0.0, 64.01]),
            ]),
            vec![vec![0, 1], vec![2], vec![3]]
        );
        assert_eq!(
            groups(&[(0, [0.0, 0.0, 0.0]), (1, [32.0, 32.0, 0.0])]),
            vec![vec![0, 1]]
        );
        assert_eq!(
            groups(&[(0, [0.0, 0.0, 0.0]), (1, [30.0, 30.0, 64.0])]),
            vec![vec![0, 1]]
        );
    }
}
