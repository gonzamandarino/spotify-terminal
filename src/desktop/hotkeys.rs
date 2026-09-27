//! Atajos de teclado globales (spec 006): andan con la ventana minimizada o
//! con otra app enfocada.
//!
//! Se registran con `RegisterHotKey` de Windows en un hilo propio
//! ("atajos") que duerme en `GetMessageW` hasta que se aprieta una
//! combinación: sin polling y sin hook de teclado (la app no ve ninguna
//! otra tecla). Cada combinación se convierte en un `Input::Global` y va
//! al motor por el mismo canal que usa la ventana.

use tokio::sync::mpsc::UnboundedSender;

use super::{combo::Combo, settings::Settings};
use crate::{
    app::engine::{GlobalAction, Input},
    error::AppError,
};

/// Un atajo global: combinación + acción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shortcut {
    pub(crate) combo: Combo,
    pub(crate) action: GlobalAction,
}

impl Shortcut {
    /// "Ctrl+Alt+→  siguiente", para `help`.
    pub(crate) fn describe(&self) -> String {
        format!("{:<16} {}", self.combo.to_string(), self.action.describe())
    }

    /// Mantener apretado repite la acción: solo el volumen (como Ctrl+↑ en
    /// la ventana). El resto cuenta una vez.
    fn repeats(&self) -> bool {
        matches!(
            self.action,
            GlobalAction::VolumeUp | GlobalAction::VolumeDown
        )
    }
}

/// Atajos globales de los ajustes (los que tienen combinación), en el
/// orden de `GlobalAction::ALL`.
pub(crate) fn from_settings(settings: &Settings) -> Vec<Shortcut> {
    settings
        .global_keys
        .iter()
        .filter_map(|(&action, combo)| combo.map(|combo| Shortcut { combo, action }))
        .collect()
}

/// Atajo que no se pudo registrar, con el motivo para el usuario.
#[derive(Debug)]
pub(crate) struct Failed {
    pub(crate) shortcut: Shortcut,
    pub(crate) reason: String,
}

impl Failed {
    /// "⚠ Ctrl+Alt+→ ya lo usa otra app: sin atajo global para siguiente."
    pub(crate) fn warning(&self) -> String {
        format!(
            "⚠ {} {}: sin atajo global para {}.",
            self.shortcut.combo,
            self.reason,
            self.shortcut.action.describe()
        )
    }
}

/// Hilo de los atajos globales corriendo.
///
/// Invariante: mientras existe, los atajos de `active` están registrados
/// por esta app. Al soltarlo se desregistran y el hilo termina (espera a
/// que termine).
pub(crate) struct Hotkeys {
    pub(crate) active: Vec<Shortcut>,
    pub(crate) failed: Vec<Failed>,
    #[cfg(windows)]
    thread: Option<(u32, std::thread::JoinHandle<()>)>,
}

/// Registra `shortcuts` en un hilo "atajos" que manda un `Input::Global`
/// a `inputs` cada vez que se aprieta uno.
///
/// - Post: vuelve cuando terminó de registrar; `active` y `failed`
///   reparten `shortcuts`, en orden. Una combinación tomada por otra app
///   (o por otra ventana de esta) va a `failed` sin cortar las demás. El
///   hilo termina solo si `inputs` se cierra.
/// - Errores: `Internal` si no arranca el hilo.
/// - No debe: ver otras teclas que las registradas, ni despertarse si no
///   se aprieta ninguna.
#[cfg(windows)]
pub(crate) fn spawn(
    shortcuts: Vec<Shortcut>,
    inputs: UnboundedSender<Input>,
) -> Result<Hotkeys, AppError> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("atajos".into())
        .spawn(move || win::run(&shortcuts, &inputs, &ready_tx))
        .map_err(|e| AppError::Internal(format!("hilo de atajos: {e}")))?;
    let (thread_id, active, failed) = ready_rx
        .recv()
        .map_err(|_| AppError::Internal("el hilo de atajos no arrancó".into()))?;
    Ok(Hotkeys {
        active,
        failed,
        thread: Some((thread_id, thread)),
    })
}

