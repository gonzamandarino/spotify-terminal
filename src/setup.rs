//! Configuración del Client ID propio (spec 008): validarlo, guardarlo en
//! `config::CLIENT_ID_FILE` y aplicarlo. Lo usan la CLI (`setup` y la guía
//! al faltar el Client ID) y la app de escritorio, así las dos se comportan
//! igual.

use std::{
    fs,
    io::{self, BufRead, Write},
    path::Path,
};

use crate::{
    config::{self, ClientIdSource},
    error::AppError,
};

/// Client ID que ya pasó [`validate`].
///
/// Invariante: `config::CLIENT_ID_LEN` caracteres hexadecimales en
/// minúscula, sin espacios.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientId(String);

impl ClientId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Lo que hay en `client-id.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedClientId {
    /// No existe el archivo.
    Missing,
    Valid(ClientId),
    /// Existe pero no se pudo leer o no es un Client ID válido (editado a
    /// mano, corrupto): se trata como si no hubiera.
    Invalid,
}

/// Valida un Client ID pegado por el usuario.
///
/// - Post: `Ok` con el Client ID recortado y en minúscula, o `Err` con el
///   motivo para mostrar ("está vacío", "tiene 31 caracteres...").
/// - No debe: tocar disco ni red.
pub fn validate(raw: &str) -> Result<ClientId, String> {
    let id = raw.trim();
    if id.is_empty() {
        return Err("está vacío".into());
    }
    let len = id.chars().count();
    if len != config::CLIENT_ID_LEN {
        return Err(format!(
            "tiene {len} caracteres y un Client ID tiene {}",
            config::CLIENT_ID_LEN
        ));
    }
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("tiene caracteres que no son 0-9 ni a-f (¿se copió algo de más?)".into());
    }
    Ok(ClientId(id.to_ascii_lowercase()))
}

/// Lee el Client ID guardado en `dir`.
///
/// - Post: nunca falla; lo ilegible o inválido es `Invalid`.
pub fn read_saved(dir: &Path) -> SavedClientId {
    match fs::read_to_string(dir.join(config::CLIENT_ID_FILE)) {
        Ok(text) => validate(&text).map_or(SavedClientId::Invalid, SavedClientId::Valid),
        Err(e) if e.kind() == io::ErrorKind::NotFound => SavedClientId::Missing,
        Err(_) => SavedClientId::Invalid,
    }
}

/// Guarda `id` en `dir` (la crea si hace falta).
///
/// - Post: el archivo tiene solo el Client ID. Se escribe a un temporal y
///   se renombra: un corte a mitad deja el archivo anterior entero.
/// - Errores: los de I/O.
pub fn save(dir: &Path, id: &ClientId) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(config::CLIENT_ID_FILE);
    let tmp = path.with_extension("txt.tmp");
    fs::write(&tmp, id.as_str())?;
    fs::rename(&tmp, &path)
}

/// Resultado de [`apply`], para armar el aviso al usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// Cambió el Client ID en uso: se borró el token de la Web API y hay
    /// que volver a hacer login.
    pub changed: bool,
    /// `SPOTIFY_CLIENT_ID` (variable de entorno o `.env`) tiene prioridad y
    /// tapa al recién guardado.
    pub overridden_by_env: bool,
}

impl Applied {
    /// Aviso para el usuario después de guardar, o `None` si no hay nada
    /// que avisar. `login` es cómo se llama el comando en quien lo muestra.
    pub fn notice(&self, login: &str) -> Option<String> {
        if self.overridden_by_env {
            Some(
                "⚠ Se guardó, pero SPOTIFY_CLIENT_ID (variable de entorno o .env) \
                 tiene prioridad: mientras exista se sigue usando ese."
                    .into(),
            )
        } else if self.changed {
            Some(format!(
                "Cambió el Client ID: la sesión anterior se borró. Hacé `{login}` de nuevo."
            ))
        } else {
            None
        }
    }
}

/// Guarda `id` en la carpeta de datos y lo deja en uso.
///
/// - Post: `client-id.txt` tiene `id`. Si el Client ID en uso (ver
///   `config::Config::load`) cambió, se borró el token cacheado de la Web
///   API (era de otro Client ID); el de audio no se toca (es del Client ID
///   de librespot).
/// - Errores: `NoConfigDir`, `EnvFile`, `Io`.
pub fn apply(id: &ClientId) -> Result<Applied, AppError> {
    apply_in(&config::data_dir()?, config::env_client_id()?, id)
}

/// [`apply`] con la carpeta y el valor de `SPOTIFY_CLIENT_ID` explícitos,
/// para testearla sin tocar `%APPDATA%` ni el entorno.
fn apply_in(dir: &Path, env: Option<String>, id: &ClientId) -> Result<Applied, AppError> {
    let before = config::resolve_client_id(env.clone(), read_saved(dir))
        .ok()
        .map(|(id, _)| id);
    save(dir, id)?;
    let (after, source) = config::resolve_client_id(env, read_saved(dir))?;
    let changed = before.as_deref() != Some(after.as_str());
    if changed {
        match fs::remove_file(dir.join(config::WEB_TOKEN_FILE)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(Applied {
        changed,
        overridden_by_env: source == ClientIdSource::Env && after != id.as_str(),
    })
}

/// Pide un Client ID por `input` hasta que sea válido.
///
/// - Post: `Some` con el Client ID válido, o `None` si el usuario canceló
///   (`q`) o se terminó la entrada (Ctrl+Z / Ctrl+C). No guarda nada.
/// - Errores: los de I/O de `input` / `out`.
pub fn prompt(input: &mut impl BufRead, out: &mut impl Write) -> io::Result<Option<ClientId>> {
    loop {
        write!(out, "Client ID (q cancela): ")?;
        out.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 || line.trim().eq_ignore_ascii_case("q") {
            return Ok(None);
        }
        match validate(&line) {
            Ok(id) => return Ok(Some(id)),
            Err(reason) => writeln!(out, "⚠ Ese Client ID {reason}. Probá de nuevo.")?,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef";
    const OTHER: &str = "fedcba9876543210fedcba9876543210";

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("spt-setup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn id(s: &str) -> ClientId {
        validate(s).unwrap()
    }

    #[test]
    fn validate_rechaza_con_motivo() {
        for (raw, reason) in [
            ("", "vacío"),
            ("   ", "vacío"),
            (&ID[1..], "31 caracteres"),
            (&format!("{ID}0"), "33 caracteres"),
            ("0123456789abcdef0123456789abcdeg", "0-9 ni a-f"),
            ("0123456789abcdef 123456789abcdef", "0-9 ni a-f"),
        ] {
            let err = validate(raw).unwrap_err();
            assert!(err.contains(reason), "{raw:?}: {err}");
        }
    }

    #[test]
    fn validate_recorta_y_pasa_a_minuscula() {
        assert_eq!(validate(&format!("  {ID}\r\n")).unwrap().as_str(), ID);
        assert_eq!(validate(&ID.to_uppercase()).unwrap().as_str(), ID);
    }

    #[test]
    fn save_y_read_saved_ida_y_vuelta_sin_temporal() {
        let dir = temp_dir("ida-y-vuelta");
        assert_eq!(read_saved(&dir), SavedClientId::Missing);
        save(&dir, &id(ID)).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(config::CLIENT_ID_FILE)).unwrap(),
            ID
        );
        assert!(!dir.join("client-id.txt.tmp").exists());
        assert_eq!(read_saved(&dir), SavedClientId::Valid(id(ID)));
    }

    #[test]
    fn read_saved_con_basura_es_invalido() {
        let dir = temp_dir("basura");
        fs::create_dir_all(&dir).unwrap();
        for text in ["", "hola", "{\"id\": 1}"] {
            fs::write(dir.join(config::CLIENT_ID_FILE), text).unwrap();
            assert_eq!(read_saved(&dir), SavedClientId::Invalid, "{text:?}");
        }
    }

    fn write_tokens(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(config::WEB_TOKEN_FILE), "{}").unwrap();
        fs::write(dir.join(config::AUDIO_TOKEN_FILE), "{}").unwrap();
    }

    #[test]
    fn apply_con_cambio_borra_solo_el_token_web() {
        let dir = temp_dir("cambio");
        write_tokens(&dir);
        save(&dir, &id(ID)).unwrap();
        let applied = apply_in(&dir, None, &id(OTHER)).unwrap();
        assert!(applied.changed && !applied.overridden_by_env);
        assert!(!dir.join(config::WEB_TOKEN_FILE).exists());
        assert!(dir.join(config::AUDIO_TOKEN_FILE).exists());
        assert_eq!(read_saved(&dir), SavedClientId::Valid(id(OTHER)));
    }

    #[test]
    fn apply_la_primera_vez_cuenta_como_cambio() {
        let dir = temp_dir("primera");
        assert!(apply_in(&dir, None, &id(ID)).unwrap().changed);
    }

    #[test]
    fn apply_sin_cambio_no_borra_nada() {
        let dir = temp_dir("igual");
        write_tokens(&dir);
        save(&dir, &id(ID)).unwrap();
        let applied = apply_in(&dir, None, &id(ID)).unwrap();
        assert_eq!(applied.notice("login"), None);
        assert!(dir.join(config::WEB_TOKEN_FILE).exists());
    }

    #[test]
    fn apply_con_env_activo_avisa_y_no_borra() {
        let dir = temp_dir("env");
        write_tokens(&dir);
        let applied = apply_in(&dir, Some(ID.into()), &id(OTHER)).unwrap();
        assert!(applied.overridden_by_env && !applied.changed);
        assert!(dir.join(config::WEB_TOKEN_FILE).exists());
        assert_eq!(read_saved(&dir), SavedClientId::Valid(id(OTHER)));
    }

    fn run_prompt(input: &str) -> (Option<ClientId>, String) {
        let mut out = Vec::new();
        let got = prompt(&mut input.as_bytes(), &mut out).unwrap();
        (got, String::from_utf8(out).unwrap())
    }

    #[test]
    fn prompt_insiste_hasta_uno_valido() {
        let (got, out) = run_prompt(&format!("hola\n\n{ID}\n"));
        assert_eq!(got, Some(id(ID)));
        assert_eq!(out.matches("Probá de nuevo").count(), 2);
    }

    #[test]
    fn prompt_se_cancela_con_q_o_fin_de_entrada() {
        assert_eq!(run_prompt("q\n").0, None);
        assert_eq!(run_prompt("Q\n").0, None);
        assert_eq!(run_prompt("hola\n").0, None);
        assert_eq!(run_prompt("").0, None);
    }
}
