//! 多语言与无障碍（B-25）：
//! - 中/英界面切换（L 键）；界面文案统一走 t(zh, en) 双语对
//! - 无障碍原则（REVIEW P7）：关键反馈不依赖纯颜色（幽灵无效态叠加脉冲）
//!
//! 说明：积木名称与文化描述属内容（博物馆式标签），保持中文；
//! 界面镀铬（HUD/面板/教程/成就）双语化。

use crate::ui::input::{shortcuts_allowed, InputOwnership};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Lang {
    #[default]
    Zh,
    En,
}

/// 当前界面语言。
#[derive(Resource, Default)]
pub struct Locale {
    pub lang: Lang,
}

pub struct I18nPlugin;

impl Plugin for I18nPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Locale::default())
            .add_systems(Update, toggle_lang);
    }
}

/// L 键切换中/英。
fn toggle_lang(
    ownership: Option<Res<InputOwnership>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut locale: ResMut<Locale>,
) {
    if shortcuts_allowed(&keys, ownership.as_deref()) && keys.just_pressed(KeyCode::KeyL) {
        locale.lang = match locale.lang {
            Lang::Zh => Lang::En,
            Lang::En => Lang::Zh,
        };
    }
}

/// 双语选择（zh/en 均为字符串字面量）。
pub fn t(zh: &'static str, en: &'static str, lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => zh,
        Lang::En => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_selects_by_lang() {
        assert_eq!(t("积木", "Blocks", Lang::Zh), "积木");
        assert_eq!(t("积木", "Blocks", Lang::En), "Blocks");
    }

    #[test]
    fn lang_toggle_cycles() {
        let mut l = Locale::default();
        assert_eq!(l.lang, Lang::Zh);
        l.lang = match l.lang {
            Lang::Zh => Lang::En,
            Lang::En => Lang::Zh,
        };
        assert_eq!(l.lang, Lang::En);
    }
}
