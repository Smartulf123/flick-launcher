//! The six public-domain pictures that ship inside Flick (credits in
//! `assets/backgrounds/CREDITS.md`).

/// One built-in picture. `id` is what `builtin:<id>` in the config names.
#[derive(Debug)]
pub struct Builtin {
    pub id: &'static str,
    pub title: &'static str,
    pub credit: &'static str,
    pub bytes: &'static [u8],
}

macro_rules! builtin {
    ($id:literal, $title:literal, $credit:literal) => {
        Builtin {
            id: $id,
            title: $title,
            credit: $credit,
            bytes: include_bytes!(concat!("../assets/backgrounds/", $id, ".jpg")),
        }
    };
}

/// In gallery order; the first is the default background.
pub const BUILTINS: [Builtin; 6] = [
    builtin!(
        "great-wave",
        "The Great Wave off Kanagawa",
        "Katsushika Hokusai"
    ),
    builtin!("red-fuji", "Red Fuji", "Katsushika Hokusai"),
    builtin!("starry-night", "The Starry Night", "Vincent van Gogh"),
    builtin!("impression-sunrise", "Impression, Sunrise", "Claude Monet"),
    builtin!("earthrise", "Earthrise", "NASA, Apollo 8"),
    builtin!(
        "pillars-of-creation",
        "Pillars of Creation",
        "NASA, ESA, Hubble Heritage Team"
    ),
];

pub fn find(id: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_builtins_in_menu_order() {
        let ids: Vec<_> = BUILTINS.iter().map(|b| b.id).collect();
        assert_eq!(
            ids,
            [
                "great-wave",
                "red-fuji",
                "starry-night",
                "impression-sunrise",
                "earthrise",
                "pillars-of-creation"
            ]
        );
    }

    #[test]
    fn every_builtin_decodes() {
        for b in &BUILTINS {
            let img = crate::background::decode(b.bytes).unwrap();
            assert!(img.size[0] <= 1280 && img.size[1] > 0, "{}", b.id);
        }
    }

    #[test]
    fn find_by_id() {
        assert_eq!(find("earthrise").unwrap().title, "Earthrise");
        assert!(find("nope").is_none());
    }
}
