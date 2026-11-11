use crate::domain::model::{Centimeters, Item};

/// Usable volume is geometric volume times this factor.
/// A drawer counts as full before it is packed solid.
pub const PACKING_EFFICIENCY: f64 = 0.65;

/// A category that would fill less than this fraction of its own container
/// may share a container with another category in the same group.
pub const SMALL_SHARE_MAX: f64 = 0.10;

/// A host container can take a small category when at least this fraction
/// of its usable volume is still free.
pub const HOST_MIN_FREE: f64 = 0.50;

const FIT_EPSILON: f64 = 1e-6;

impl Centimeters {
    pub fn geometric_volume_cm3(self) -> f64 {
        self.width * self.height * self.depth
    }

    pub fn usable_volume_cm3(self) -> f64 {
        self.geometric_volume_cm3() * PACKING_EFFICIENCY
    }

    /// True when the item fits in some rotation of its three axes.
    pub fn fits(self, item: Centimeters) -> bool {
        let container = [self.width, self.height, self.depth];
        let item = [item.width, item.height, item.depth];
        let permutations = [
            [item[0], item[1], item[2]],
            [item[0], item[2], item[1]],
            [item[1], item[0], item[2]],
            [item[1], item[2], item[0]],
            [item[2], item[0], item[1]],
            [item[2], item[1], item[0]],
        ];
        permutations.into_iter().any(|perm| {
            perm[0] <= container[0] + FIT_EPSILON
                && perm[1] <= container[1] + FIT_EPSILON
                && perm[2] <= container[2] + FIT_EPSILON
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Usage {
    pub used_cm3: f64,
    pub usable_cm3: f64,
    pub geometric_cm3: f64,
}

impl Usage {
    pub fn fraction(self) -> f64 {
        if self.usable_cm3 <= FIT_EPSILON {
            return if self.used_cm3 > FIT_EPSILON {
                1.0
            } else {
                0.0
            };
        }
        self.used_cm3 / self.usable_cm3
    }
}

pub fn usage_of(size: Centimeters, items: &[&Item]) -> Usage {
    // f64's Iterator::sum starts at -0.0, so an empty container would display as "-0.0 L".
    let used_cm3 = items
        .iter()
        .fold(0.0, |total, item| total + item.size.geometric_volume_cm3());
    Usage {
        used_cm3,
        usable_cm3: size.usable_volume_cm3(),
        geometric_cm3: size.geometric_volume_cm3(),
    }
}

pub fn format_liters(cm3: f64) -> String {
    format!("{:.1} L", cm3 / 1000.0)
}

pub fn volume_fits(size: Centimeters, used_cm3: f64, needed_cm3: f64) -> bool {
    needed_cm3 <= size.usable_volume_cm3() - used_cm3 + FIT_EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cm(width: f64, height: f64, depth: f64) -> Centimeters {
        Centimeters {
            width,
            height,
            depth,
        }
    }

    #[test]
    fn rotation_lets_a_long_item_fit() {
        let drawer = cm(10.0, 20.0, 30.0);
        assert!(drawer.fits(cm(30.0, 10.0, 20.0)));
        assert!(!drawer.fits(cm(31.0, 10.0, 20.0)));
    }

    #[test]
    fn volume_can_fit_when_the_box_does_not() {
        let drawer = cm(10.0, 10.0, 10.0);
        let item = cm(50.0, 2.0, 1.0);
        assert!(volume_fits(drawer, 0.0, item.geometric_volume_cm3()));
        assert!(!drawer.fits(item));
    }

    #[test]
    fn usable_volume_applies_the_packing_factor() {
        let drawer = cm(10.0, 10.0, 10.0);
        assert!((drawer.geometric_volume_cm3() - 1000.0).abs() < 1e-9);
        assert!((drawer.usable_volume_cm3() - 650.0).abs() < 1e-6);
    }

    #[test]
    fn empty_container_usage_reads_as_zero() {
        let usage = usage_of(cm(30.0, 10.0, 30.0), &[]);
        let text = format!(
            "{} used of {} usable ({:.0}%)",
            format_liters(usage.used_cm3),
            format_liters(usage.usable_cm3),
            usage.fraction() * 100.0
        );
        assert_eq!(
            text,
            "0.0 L used of 5.8 L usable (0%)",
            "used bits {:x} fraction bits {:x}",
            usage.used_cm3.to_bits(),
            usage.fraction().to_bits()
        );
    }
}
