//! Bounded, opt-in player workshop catalog. These are new recipes, not Crystal
//! recipe imports. Fashion definitions contain presentation only, never stats.
use crate::LanguageCode;
use mir2_protocol::{MirClass, MirGender};

pub const CATALOG_VERSION: u16 = 1;
pub const RECEIPT_LIMIT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    Jewellery(i32),
    Fashion(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ingredient {
    pub template_id: i32,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recipe {
    pub id: &'static str,
    pub names: [&'static str; 4],
    pub ingredients: &'static [Ingredient],
    pub gold: u32,
    pub product: Product,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fashion {
    pub id: &'static str,
    pub names: [&'static str; 4],
    pub armour_shape: u16,
    pub male_state_image: u32,
    pub female_state_image: u32,
    /// The local classic library is complete; this style's alternate Assassin
    /// and Archer libraries are not shipped, so those classes cannot select it.
    pub classic_classes_only: bool,
}

fn localized(names: [&'static str; 4], language: LanguageCode) -> &'static str {
    names[match language {
        LanguageCode::English => 0,
        LanguageCode::ChineseSimplified => 1,
        LanguageCode::Portuguese => 2,
        LanguageCode::Spanish => 3,
    }]
}

impl Recipe {
    pub fn name(&self, language: LanguageCode) -> &'static str {
        localized(self.names, language)
    }

    pub fn supports_class(&self, class: MirClass) -> bool {
        match self.product {
            Product::Jewellery(_) => true,
            Product::Fashion(id) => fashion(id).is_some_and(|style| style.supports_class(class)),
        }
    }
}

impl Fashion {
    pub fn name(&self, language: LanguageCode) -> &'static str {
        localized(self.names, language)
    }

    pub fn supports_class(&self, class: MirClass) -> bool {
        !self.classic_classes_only
            || matches!(
                class,
                MirClass::Warrior | MirClass::Wizard | MirClass::Taoist
            )
    }

    pub fn state_image(&self, gender: MirGender) -> u32 {
        match gender {
            MirGender::Male => self.male_state_image,
            MirGender::Female => self.female_state_image,
        }
    }
}

pub const FASHIONS: &[Fashion] = &[
    Fashion {
        id: "traveler",
        names: [
            "Traveler attire",
            "旅人时装",
            "Traje de viajante",
            "Atuendo de viajero",
        ],
        armour_shape: 1,
        male_state_image: 60,
        female_state_image: 80,
        classic_classes_only: false,
    },
    Fashion {
        id: "jade-gala",
        names: [
            "Jade gala attire",
            "玉晶礼装",
            "Traje de gala de jade",
            "Atuendo de gala de jade",
        ],
        armour_shape: 10,
        male_state_image: 595,
        female_state_image: 605,
        classic_classes_only: true,
    },
];

pub const RECIPES: &[Recipe] = &[
    Recipe {
        id: "copper-ring",
        names: ["Copper ring", "铜戒指", "Anel de cobre", "Anillo de cobre"],
        ingredients: &[Ingredient {
            template_id: 824,
            count: 1,
        }],
        gold: 300,
        product: Product::Jewellery(404),
    },
    Recipe {
        id: "silver-bracelet",
        names: [
            "Silver bracelet",
            "银手镯",
            "Bracelete de prata",
            "Pulsera de plata",
        ],
        ingredients: &[Ingredient {
            template_id: 826,
            count: 1,
        }],
        gold: 750,
        product: Product::Jewellery(479),
    },
    Recipe {
        id: "traveler",
        names: FASHIONS[0].names,
        ingredients: &[Ingredient {
            template_id: 824,
            count: 1,
        }],
        gold: 500,
        product: Product::Fashion("traveler"),
    },
    Recipe {
        id: "jade-gala",
        names: FASHIONS[1].names,
        ingredients: &[
            Ingredient {
                template_id: 824,
                count: 1,
            },
            Ingredient {
                template_id: 904,
                count: 1,
            },
        ],
        gold: 1_500,
        product: Product::Fashion("jade-gala"),
    },
];

pub fn recipe(id: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|entry| entry.id == id)
}

pub fn fashion(id: &str) -> Option<&'static Fashion> {
    FASHIONS.iter().find(|entry| entry.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn atelier_recipes_are_available_in_both_profiles_without_enabling_stage5_craft() {
        for profile in [
            crate::platinum_176_profile(),
            crate::platinum_fiveclass_profile(),
        ] {
            assert!(profile
                .disabled_stage5_action_prefixes
                .iter()
                .any(|p| p == "craft."));
            for entry in RECIPES {
                let mut templates: Vec<_> =
                    entry.ingredients.iter().map(|i| i.template_id).collect();
                if let Product::Jewellery(index) = entry.product {
                    templates.push(index);
                }
                for index in templates {
                    let item = crate::crystal_item_by_index(index).unwrap();
                    assert!(
                        profile.item_whitelist.contains(&item.name),
                        "{} omits {}",
                        profile.profile_id,
                        item.name
                    );
                }
            }
        }
    }

    #[test]
    fn atelier_catalog_uses_real_materials_and_original_jewellery_without_gold_arbitrage() {
        let mut ids = BTreeSet::new();
        for entry in RECIPES {
            assert!(ids.insert(entry.id));
            assert!(entry.gold > 0 && !entry.ingredients.is_empty());
            for input in entry.ingredients {
                assert!(input.count > 0);
                assert!(crate::crystal_item_by_index(input.template_id).is_some());
            }
            if let Product::Jewellery(index) = entry.product {
                let output = crate::crystal_item_by_index(index).unwrap();
                assert_eq!(output.stack_size, 1);
                assert_eq!(output.bind, 0);
                assert!(!output.stats.is_empty());
                // The fee alone exceeds the original full-durability NPC sell
                // value, even before accounting for real consumed ore.
                assert!(entry.gold > output.price / 2);
            }
        }
    }

    #[test]
    fn atelier_fashion_art_matches_real_gender_templates_and_refuses_missing_alternate_libraries() {
        for (style, male, female) in [(&FASHIONS[0], 317, 318), (&FASHIONS[1], 45, 51)] {
            for (gender, index) in [(MirGender::Male, male), (MirGender::Female, female)] {
                let art = crate::crystal_item_by_index(index).unwrap();
                assert_eq!(u16::try_from(art.shape).unwrap(), style.armour_shape);
                assert_eq!(u32::from(art.image), style.state_image(gender));
                // These templates are art references; their original defence
                // is deliberately absent from Fashion and Product::Fashion.
                assert!(!art.stats.is_empty());
            }
        }
        assert!(FASHIONS[0].supports_class(MirClass::Assassin));
        assert!(FASHIONS[0].supports_class(MirClass::Archer));
        assert!(!FASHIONS[1].supports_class(MirClass::Assassin));
        assert!(!FASHIONS[1].supports_class(MirClass::Archer));
        for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
            assert!(FASHIONS[1].supports_class(class));
        }
    }
}
