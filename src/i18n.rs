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

    pub fn hints_library(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Enter play · a queue · / find · Tab panel · ' now · ? · q",
            Lang::Ru => "Enter играть · a в очередь · / поиск · Tab панель · ' к треку · ? · q",
        }
    }

    pub fn hints_queue(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Enter play · x drop · Shift↑↓ move · s shuffle · ' now · ? · q",
            Lang::Ru => {
                "Enter играть · x убрать · Shift↑↓ двигать · s перемешать · ' к треку · ? · q"
            }
        }
    }

    pub fn hints_playlists(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "Enter open · d delete · l load · ? · q",
            Lang::Ru => "Enter открыть · d удалить · l загрузить · ? · q",
        }
    }

    pub fn up_next(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "next",
            Lang::Ru => "далее",
        }
    }

    pub fn empty_queue(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "press a in the library to queue a track",
            Lang::Ru => "a в библиотеке — добавить трек",
        }
    }

    pub fn empty_playlists(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "S saves the queue as a playlist",
            Lang::Ru => "S — сохранить очередь как плейлист",
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
            Lang::En => "No audio files in the music folder",
            Lang::Ru => "В папке с музыкой нет аудиофайлов",
        }
    }

    pub fn queue_cleared(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "queue cleared",
            Lang::Ru => "очередь очищена",
        }
    }

    pub fn track_removed(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "removed from queue",
            Lang::Ru => "убрано из очереди",
        }
    }

    pub fn volume(lang: Lang, pct: u8) -> String {
        match lang {
            Lang::En => format!("vol {pct}%"),
            Lang::Ru => format!("громкость {pct}%"),
        }
    }

    pub fn theme(lang: Lang, name: &str) -> String {
        match lang {
            Lang::En => format!("theme: {name}"),
            Lang::Ru => format!("тема: {name}"),
        }
    }

    pub fn scanned(lang: Lang, n: usize) -> String {
        match lang {
            Lang::En => format!("scanned {n} tracks"),
            Lang::Ru => format!("в библиотеке {n} треков"),
        }
    }

    pub fn saved(lang: Lang, path: &str) -> String {
        match lang {
            Lang::En => format!("saved {path}"),
            Lang::Ru => format!("сохранено {path}"),
        }
    }

    pub fn save_failed(lang: Lang, err: &str) -> String {
        match lang {
            Lang::En => format!("save failed: {err}"),
            Lang::Ru => format!("не сохранилось: {err}"),
        }
    }

    pub fn loaded(lang: Lang, name: &str) -> String {
        match lang {
            Lang::En => format!("loaded {name}"),
            Lang::Ru => format!("открыт {name}"),
        }
    }

    pub fn load_failed(lang: Lang, err: &str) -> String {
        match lang {
            Lang::En => format!("load failed: {err}"),
            Lang::Ru => format!("не открылось: {err}"),
        }
    }

    pub fn deleted(lang: Lang, name: &str) -> String {
        match lang {
            Lang::En => format!("deleted {name}"),
            Lang::Ru => format!("удалён {name}"),
        }
    }

    pub fn delete_failed(lang: Lang, err: &str) -> String {
        match lang {
            Lang::En => format!("delete failed: {err}"),
            Lang::Ru => format!("не удалилось: {err}"),
        }
    }

    pub fn delete_confirm(lang: Lang, name: &str) -> String {
        match lang {
            Lang::En => format!("press d again to delete {name}"),
            Lang::Ru => format!("ещё раз d — удалить {name}"),
        }
    }

    pub fn play_error(lang: Lang, err: &str) -> String {
        match lang {
            Lang::En => format!("play error: {err}"),
            Lang::Ru => format!("ошибка воспроизведения: {err}"),
        }
    }

    pub fn seek_failed(lang: Lang) -> &'static str {
        match lang {
            Lang::En => "seek failed",
            Lang::Ru => "перемотка не вышла",
        }
    }

    pub fn added(lang: Lang, name: &str) -> String {
        match lang {
            Lang::En => format!("+ {name}"),
            Lang::Ru => format!("+ {name}"),
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

    pub fn help_lines(lang: Lang) -> Vec<&'static str> {
        match lang {
            Lang::En => vec![
                "Space       play / pause",
                "Enter       play the visible list from the selection",
                "n / p       next / previous track",
                "← / →       seek −5s / +5s",
                "- / =       volume down / up",
                "s / S       shuffle / save playlist",
                "r / F5      cycle repeat / rescan library",
                "a           add track to queue",
                "x / Delete  remove the selected queue track",
                "Shift↑↓     move the selected queue track",
                "c           clear queue and stop",
                "l / d       load playlist / delete (press d twice)",
                "Tab / S-Tab switch panel / previous panel",
                "'           jump to the playing track",
                "click       select · double-click plays · click the bar to seek",
                "/           filter library (updates as you type)",
                "L           language EN ↔ RU",
                "t           theme glass ↔ light",
                "?           show / hide this help",
                "q           quit",
                "",
                "Works on RU keyboard layout too (same physical keys).",
            ],
            Lang::Ru => vec![
                "Space       играть / пауза",
                "Enter       играть видимый список от выбранного",
                "n/т  p/з    следующий / предыдущий",
                "← / →       −5с / +5с",
                "- / =       тише / громче",
                "s/ы  S/Ы    перемешать / сохранить плейлист",
                "r/к  F5     повтор / перескан",
                "a/ф         добавить в очередь",
                "x/ч  Delete убрать выбранное из очереди",
                "Shift↑↓     подвинуть трек в очереди",
                "c/с         очистить очередь и остановить",
                "l/д  d/в    открыть плейлист / удалить (d дважды)",
                "Tab / S-Tab панель / предыдущая панель",
                "' / э       к играющему треку",
                "клик        выбрать · двойной клик играет · клик по шкале",
                "/ или .     фильтр, список сужается сразу",
                "L/Д         язык EN ↔ RU",
                "t/е         тема glass ↔ light",
                "?           показать / скрыть справку",
                "q/й         выход",
            ],
        }
    }
}
