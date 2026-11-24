pub(super) struct Product {
    pub(super) brand: &'static str,
    pub(super) name: &'static str,
    pub(super) short: &'static str,
    pub(super) variants: &'static [&'static str],
}

const PRODUCTS: [Product; 49] = [
    Product::new(
        "Wooting",
        "Lekker V1/V2",
        "Lekker",
        &[
            "Lekker V1 L45",
            "Lekker V1 L60",
            "Lekker V2 L45",
            "Lekker V2 L60",
        ],
    ),
    Product::new(
        "Wooting",
        "Lekker Tikken",
        "Tikken",
        &[
            "Medium",
            "Light",
            "TenZ TikkenZ",
            "Heist Splits",
            "Heist OGs",
        ],
    ),
    Product::new("Gateron", "Jade", "Jade", &[]),
    Product::new("GEON", "Raw HE", "Raw HE", &[]),
    Product::new("TTC x Wooting", "KOM RGB", "W KOM R", &[]),
    Product::new("TTC x Wooting", "KOM POM", "W KOM P", &[]),
    Product::new("TTC", "KOM (King)", "King", &[]),
    Product::new("TTC", "KOM RGB", "KOM RGB", &[]),
    Product::new("TTC", "KOM POM", "KOM POM", &[]),
    Product::new("Gateron", "Jade Gaming", "Jade G", &[]),
    Product::new("Gateron", "Jade Pro", "Jade Pro", &[]),
    Product::new("Gateron", "Spark", "Spark", &["Regular", "Pink"]),
    Product::new("OwLab", "Ti HE", "Ti HE", &[]),
    Product::new(
        "PHYLINA",
        "ThunderFlash",
        "Thunder",
        &["White", "Black", "Pink"],
    ),
    Product::new(
        "Urion",
        "Ice Ultra / Extreme",
        "Ice",
        &["Ice Ultra", "Ice Ultra R2", "Ice Extreme"],
    ),
    Product::new(
        "Wuque Studio",
        "Flux",
        "Flux",
        &[
            "Clacky",
            "Deep Clacky",
            "Pink Lotus",
            "Ocean Whisper",
            "Jade",
        ],
    ),
    Product::new("Gateron", "Jade Ultra", "Jade U", &[]),
    Product::new("Gateron", "Jade Ruby", "Ruby", &[]),
    Product::new("MMD", "C1", "C1", &[]),
    Product::new("GEON", "Raptor HE V2", "RaptorV2", &[]),
    Product::new("Gateron", "Jade Emerald", "Emerald", &[]),
    Product::new("TTC", "KOM Flip", "Flip", &[]),
    Product::new(
        "TTC",
        "Horse",
        "Horse",
        &["White Horse", "Black Horse", "Noble Horse"],
    ),
    Product::new("Unionwell", "Xueyao (Snow Shine)", "Xueyao", &[]),
    Product::new("GravaStar", "UFO", "UFO", &["Purple", "White"]),
    Product::new("KeyTok", "Nova", "Nova", &[]),
    Product::new("HMX x Squashy Boy", "SquisHE", "SquisHE", &[]),
    Product::new(
        "Wuque Studio",
        "Diamond",
        "Diamond",
        &[
            "Heart of the Ocean",
            "Cullinan",
            "Sunshine Tears",
            "Star Kiss",
        ],
    ),
    Product::new("Akko", "Astrolink", "Astro", &[]),
    Product::new("HMX", "Gamer Fuel", "Fuel", &[]),
    Product::new(
        "Gateron",
        "Green Dragon",
        "Green D",
        &["Green", "Red", "Purple"],
    ),
    Product::new("Unionwell", "BingYao (Ice Shine)", "BingYao", &[]),
    Product::new("Wuque Studio", "Dash", "Dash", &[]),
    Product::new("Gateron", "Jade Max", "Jade Max", &[]),
    Product::new("Gateron", "KS-20 Dual-rail Orange", "KS-20", &[]),
    Product::new("Neo", "Lavender HE", "Lavender", &[]),
    Product::new("Everglide", "Wukong", "Wukong", &[]),
    Product::new("Gateron", "Jade Air", "Jade Air", &[]),
    Product::new("Gateron", "Jade Attraction", "Attract", &[]),
    Product::new("Gateron", "Jade Delta", "Delta", &["Dark", "Light", "Pink"]),
    Product::new("Gateron", "Jade Dragon", "Dragon", &[]),
    Product::new("Glorious", "Raptor HE", "Raptor", &[]),
    Product::new("TTC x YF x MelGeek", "Zero35", "Zero35", &[]),
    Product::new("TTC", "Lite KOM", "Lite KOM", &[]),
    Product::new("TTC", "Pro KOM", "Pro KOM", &[]),
    Product::new("OFF Studio & TNT", "Six Realms HiFi", "HiFi", &[]),
    Product::new("UGAMING", "Mount Tai HE", "Tai", &[]),
    Product::new("UGAMING", "Mount Tai GT HE", "Tai GT", &[]),
    Product::new("UGAMING", "Berserk", "Berserk", &[]),
];

impl Product {
    const fn new(
        brand: &'static str,
        name: &'static str,
        short: &'static str,
        variants: &'static [&'static str],
    ) -> Self {
        Self {
            brand,
            name,
            short,
            variants,
        }
    }
}

pub(super) fn product(model: u8) -> Option<&'static Product> {
    PRODUCTS.get(usize::from(model))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const FIRMWARE_MODELS: &str = include_str!("../../../../openwoot/src/analog/switch_model.rs");
    const MAX_SHORT_CHARS: usize = 8;

    fn firmware_model_count() -> u8 {
        FIRMWARE_MODELS
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("pub const COUNT: u8 = ")?
                    .strip_suffix(';')?
                    .parse()
                    .ok()
            })
            .expect("the firmware names its number of switch models")
    }

    #[test]
    fn every_switch_model_of_the_firmware_has_a_product_name() {
        let count = firmware_model_count();
        assert!(count > 0);
        for model in 0..count {
            let product = product(model).expect("the catalogue lists the model");
            assert_ne!(product.brand, "");
            assert_ne!(product.name, "");
            assert_ne!(product.short, "");
            assert!(product.short.chars().count() <= MAX_SHORT_CHARS);
        }
    }

    #[test]
    fn products_and_their_short_names_are_distinct() {
        let names: BTreeSet<_> = PRODUCTS
            .iter()
            .map(|product| (product.brand, product.name))
            .collect();
        let shorts: BTreeSet<_> = PRODUCTS.iter().map(|product| product.short).collect();
        assert_eq!(names.len(), PRODUCTS.len());
        assert_eq!(shorts.len(), PRODUCTS.len());
    }
}