/// Cambia los atajos registrados por `wanted` (spec 007).
///
/// - Pre: `current` son los registrados ahora (o `None`); `previous`, la
///   lista con la que se registraron.
/// - Post: `current` se suelta antes de registrar (sus combinaciones
///   quedan libres para otras apps y para `wanted`). Una combinación de
///   `wanted` que no estaba en `previous` y que Windows rechaza vuelve a la
///   de `previous` para esa acción (o a ninguna), y va en la lista
///   devuelta. Las que ya fallaban antes siguen en `failed`, sin volver a
///   avisar.
/// - Errores: `Internal` si no arranca el hilo.
pub(crate) fn replace(
    current: Option<Hotkeys>,
    wanted: &[Shortcut],
    previous: &[Shortcut],
    inputs: &UnboundedSender<Input>,
) -> Result<(Hotkeys, Vec<Failed>), AppError> {
    drop(current);
    let hotkeys = spawn(wanted.to_vec(), inputs.clone())?;
    let (rejected, _): (Vec<Failed>, Vec<Failed>) = hotkeys
        .failed
        .iter()
        .map(|f| Failed {
            shortcut: f.shortcut,
            reason: f.reason.clone(),
        })
        .partition(|f| !previous.contains(&f.shortcut));
    if rejected.is_empty() {
        return Ok((hotkeys, rejected));
    }
    drop(hotkeys);
    let fallback: Vec<Shortcut> = wanted
        .iter()
        .filter_map(|&shortcut| {
            if rejected.iter().any(|f| f.shortcut == shortcut) {
                previous
                    .iter()
                    .find(|p| p.action == shortcut.action)
                    .copied()
            } else {
                Some(shortcut)
            }
        })
        .collect();
    Ok((spawn(fallback, inputs.clone())?, rejected))
}

/// Fuera de Windows no hay atajos globales: todos van a `failed`.
#[cfg(not(windows))]
pub(crate) fn spawn(
    shortcuts: Vec<Shortcut>,
    _inputs: UnboundedSender<Input>,
) -> Result<Hotkeys, AppError> {
    Ok(Hotkeys {
        active: Vec::new(),
        failed: shortcuts
            .into_iter()
            .map(|shortcut| Failed {
                shortcut,
                reason: "no está disponible en este sistema".into(),
            })
            .collect(),
    })
}

#[cfg(windows)]
impl Drop for Hotkeys {
    fn drop(&mut self) {
        if let Some((thread_id, thread)) = self.thread.take() {
            win::quit(thread_id);
            let _ = thread.join();
        }
    }
}

#[cfg(windows)]
mod win {
    use std::{io, sync::mpsc};

