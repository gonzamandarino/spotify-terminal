//! Volumen de la app (spec 005): nivel 0-100 y mute, y dónde se guarda
//! entre sesiones. Lo usan el motor de la app de escritorio y la CLI; el
//! reproductor solo recibe [`Volume::output`].

use std::{fmt, fs, path::Path};

use crate::{config, error::AppError};

/// Volumen elegido por el usuario.
///
/// Invariante: `level` ≤ 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volume {
    level: u8,
    muted: bool,
}

impl Default for Volume {
    /// `config::VOLUME_DEFAULT`, sin mute.
    fn default() -> Volume {
        Volume {
            level: config::VOLUME_DEFAULT,
            muted: false,
        }
    }
}

impl Volume {
    /// Nivel en %, aunque esté en mute (es al que vuelve).
    pub fn level(&self) -> u8 {
        self.level
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Fija el nivel (lo que pase de 100 queda en 100) y saca del mute.
    pub fn set(&mut self, level: u8) {
        self.level = level.min(100);
        self.muted = false;
    }

    /// Sube `config::VOLUME_STEP` desde el nivel de antes del mute, hasta
    /// 100, y saca del mute.
    pub fn up(&mut self) {
        self.set(self.level.saturating_add(config::VOLUME_STEP));
    }

    /// Baja `config::VOLUME_STEP`, hasta 0, y saca del mute.
    pub fn down(&mut self) {
        self.set(self.level.saturating_sub(config::VOLUME_STEP));
    }

    /// Silencia, o vuelve al nivel de antes.
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    /// Volumen para el reproductor en su escala (0 = silencio, `u16::MAX` =
    /// 100 %). En mute, 0.
    pub fn output(&self) -> u16 {
        if self.muted {
            return 0;
        }
        let scaled = u32::from(self.level) * u32::from(u16::MAX) / 100;
        // `level` ≤ 100 → `scaled` ≤ u16::MAX.
        u16::try_from(scaled).unwrap_or(u16::MAX)
    }

    /// Último volumen guardado en `dir` (sin mute).
    ///
    /// - Post: sin archivo, ilegible o con otra cosa que un número →
    ///   `Volume::default()`; más de 100 → 100. Nunca falla: el archivo no
    ///   es de confiar.
    /// - No debe: crear ni modificar nada.
    pub fn load(dir: &Path) -> Volume {
        let mut volume = Volume::default();
        if let Some(level) = fs::read_to_string(dir.join(config::VOLUME_FILE))
            .ok()
            .and_then(|text| text.trim().parse::<u64>().ok())
        {
            volume.set(u8::try_from(level.min(100)).unwrap_or(100));
        }
        volume
    }

    /// Guarda el nivel en `dir` (la crea si hace falta). El mute no se
    /// guarda: al reabrir suena al último nivel.
    ///
    /// - Errores: `Io` si no se puede escribir.
    pub fn save(&self, dir: &Path) -> Result<(), AppError> {
        fs::create_dir_all(dir)?;
        fs::write(dir.join(config::VOLUME_FILE), self.level.to_string())?;
        Ok(())
    }
}

impl fmt::Display for Volume {
    /// "70 %" o "silenciado (70 %)".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.muted {
            write!(f, "silenciado ({} %)", self.level)
        } else {
            write!(f, "{} %", self.level)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn at(level: u8) -> Volume {
        let mut volume = Volume::default();
        volume.set(level);
        volume
    }

    /// Carpeta temporal propia de cada test.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spotify-terminal-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn sube_y_baja_sin_salirse_de_0_a_100() {
        let mut volume = at(98);
        volume.up();
        assert_eq!(volume.level(), 100);
        volume.up();
        assert_eq!(volume.level(), 100);
        let mut volume = at(3);
        volume.down();
        assert_eq!(volume.level(), 0);
        volume.down();
        assert_eq!(volume.level(), 0);
        assert_eq!(at(150).level(), 100);
    }

    #[test]
    fn mute_y_vuelta() {
        let mut volume = at(70);
        volume.toggle_mute();
        assert!(volume.muted());
        assert_eq!(volume.output(), 0);
        assert_eq!(volume.to_string(), "silenciado (70 %)");
        volume.toggle_mute();
        assert_eq!(volume, at(70));
        // `+`, `-` y fijar sacan del mute; `+`/`-` parten del nivel de antes.
        volume.toggle_mute();
        volume.up();
        assert_eq!(volume, at(75));
        volume.toggle_mute();
        volume.set(20);
        assert_eq!(volume, at(20));
    }

    #[test]
    fn salida_en_la_escala_del_reproductor() {
        assert_eq!(at(0).output(), 0);
        assert_eq!(at(100).output(), u16::MAX);
        assert_eq!(at(50).output(), u16::MAX / 2);
        assert_eq!(at(70).to_string(), "70 %");
    }

    #[test]
    fn cargar_sin_archivo_o_basura_da_el_default() {
        let dir = temp_dir("basura");
        assert_eq!(Volume::load(&dir), Volume::default());
        fs::create_dir_all(&dir).unwrap();
        for text in ["", "abc", "-3", "7.5", "1 2"] {
            fs::write(dir.join(config::VOLUME_FILE), text).unwrap();
            assert_eq!(Volume::load(&dir), Volume::default(), "{text:?}");
        }
        fs::write(dir.join(config::VOLUME_FILE), "150").unwrap();
        assert_eq!(Volume::load(&dir), at(100));
        fs::write(dir.join(config::VOLUME_FILE), "99999999999999999999").unwrap();
        assert_eq!(Volume::load(&dir), Volume::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn guardar_y_cargar_ida_y_vuelta_sin_mute() {
        let dir = temp_dir("ida-y-vuelta");
        let mut volume = at(35);
        volume.toggle_mute();
        volume.save(&dir).unwrap();
        assert_eq!(Volume::load(&dir), at(35));
        let _ = fs::remove_dir_all(&dir);
    }
}
