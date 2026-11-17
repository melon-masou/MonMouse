use eframe::egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use fontdb::{Database, Family, Query, Source};

use crate::styles::gscale;

fn families_for_lang(lang: &str) -> &'static [&'static str] {
    match lang {
        "zh-Hans" => &["Microsoft YaHei", "SimHei", "Noto Sans CJK SC"],
        "ja" => &["Meiryo", "MS Gothic", "Noto Sans CJK JP"],
        _ => &[],
    }
}

fn find_font_by_families(db: &Database, families: &[&str]) -> Option<(String, Vec<u8>)> {
    for fam in families {
        let query = Query {
            families: &[Family::Name(fam)],
            ..Default::default()
        };

        if let Some(id) = db.query(&query) {
            let face = db.face(id)?;
            if let Source::File(path) = &face.source {
                let data = std::fs::read(path).ok()?;
                let name = face.post_script_name.clone();
                return Some((name, data));
            }
        }
    }
    None
}

pub fn setup_fonts(defs: &mut FontDefinitions) {
    let mut db = Database::new();
    db.load_system_fonts();

    for lang in rust_i18n::available_locales!() {
        let families = families_for_lang(&lang);
        let Some((font_name, bytes)) = find_font_by_families(&db, families) else {
            continue;
        };

        if !defs.font_data.contains_key(&font_name) {
            let font_data = FontData::from_owned(bytes).tweak(FontTweak {
                scale: gscale(1.0),
                ..Default::default()
            });
            defs.font_data.insert(font_name.clone(), font_data.into());
        }
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let fonts = defs.families.get_mut(&family).unwrap();
            if !fonts.contains(&font_name) {
                fonts.push(font_name.clone());
            }
        }
    }
}
