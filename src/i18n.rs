use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub fn from_env() -> Self {
        let lang = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_default()
            .to_lowercase();
        if lang.starts_with("ru") {
            Self::Ru
        } else {
            Self::En
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::En => Self::Ru,
            Self::Ru => Self::En,
        }
    }
}

pub struct T;

impl T {
    pub fn app_title(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "coda",
            Lang::Ru => "coda",
        }
    }

    pub fn library(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Library",
            Lang::Ru => "Библиотека",
        }
    }

    pub fn queue(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Queue",
            Lang::Ru => "Очередь",
        }
    }

    pub fn playlists(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Playlists",
            Lang::Ru => "Плейлисты",
        }
    }

    pub fn help(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Help — keys",
            Lang::Ru => "Справка — клавиши",
        }
    }

    pub fn help_banner(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Keyboard shortcuts",
            Lang::Ru => "Горячие клавиши",
        }
    }

    pub fn help_dismiss(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Press Esc / Space / Enter / ? to close",
            Lang::Ru => "Esc / Space / Enter / ? — закрыть",
        }
    }

    pub fn footer_hints(lang: Lang) -> &'static str {
        match lang {
            Lang::En => {
                "Space play/pause · Enter play · n/p next/prev · ←/→ seek · -/= vol\n\
                 a add · s shuffle · / find · Tab panels · ? keys · q quit"
            }
            Lang::Ru => {
                "Space пауза · Enter играть · n/p след/пред · ←/→ перемотка · -/= громкость\n\
                 a в очередь · s перемешать · / поиск · Tab панели · ? клавиши · q выход"
            }
        }
    }

    pub fn playing(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Playing",
            Lang::Ru => "Играет",
        }
    }

    pub fn paused(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Paused",
            Lang::Ru => "Пауза",
        }
    }

    pub fn stopped(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Stopped",
            Lang::Ru => "Стоп",
        }
    }

    pub fn shuffle_on(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Shuffle ON",
            Lang::Ru => "Перемешать ВКЛ",
        }
    }

    pub fn shuffle_off(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Shuffle OFF",
            Lang::Ru => "Перемешать ВЫКЛ",
        }
    }

    pub fn repeat_off(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Repeat OFF",
            Lang::Ru => "Повтор ВЫКЛ",
        }
    }

    pub fn repeat_all(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Repeat ALL",
            Lang::Ru => "Повтор ВСЕ",
        }
    }

    pub fn repeat_one(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Repeat ONE",
            Lang::Ru => "Повтор ТРЕК",
        }
    }

    pub fn empty_library(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "No audio files in Music folder",
            Lang::Ru => "Нет аудиофайлов в папке Music",
        }
    }

    pub fn filter_prompt(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Filter: ",
            Lang::Ru => "Фильтр: ",
        }
    }

    pub fn save_playlist_prompt(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Playlist name: ",
            Lang::Ru => "Имя плейлиста: ",
        }
    }

    pub fn temp_playlist(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "(temporary queue)",
            Lang::Ru => "(временная очередь)",
        }
    }

    pub fn help_lines(lang: Lang) -> Vec<&'static str> {
        match lang {
            Lang::En => vec![
                "Space       play / pause",
                "Enter       play selected (or whole library from here)",
                "n / p       next / previous track",
                "← / →       seek −5s / +5s",
                "- / =       volume down / up",
                "s / S       shuffle / save playlist",
                "r / F5      cycle repeat / rescan library",
                "a           add track to queue",
                "c           clear queue",
                "l / d       load / delete playlist",
                "Tab         switch panel (Library / Queue / Playlists)",
                "/           filter library",
                "L           language EN ↔ RU",
                "?           show / hide this help",
                "q           quit",
                "",
                "Works on RU keyboard layout too (same physical keys).",
            ],
            Lang::Ru => vec![
                "Space       играть / пауза",
                "Enter       играть выбранное (или библиотеку отсюда)",
                "n/т  p/з    следующий / предыдущий",
                "← / →       −5с / +5с",
                "- / =       тише / громче",
                "s/ы  S/Ы    перемешать / сохранить плейлист",
                "r/к  F5     повтор / перескан",
                "a/ф         добавить в очередь",
                "c/с         очистить очередь",
                "l/д  d/в    загрузить / удалить плейлист",
                "Tab         панель (Библиотека / Очередь / Плейлисты)",
                "/ или .     фильтр библиотеки",
                "L/Д         язык EN ↔ RU",
                "?           показать / скрыть справку",
                "q/й         выход",
            ],
        }
    }
}
