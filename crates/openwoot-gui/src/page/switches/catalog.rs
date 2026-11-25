pub(super) struct Product {
    pub(super) brand: &'static str,
    pub(super) name: &'static str,
    pub(super) variants: &'static [&'static str],
}

const PRODUCTS: [Product; 49] = [
    Product::new(
        "Wooting",
        "Lekker V1/V2",
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
        &[
            "Medium",
            "Light",
            "TenZ TikkenZ",
            "Heist Splits",
            "Heist OGs",
        ],
    ),
    Product::new("Gateron", "Jade", &[]),
    Product::new("GEON", "Raw HE", &[]),
    Product::new("TTC x Wooting", "KOM RGB", &[]),
    Product::new("TTC x Wooting", "KOM POM", &[]),
    Product::new("TTC", "KOM (King)", &[]),
    Product::new("TTC", "KOM RGB", &[]),
    Product::new("TTC", "KOM POM", &[]),
    Product::new("Gateron", "Jade Gaming", &[]),
    Product::new("Gateron", "Jade Pro", &[]),
    Product::new("Gateron", "Spark", &["Regular", "Pink"]),
    Product::new("OwLab", "Ti HE", &[]),
    Product::new("PHYLINA", "ThunderFlash", &["White", "Black", "Pink"]),
    Product::new(
        "Urion",
        "Ice Ultra / Extreme",
        &["Ice Ultra", "Ice Ultra R2", "Ice Extreme"],
    ),
    Product::new(
        "Wuque Studio",
        "Flux",
        &[
            "Clacky",
            "Deep Clacky",
            "Pink Lotus",
            "Ocean Whisper",
            "Jade",
        ],
    ),
    Product::new("Gateron", "Jade Ultra", &[]),
    Product::new("Gateron", "Jade Ruby", &[]),
    Product::new("MMD", "C1", &[]),
    Product::new("GEON", "Raptor HE V2", &[]),
    Product::new("Gateron", "Jade Emerald", &[]),
    Product::new("TTC", "KOM Flip", &[]),
    Product::new(
        "TTC",
        "Horse",
        &["White Horse", "Black Horse", "Noble Horse"],
    ),
    Product::new("Unionwell", "Xueyao (Snow Shine)", &[]),
    Product::new("GravaStar", "UFO", &["Purple", "White"]),
    Product::new("KeyTok", "Nova", &[]),
    Product::new("HMX x Squashy Boy", "SquisHE", &[]),
    Product::new(
        "Wuque Studio",
        "Diamond",
        &[
            "Heart of the Ocean",
            "Cullinan",
            "Sunshine Tears",
            "Star Kiss",
        ],
    ),
    Product::new("Akko", "Astrolink", &[]),
    Product::new("HMX", "Gamer Fuel", &[]),
    Product::new("Gateron", "Green Dragon", &["Green", "Red", "Purple"]),
    Product::new("Unionwell", "BingYao (Ice Shine)", &[]),
    Product::new("Wuque Studio", "Dash", &[]),
    Product::new("Gateron", "Jade Max", &[]),
    Product::new("Gateron", "KS-20 Dual-rail Orange", &[]),
    Product::new("Neo", "Lavender HE", &[]),
    Product::new("Everglide", "Wukong", &[]),
    Product::new("Gateron", "Jade Air", &[]),
    Product::new("Gateron", "Jade Attraction", &[]),
    Product::new("Gateron", "Jade Delta", &["Dark", "Light", "Pink"]),
    Product::new("Gateron", "Jade Dragon", &[]),
    Product::new("Glorious", "Raptor HE", &[]),
    Product::new("TTC x YF x MelGeek", "Zero35", &[]),
    Product::new("TTC", "Lite KOM", &[]),
    Product::new("TTC", "Pro KOM", &[]),
    Product::new("OFF Studio & TNT", "Six Realms HiFi", &[]),
    Product::new("UGAMING", "Mount Tai HE", &[]),
    Product::new("UGAMING", "Mount Tai GT HE", &[]),
    Product::new("UGAMING", "Berserk", &[]),
];

impl Product {
    const fn new(
        brand: &'static str,
        name: &'static str,
        variants: &'static [&'static str],
    ) -> Self {
        Self {
            brand,
            name,
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

    #[test]
    fn every_switch_model_has_a_product_name() {
        for model in 0..openwoot_protocol::switches::MODEL_COUNT {
            let product = product(model).expect("the catalogue lists the model");
            assert_ne!(product.brand, "");
            assert_ne!(product.name, "");
        }
    }

    #[test]
    fn products_are_distinct() {
        let names: BTreeSet<_> = PRODUCTS
            .iter()
            .map(|product| (product.brand, product.name))
            .collect();
        assert_eq!(names.len(), PRODUCTS.len());
    }
}
