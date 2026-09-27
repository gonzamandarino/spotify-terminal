//! Combinación de teclas (modificadores + tecla), común a los atajos de la
//! ventana y a los globales (spec 007). Se escribe como en los menús:
//! `"Ctrl+Alt+P"`, `"Ctrl+→"`, `"Ctrl++"`.

use std::{fmt, str::FromStr};

/// Tecla de una combinación, sin los modificadores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    /// Letra A-Z o dígito 0-9, en mayúscula.
    Char(char),
    /// F1-F12.
    F(u8),
    Enter,
    Space,
    Tab,
    Escape,
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Plus,
    Minus,
}

/// Nombres de las teclas con nombre: el primero es el que se muestra; el
/// resto también se acepta al leer (ajustes escritos a mano).
const NAMES: &[(Key, &[&str])] = &[
    (Key::Enter, &["Enter", "Intro"]),
    (Key::Space, &["Espacio", "Space"]),
    (Key::Tab, &["Tab"]),
    (Key::Escape, &["Esc", "Escape"]),
    (Key::Backspace, &["Retroceso", "Backspace"]),
    (Key::Left, &["←", "Left"]),
    (Key::Right, &["→", "Right"]),
    (Key::Up, &["↑", "Up"]),
    (Key::Down, &["↓", "Down"]),
    (Key::Home, &["Inicio", "Home"]),
    (Key::End, &["Fin", "End"]),
    (Key::PageUp, &["RePág", "PageUp"]),
    (Key::PageDown, &["AvPág", "PageDown"]),
    (Key::Insert, &["Insert", "Ins"]),
    (Key::Delete, &["Supr", "Delete", "Del"]),
    (Key::Plus, &["+", "Plus"]),
    (Key::Minus, &["-", "Minus"]),
];

impl Key {
    /// Virtual-key code de Windows. Letras y dígitos son su código ASCII
    /// en mayúscula; `+` y `-` son las teclas de la fila principal.
    pub(crate) fn vk(self) -> u32 {
        match self {
            Key::Char(c) => u32::from(c.to_ascii_uppercase()),
            Key::F(n) => 0x70 + u32::from(n) - 1,
            Key::Enter => 0x0D,
            Key::Space => 0x20,
            Key::Tab => 0x09,
            Key::Escape => 0x1B,
            Key::Backspace => 0x08,
            Key::Left => 0x25,
            Key::Up => 0x26,
            Key::Right => 0x27,
            Key::Down => 0x28,
            Key::Home => 0x24,
            Key::End => 0x23,
            Key::PageUp => 0x21,
            Key::PageDown => 0x22,
            Key::Insert => 0x2D,
            Key::Delete => 0x2E,
            Key::Plus => 0xBB,
            Key::Minus => 0xBD,
        }
    }

    /// La tecla de egui que la representa. `+` también coincide con `=`
    /// (misma tecla física en teclados en inglés; ver `matches_egui`).
    pub(crate) fn to_egui(self) -> Option<egui::Key> {
        match self {
            Key::Char(c) => egui::Key::from_name(&c.to_ascii_uppercase().to_string()),
            Key::F(n) => egui::Key::from_name(&format!("F{n}")),
            Key::Enter => Some(egui::Key::Enter),
            Key::Space => Some(egui::Key::Space),
            Key::Tab => Some(egui::Key::Tab),
            Key::Escape => Some(egui::Key::Escape),
            Key::Backspace => Some(egui::Key::Backspace),
            Key::Left => Some(egui::Key::ArrowLeft),
            Key::Right => Some(egui::Key::ArrowRight),
            Key::Up => Some(egui::Key::ArrowUp),
            Key::Down => Some(egui::Key::ArrowDown),
            Key::Home => Some(egui::Key::Home),
            Key::End => Some(egui::Key::End),
            Key::PageUp => Some(egui::Key::PageUp),
            Key::PageDown => Some(egui::Key::PageDown),
            Key::Insert => Some(egui::Key::Insert),
            Key::Delete => Some(egui::Key::Delete),
            Key::Plus => Some(egui::Key::Plus),
            Key::Minus => Some(egui::Key::Minus),
        }
    }

