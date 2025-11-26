use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontTweak};
use fontdb::{Database, Family, Query, Source};

use crate::styles::gscale;

fn families_for_lang(lang: &str) -> &'static [&'static str] {
    match lang {
        "zh-Hans" => &["Microsoft YaHei", "SimHei", "Noto Sans CJK SC"],
        "ja" => &["Meiryo", "MS Gothic", "Noto Sans CJK JP"],
        _ => &[],
    }
}

fn find_font_by_families(families: &[&str]) -> Option<(String, Vec<u8>)> {
    let mut db = Database::new();
    db.load_system_fonts();

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

pub fn setup_fonts_for_lang(ctx: &egui::Context, lang: &str) {
    let mut defs = ctx.fonts(|f| f.definitions().clone());
    setup_fonts_for_lang_defs(&mut defs, lang);
    ctx.set_fonts(defs);
}

pub fn setup_fonts_for_lang_defs(defs: &mut FontDefinitions, lang: &str) {
    let families = families_for_lang(lang);

    let Some((font_name, bytes)) = find_font_by_families(families) else {
        return;
    };

    if !defs.font_data.contains_key(&font_name) {
        let font_data = FontData::from_owned(bytes).tweak(FontTweak {
            scale: gscale(1.0),
            ..Default::default()
        });
        defs.font_data.insert(font_name.clone(), font_data.into());
    }
    if !defs.families[&FontFamily::Proportional].contains(&font_name) {
        defs.families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, font_name.clone());
    }
    if !defs.families[&FontFamily::Monospace].contains(&font_name) {
        defs.families
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .insert(0, font_name.clone());
    }
}