    use tokio::sync::mpsc::UnboundedSender;
    use windows_sys::Win32::{
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
            },
            WindowsAndMessaging::{
                GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_HOTKEY, WM_QUIT,
            },
        },
    };

    use super::{Failed, Shortcut};
    use crate::app::engine::Input;

    /// Lo que el hilo avisa al terminar de registrar.
    pub(super) type Ready = (u32, Vec<Shortcut>, Vec<Failed>);

    /// `ERROR_HOTKEY_ALREADY_REGISTERED`.
    const ALREADY_REGISTERED: i32 = 1409;

    /// Cuerpo del hilo "atajos".
    pub(super) fn run(
        shortcuts: &[Shortcut],
        inputs: &UnboundedSender<Input>,
        ready: &mpsc::Sender<Ready>,
    ) {
        // SAFETY: sin argumentos; devuelve el id del hilo actual.
        let thread_id = unsafe { GetCurrentThreadId() };
        // SAFETY: MSG es un struct de C sin invariantes (todo cero vale).
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        // Crea la cola de mensajes del hilo antes de avisar su id: sin
        // cola, un `quit` temprano se perdería.
        // SAFETY: `msg` es válido; sin ventana (0); no saca nada de la cola.
        unsafe { PeekMessageW(&mut msg, 0, 0, 0, PM_NOREMOVE) };

        // id del atajo = índice + 1 (0 no es un id válido).
        let mut active = Vec::new();
        let mut registered = Vec::new();
        let mut failed = Vec::new();
        for (index, shortcut) in shortcuts.iter().enumerate() {
            let id = i32::try_from(index + 1).unwrap_or(i32::MAX);
            match register(id, shortcut) {
                Ok(()) => {
                    active.push(*shortcut);
                    registered.push((id, shortcut.action));
                }
                Err(e) => failed.push(Failed {
                    shortcut: *shortcut,
                    reason: reason(&e),
                }),
            }
        }
        if ready.send((thread_id, active, failed)).is_ok() {
            // `GetMessageW` duerme hasta el próximo mensaje: 0 con WM_QUIT,
            // -1 si falla.
            // SAFETY: `msg` es válido; sin ventana (0) = mensajes del hilo.
            while unsafe { GetMessageW(&mut msg, 0, 0, 0) } > 0 {
                if msg.message != WM_HOTKEY {
                    continue;
                }
                let action = registered
                    .iter()
                    .find(|(id, _)| usize::try_from(*id).ok() == Some(msg.wParam))
                    .map(|(_, action)| *action);
                if let Some(action) = action {
                    if inputs.send(Input::Global(action)).is_err() {
                        break; // el motor terminó
                    }
                }
            }
        }
        for (id, _) in registered {
            unregister(id);
        }
    }

    /// Pide al hilo `thread_id` que termine.
    pub(super) fn quit(thread_id: u32) {
        // SAFETY: mensaje sin punteros; si el hilo ya terminó, falla sin
        // efecto.
        unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
    }

    /// Registra `shortcut` con `id` para el hilo actual.
    pub(super) fn register(id: i32, shortcut: &Shortcut) -> io::Result<()> {
        let mut modifiers = 0;
        for (on, flag) in [
            (shortcut.combo.ctrl, MOD_CONTROL),
            (shortcut.combo.alt, MOD_ALT),
            (shortcut.combo.shift, MOD_SHIFT),
            (!shortcut.repeats(), MOD_NOREPEAT),
        ] {
            if on {
                modifiers |= flag;
            }
        }
        // SAFETY: sin ventana (0) = los mensajes van a la cola del hilo.
        if unsafe { RegisterHotKey(0, id, modifiers, shortcut.combo.key.vk()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn unregister(id: i32) {
        // SAFETY: si `id` no está registrado en este hilo, falla sin efecto.
        unsafe { UnregisterHotKey(0, id) };
    }

    fn reason(error: &io::Error) -> String {
        if error.raw_os_error() == Some(ALREADY_REGISTERED) {
            "ya lo usa otra app".into()
        } else {
            format!("no se pudo registrar ({error})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(combo: &str, action: GlobalAction) -> Shortcut {
        Shortcut {
            combo: combo.parse().unwrap(),
            action,
        }
    }

    #[test]
    fn nombres_de_las_combinaciones() {
        let next = shortcut("Ctrl+Alt+→", GlobalAction::Next);
        assert!(next.describe().starts_with("Ctrl+Alt+→ "));
        assert!(next.describe().ends_with(" siguiente"));
        let failed = Failed {
            shortcut: next,
            reason: "ya lo usa otra app".into(),
        };
        assert_eq!(
            failed.warning(),
            "⚠ Ctrl+Alt+→ ya lo usa otra app: sin atajo global para siguiente."
        );
    }

    #[test]
    fn config_sin_combinaciones_repetidas_y_solo_volumen_repite() {
        let all = from_settings(&Settings::default());
        assert_eq!(all.len(), GlobalAction::ALL.len());
        assert!(!all.is_empty());
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.combo, b.combo, "{} repetida", a.combo);
            }
            assert_eq!(
                a.repeats(),
                matches!(a.action, GlobalAction::VolumeUp | GlobalAction::VolumeDown)
            );
        }
    }

    /// Contra Windows de verdad, con una combinación que nadie usa.
    #[cfg(windows)]
    #[test]
    fn combinacion_tomada_falla_y_al_soltarla_queda_libre() {
        let rare = shortcut("Ctrl+Alt+Shift+Q", GlobalAction::Stop);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let first = spawn(vec![rare], tx.clone()).unwrap();
        assert_eq!(first.active.len(), 1, "{:?}", first.failed);
        // Otra "ventana" (otro hilo) no puede tomarla: AC-6.
        let second = spawn(vec![rare], tx.clone()).unwrap();
        assert!(second.active.is_empty());
        assert_eq!(second.failed.len(), 1);
        assert!(second.failed[0].warning().contains("ya lo usa otra app"));
        drop(second);
        // Al cerrar la primera, queda libre: AC-7.
        drop(first);
        let third = spawn(vec![rare], tx).unwrap();
        assert_eq!(third.active.len(), 1, "{:?}", third.failed);
    }

    /// Spec 007, AC-5: cambiar suelta la anterior; si la nueva está tomada,
    /// vuelve a la anterior y avisa.
    #[cfg(windows)]
    #[test]
    fn reemplazar_suelta_la_anterior_y_vuelve_si_la_nueva_esta_tomada() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let old = shortcut("Ctrl+Alt+Shift+F9", GlobalAction::Stop);
        let new = shortcut("Ctrl+Alt+Shift+F10", GlobalAction::Stop);
        let taken = shortcut("Ctrl+Alt+Shift+F11", GlobalAction::Stop);
        let current = spawn(vec![old], tx.clone()).unwrap();

        let (hotkeys, rejected) = replace(Some(current), &[new], &[old], &tx).unwrap();
        assert!(rejected.is_empty());
        assert_eq!(hotkeys.active, vec![new]);
        // La anterior quedó libre.
        let other = spawn(vec![old], tx.clone()).unwrap();
        assert_eq!(other.active, vec![old], "{:?}", other.failed);
        drop(other);

        // Otra "app" tiene la que se pide: vuelve a la de antes.
        let holder = spawn(vec![taken], tx.clone()).unwrap();
        let (hotkeys, rejected) = replace(Some(hotkeys), &[taken], &[new], &tx).unwrap();
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].shortcut, taken);
        assert_eq!(hotkeys.active, vec![new]);
        drop(holder);
    }
}