    /// La tecla que representa a una de egui, o `None` si no se puede usar
    /// en un atajo (signos de puntuación, F13+...). `=` cuenta como `+`.
    pub(crate) fn from_egui(key: egui::Key) -> Option<Key> {
        if key == egui::Key::Equals {
            return Some(Key::Plus);
        }
        let found = NAMES
            .iter()
            .map(|&(k, _)| k)
            .find(|k| k.to_egui() == Some(key));
        found.or_else(|| key.name().parse::<Key>().ok())
    }

    fn name(self) -> String {
        match self {
            Key::Char(c) => c.to_ascii_uppercase().to_string(),
            Key::F(n) => format!("F{n}"),
            named => NAMES
                .iter()
                .find(|(k, _)| *k == named)
                .map_or_else(String::new, |(_, names)| names[0].to_string()),
        }
    }
}

impl FromStr for Key {
    type Err = String;

    /// Una letra, un dígito, `F1`-`F12` o un nombre de `NAMES` (sin
    /// importar mayúsculas).
    fn from_str(text: &str) -> Result<Key, String> {
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.clone().next()) {
            if c.is_ascii_alphanumeric() {
                return Ok(Key::Char(c.to_ascii_uppercase()));
            }
        }
        if let Some(n) = text
            .strip_prefix(['F', 'f'])
            .and_then(|n| n.parse::<u8>().ok())
        {
            if (1..=12).contains(&n) {
                return Ok(Key::F(n));
            }
        }
        NAMES
            .iter()
            .find(|(_, names)| names.iter().any(|name| name.eq_ignore_ascii_case(text)))
            .map(|&(key, _)| key)
            .ok_or_else(|| format!("tecla desconocida: `{text}`"))
    }
}

/// Modificadores + tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Combo {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) key: Key,
}

impl Combo {
    pub(crate) const fn new(ctrl: bool, alt: bool, shift: bool, key: Key) -> Combo {
        Combo {
            ctrl,
            alt,
            shift,
            key,
        }
    }

    /// Modificadores para `egui::InputState::count_and_consume_key`, como
    /// `egui::Modifiers::CTRL` (sin `command`, que es el Ctrl de Mac).
    pub(crate) fn egui_modifiers(self) -> egui::Modifiers {
        egui::Modifiers {
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
            mac_cmd: false,
            command: false,
        }
    }

    /// La combinación de una tecla apretada en egui, o `None` si esa tecla
    /// no se puede usar en un atajo.
    pub(crate) fn from_egui(modifiers: egui::Modifiers, key: egui::Key) -> Option<Combo> {
        Some(Combo {
            ctrl: modifiers.ctrl || modifiers.command,
            alt: modifiers.alt,
            shift: modifiers.shift,
            key: Key::from_egui(key)?,
        })
    }

    /// `true` si tiene `Ctrl` o `Alt` (un atajo sin ellos tapa lo que se
    /// escribe).
    pub(crate) fn has_command_modifier(self) -> bool {
        self.ctrl || self.alt
    }
}

impl fmt::Display for Combo {
    /// `"Ctrl+Alt+→"`, en ese orden de modificadores.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        f.write_str(&self.key.name())
    }
}

impl FromStr for Combo {
    type Err = String;

    /// Contrato:
    /// - Post: acepta lo que escribe `Display` y variantes a mano:
    ///   modificadores en cualquier orden y sin importar mayúsculas
    ///   (`ctrl`/`control`, `alt`, `shift`/`mayús`), espacios alrededor de
    ///   cada parte, y `+` como tecla al final (`"Ctrl++"`).
    /// - Errores: texto vacío, modificador desconocido o repetido, tecla
    ///   desconocida, o más de una tecla → mensaje para el aviso de la
    ///   consola.
    fn from_str(text: &str) -> Result<Combo, String> {
        let text = text.trim();
        let (mods, key) = if text == "+" {
            ("", "+")
        } else if let Some(mods) = text.strip_suffix("++") {
            (mods, "+")
        } else {
            match text.rsplit_once('+') {
                Some((mods, key)) => (mods, key),
                None => ("", text),
            }
        };
        if key.trim().is_empty() {
            return Err(format!("combinación sin tecla: `{text}`"));
        }
        let mut combo = Combo {
            ctrl: false,
            alt: false,
            shift: false,
            key: key.trim().parse()?,
        };
        for part in mods.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            let flag = match part.to_lowercase().as_str() {
                "ctrl" | "control" => &mut combo.ctrl,
                "alt" => &mut combo.alt,
                "shift" | "mayús" | "mayus" => &mut combo.shift,
                _ => return Err(format!("modificador desconocido: `{part}`")),
            };
            if *flag {
                return Err(format!("modificador repetido: `{part}`"));
            }
            *flag = true;
        }
        Ok(combo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(text: &str) -> Combo {
        text.parse().unwrap()
    }

    #[test]
    fn texto_ida_y_vuelta() {
        for text in [
            "Ctrl+Alt+P",
            "Ctrl+→",
            "Ctrl+Alt+Enter",
            "Ctrl+Shift+F12",
            "Ctrl++",
            "Ctrl+-",
            "Alt+Shift+RePág",
            "F5",
            "Ctrl+Espacio",
        ] {
            assert_eq!(combo(text).to_string(), text);
        }
    }

    #[test]
    fn variantes_escritas_a_mano() {
        assert_eq!(combo(" alt + control + p "), combo("Ctrl+Alt+P"));
        assert_eq!(combo("ctrl+right"), combo("Ctrl+→"));
        assert_eq!(combo("Ctrl+Plus"), combo("Ctrl++"));
        assert_eq!(combo("ctrl+space"), combo("Ctrl+Espacio"));
        assert_eq!(combo("Mayús+f3"), combo("Shift+F3"));
        assert_eq!(combo("+"), Combo::new(false, false, false, Key::Plus));
    }

    #[test]
    fn textos_invalidos() {
        for text in [
            "",
            "Ctrl+",
            "Ctrl+Alt",
            "Hyper+P",
            "Ctrl+Ctrl+P",
            "Ctrl+F13",
            "Ctrl+F0",
            "Ctrl+PP",
            "Ctrl+ñ",
        ] {
            assert!(text.parse::<Combo>().is_err(), "`{text}` debería fallar");
        }
    }

    #[test]
    fn teclas_virtuales() {
        assert_eq!(Key::Char('p').vk(), 0x50);
        assert_eq!(Key::Char('7').vk(), 0x37);
        assert_eq!(Key::F(1).vk(), 0x70);
        assert_eq!(Key::F(12).vk(), 0x7B);
        assert_eq!(Key::Enter.vk(), 0x0D);
        assert_eq!(Key::Right.vk(), 0x27);
        assert_eq!(Key::Down.vk(), 0x28);
        assert_eq!(Key::Plus.vk(), 0xBB);
    }

    #[test]
    fn todas_las_teclas_van_y_vuelven_de_egui() {
        let mut keys: Vec<Key> = NAMES.iter().map(|&(k, _)| k).collect();
        keys.extend(('A'..='Z').chain('0'..='9').map(Key::Char));
        keys.extend((1..=12).map(Key::F));
        for key in keys {
            let egui_key = key.to_egui().unwrap_or_else(|| panic!("{key:?} sin egui"));
            assert_eq!(Key::from_egui(egui_key), Some(key));
        }
        assert_eq!(Key::from_egui(egui::Key::Equals), Some(Key::Plus));
        assert_eq!(Key::from_egui(egui::Key::Semicolon), None);
    }

    #[test]
    fn desde_una_tecla_de_egui() {
        let pressed = Combo::from_egui(egui::Modifiers::COMMAND, egui::Key::ArrowRight);
        assert_eq!(pressed, Some(combo("Ctrl+→")));
        assert_eq!(
            combo("Ctrl+Alt+P").egui_modifiers(),
            egui::Modifiers::CTRL | egui::Modifiers::ALT
        );
        assert!(!combo("Shift+F3").has_command_modifier());
        assert!(combo("Alt+F3").has_command_modifier());
    }
}
