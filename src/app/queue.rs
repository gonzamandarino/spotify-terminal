//! Cola de reproducción: qué tema suena después, sin nada de I/O. La usan la
//! pantalla de la CLI (`ui::playback`) y el motor de la app de escritorio
//! (`app::engine`).

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use librespot_core::SpotifyUri;
use rand::{Rng, seq::SliceRandom};

use crate::{config, error::AppError};

/// Tiempo que lleva sonando el tema actual, a partir de las posiciones que
/// informa el reproductor (sin pedirle nada ni hacer polling).
#[derive(Debug, Default)]
pub(crate) struct Clock {
    base: Duration,
    /// Desde cuándo suena sin pausa; `None` si está en pausa o cargando.
    since: Option<Instant>,
}

impl Clock {
    pub(crate) fn playing(&mut self, position_ms: u32) {
        self.base = Duration::from_millis(position_ms.into());
        self.since = Some(Instant::now());
    }

    pub(crate) fn paused(&mut self, position_ms: u32) {
        self.base = Duration::from_millis(position_ms.into());
        self.since = None;
    }

    pub(crate) fn seeked(&mut self, position_ms: u32) {
        self.base = Duration::from_millis(position_ms.into());
        if self.since.is_some() {
            self.since = Some(Instant::now());
        }
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.base + self.since.map_or(Duration::ZERO, |s| s.elapsed())
    }
}

/// Qué tiene que hacer el loop con el reproductor.
#[derive(Debug)]
pub(crate) enum Step {
    Nothing,
    Preload(SpotifyUri),
    Play(SpotifyUri),
    /// Volver al principio del tema actual.
    Restart,
    /// El tema actual no está disponible: avisar y pasar a `next`.
    Skip {
        unavailable: SpotifyUri,
        next: SpotifyUri,
    },
    Done,
    Fail(AppError),
}

/// Un tema que ya sonó o suena, con su etiqueta de posición ("[3/12] ",
/// "[cola] " o vacía).
#[derive(Debug)]
struct Entry {
    uri: SpotifyUri,
    label: String,
}

/// Estado de la cola, separado de la I/O para poder testearlo.
///
/// Qué suena después del tema actual, en orden: lo encolado (`up_next`),
/// los temas a los que se volvió con "anterior" y quedaron adelante
/// (`timeline` después de `cursor`), y el resto de la lista en `order`.
///
/// Invariantes:
/// - `order` es una permutación de `0..tracks.len()`; con shuffle apagado,
///   es la identidad. Las posiciones hasta `context_pos` ya entraron a
///   `timeline`.
/// - `up_next` nunca se mezcla.
/// - Después de `start`, `timeline` no está vacío y `cursor` apunta al tema
///   actual.
/// - `request_id` es el del último `play` pedido, o `None` entre que se pide
///   y librespot lo confirma (`PlayRequestIdChanged`); mientras es `None` se
///   ignoran los eventos, que son del tema anterior.
pub(crate) struct Queue {
    tracks: Vec<SpotifyUri>,
    order: Vec<usize>,
    /// Posición en `order` del último tema de la lista que empezó a sonar.
    context_pos: Option<usize>,
    up_next: VecDeque<SpotifyUri>,
    /// Lo que sonó y suena, en el orden en que se escuchó.
    timeline: Vec<Entry>,
    cursor: usize,
    shuffled: bool,
    request_id: Option<u64>,
    /// El tema actual llegó a sonar (`Playing`/`Paused`). Un `Unavailable`
    /// después de eso es de la precarga del siguiente, no del actual.
    started: bool,
    /// Ya llegó `TimeToPreloadNextTrack` del tema actual: si cambia lo que
    /// sigue (shuffle, encolar), hay que precargar de nuevo.
    preload_due: bool,
    consecutive_unavailable: u32,
}

impl Queue {
    /// Con `shuffle` (y más de un tema), el orden entero sale mezclado: el
    /// primer tema también es al azar.
    pub(crate) fn new(tracks: Vec<SpotifyUri>, shuffle: bool, rng: &mut impl Rng) -> Queue {
        let mut order: Vec<usize> = (0..tracks.len()).collect();
        let shuffled = shuffle && tracks.len() > 1;
        if shuffled {
            order.shuffle(rng);
        }
        Queue {
            tracks,
            order,
            context_pos: None,
            up_next: VecDeque::new(),
            timeline: Vec::new(),
            cursor: 0,
            shuffled,
            request_id: None,
            started: false,
            preload_due: false,
            consecutive_unavailable: 0,
        }
    }

    /// Primer tema a pedir, o `None` si la lista está vacía.
    pub(crate) fn start(&mut self) -> Option<SpotifyUri> {
        self.advance()
    }

    pub(crate) fn can_shuffle(&self) -> bool {
        self.tracks.len() > 1
    }

    pub(crate) fn shuffled(&self) -> bool {
        self.shuffled
    }

    /// Temas encolados que todavía no sonaron.
    pub(crate) fn queued(&self) -> usize {
        self.up_next.len()
    }

    /// Etiqueta del tema actual: "[3/12] ", "[cola] " o vacía.
    pub(crate) fn position(&self) -> &str {
        self.timeline
            .get(self.cursor)
            .map_or("", |e| e.label.as_str())
    }

    pub(crate) fn is_current(&self, id: u64) -> bool {
        self.request_id == Some(id)
    }

    pub(crate) fn on_request_id(&mut self, id: u64) {
        self.request_id = Some(id);
        self.started = false;
        self.preload_due = false;
    }

    /// Devuelve `true` si el evento es del tema actual.
    pub(crate) fn on_started(&mut self, id: u64) -> bool {
        if !self.is_current(id) {
            return false;
        }
        self.started = true;
        self.consecutive_unavailable = 0;
        true
    }

    pub(crate) fn on_preload_time(&mut self, id: u64) -> Step {
        if !self.is_current(id) {
            return Step::Nothing;
        }
        self.preload_due = true;
        self.repreload()
    }

    /// Precarga lo que sigue si ya era momento (después de cambiar el orden
    /// o encolar); si no, nada.
    pub(crate) fn repreload(&self) -> Step {
        match self.peek_next() {
            Some(next) if self.preload_due => Step::Preload(next),
            _ => Step::Nothing,
        }
    }

    pub(crate) fn on_end(&mut self, id: u64) -> Step {
        if !self.is_current(id) {
            return Step::Nothing;
        }
        self.next()
    }

    /// Pasa al tema siguiente (tecla `n` o fin del tema).
    pub(crate) fn next(&mut self) -> Step {
        match self.advance() {
            Some(next) => Step::Play(next),
            None => Step::Done,
        }
    }

    /// Tecla `p`: con más de `config::PREVIOUS_RESTART_THRESHOLD` sonando, o
    /// en el primer tema, reinicia; si no, vuelve al tema que sonó antes.
    pub(crate) fn previous(&mut self, elapsed: Duration) -> Step {
        if elapsed > config::PREVIOUS_RESTART_THRESHOLD || self.cursor == 0 {
            return Step::Restart;
        }
        self.cursor -= 1;
        self.reset_request();
        Step::Play(self.timeline[self.cursor].uri.clone())
    }

    /// Prende o apaga el shuffle. Prenderlo mezcla lo que falta de la lista
    /// (el tema actual sigue); apagarlo retoma el orden original a partir
    /// del último tema de la lista que sonó. `None` si hay un solo tema.
    pub(crate) fn toggle_shuffle(&mut self, rng: &mut impl Rng) -> Option<bool> {
        if !self.can_shuffle() {
            return None;
        }
        if self.shuffled {
            let resume = self.context_pos.map(|p| self.order[p]);
            self.order = (0..self.tracks.len()).collect();
            self.context_pos = resume;
        } else {
            let from = self.next_context_pos();
            self.order[from..].shuffle(rng);
        }
        self.shuffled = !self.shuffled;
        Some(self.shuffled)
    }

    /// Encola `uri`: suena después del actual y de lo ya encolado.
    pub(crate) fn enqueue(&mut self, uri: SpotifyUri) {
        self.up_next.push_back(uri);
    }

    pub(crate) fn on_unavailable(&mut self, id: u64, session_lost: bool) -> Step {
        if !self.is_current(id) || self.started {
            return Step::Nothing;
        }
        let unavailable = self.timeline[self.cursor].uri.clone();
        if session_lost {
            return Step::Fail(AppError::Network(
                "se perdió la conexión con Spotify".into(),
            ));
        }
        if self.timeline.len() == 1 && self.peek_next().is_none() {
            return Step::Fail(AppError::TrackUnavailable(uri_text(&unavailable)));
        }
        self.consecutive_unavailable += 1;
        if self.consecutive_unavailable >= config::MAX_CONSECUTIVE_UNAVAILABLE {
            return Step::Fail(AppError::Network(format!(
                "fallaron {} temas seguidos",
                self.consecutive_unavailable
            )));
        }
        match self.advance() {
            Some(next) => Step::Skip { unavailable, next },
            None => Step::Done,
        }
    }

    fn next_context_pos(&self) -> usize {
        self.context_pos.map_or(0, |p| p + 1)
    }

    /// Lo que sonaría con `next`, sin moverse.
    fn peek_next(&self) -> Option<SpotifyUri> {
        if let Some(uri) = self.up_next.front() {
            return Some(uri.clone());
        }
        if let Some(entry) = self.timeline.get(self.cursor + 1) {
            return Some(entry.uri.clone());
        }
        let i = *self.order.get(self.next_context_pos())?;
        Some(self.tracks[i].clone())
    }

    /// Pasa al tema siguiente; los eventos se ignoran hasta que librespot
    /// confirme el pedido nuevo.
    fn advance(&mut self) -> Option<SpotifyUri> {
        let at = if self.timeline.is_empty() {
            0
        } else {
            self.cursor + 1
        };
        if let Some(uri) = self.up_next.pop_front() {
            let entry = Entry {
                uri,
                label: "[cola] ".into(),
            };
            self.timeline.insert(at, entry);
        } else if at >= self.timeline.len() {
            let pos = self.next_context_pos();
            let i = *self.order.get(pos)?;
            self.context_pos = Some(pos);
            let label = if self.tracks.len() > 1 {
                format!("[{}/{}] ", pos + 1, self.tracks.len())
            } else {
                String::new()
            };
            self.timeline.push(Entry {
                uri: self.tracks[i].clone(),
                label,
            });
        }
        self.cursor = at;
        self.reset_request();
        Some(self.timeline[at].uri.clone())
    }

    fn reset_request(&mut self) {
        self.request_id = None;
        self.started = false;
        self.preload_due = false;
    }
}

pub(crate) fn uri_text(uri: &SpotifyUri) -> String {
    uri.to_uri().unwrap_or_else(|_| "(sin URI)".to_string())
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    fn tracks(n: usize) -> Vec<SpotifyUri> {
        (0..n).map(|i| track(&i.to_string())).collect()
    }

    fn track(id: &str) -> SpotifyUri {
        SpotifyUri::from_uri(&format!("spotify:track:{id:0>22}")).unwrap()
    }

    fn rng() -> StdRng {
        StdRng::seed_from_u64(7)
    }

    fn is_play(step: &Step, expected: &SpotifyUri) -> bool {
        matches!(step, Step::Play(uri) if uri == expected)
    }

    fn played(step: Step) -> SpotifyUri {
        match step {
            Step::Play(uri) => uri,
            other => panic!("se esperaba Play, vino {other:?}"),
        }
    }

    /// Una cola en orden, con el primer tema sonando (request id 1).
    fn started(list: &[SpotifyUri]) -> Queue {
        let mut q = Queue::new(list.to_vec(), false, &mut rng());
        q.start();
        q.on_request_id(1);
        q.on_started(1);
        q
    }

    const SHORT: Duration = Duration::from_secs(1);
    const LONG: Duration = Duration::from_secs(10);

    #[test]
    fn fin_de_tema_repetido_no_saltea_de_mas() {
        let list = tracks(3);
        let mut q = started(&list);
        assert!(is_play(&q.on_end(1), &list[1]));
        // librespot repite EndOfTrack si falla la decodificación.
        assert!(matches!(q.on_end(1), Step::Nothing));
        q.on_request_id(2);
        assert!(matches!(q.on_end(1), Step::Nothing));
        assert!(is_play(&q.on_end(2), &list[2]));
    }

    #[test]
    fn eventos_antes_de_confirmar_el_pedido_se_ignoran() {
        let list = tracks(2);
        let mut q = Queue::new(list.clone(), false, &mut rng());
        q.start();
        assert!(matches!(q.on_end(0), Step::Nothing));
        assert!(!q.on_started(0));
        assert!(matches!(q.on_preload_time(0), Step::Nothing));
    }

    #[test]
    fn precarga_fallida_no_corta_el_tema_actual() {
        let list = tracks(3);
        let mut q = started(&list);
        assert!(matches!(q.on_preload_time(1), Step::Preload(uri) if uri == list[1]));
        // La precarga falla: librespot manda Unavailable con el id actual.
        assert!(matches!(q.on_unavailable(1, false), Step::Nothing));
        assert!(is_play(&q.on_end(1), &list[1]));
    }

    #[test]
    fn tema_no_disponible_se_saltea_en_lista() {
        let list = tracks(3);
        let mut q = Queue::new(list.clone(), false, &mut rng());
        q.start();
        q.on_request_id(1);
        let step = q.on_unavailable(1, false);
        assert!(
            matches!(&step, Step::Skip { unavailable, next } if *unavailable == list[0] && *next == list[1])
        );
    }

    #[test]
    fn tema_unico_no_disponible_es_error() {
        let list = tracks(1);
        let mut q = Queue::new(list.clone(), false, &mut rng());
        q.start();
        q.on_request_id(1);
        assert!(matches!(
            q.on_unavailable(1, false),
            Step::Fail(AppError::TrackUnavailable(_))
        ));
    }

    #[test]
    fn tema_unico_no_disponible_con_cola_sigue_con_lo_encolado() {
        let list = tracks(1);
        let mut q = Queue::new(list.clone(), false, &mut rng());
        q.start();
        q.on_request_id(1);
        q.enqueue(track("99"));
        assert!(
            matches!(q.on_unavailable(1, false), Step::Skip { next, .. } if next == track("99"))
        );
    }

    #[test]
    fn sesion_caida_corta_la_lista() {
        let list = tracks(5);
        let mut q = Queue::new(list.clone(), false, &mut rng());
        q.start();
        q.on_request_id(1);
        assert!(matches!(
            q.on_unavailable(1, true),
            Step::Fail(AppError::Network(_))
        ));
    }

    #[test]
    fn muchos_no_disponibles_seguidos_cortan_la_lista() {
        let list = tracks(10);
        let mut q = Queue::new(list.clone(), false, &mut rng());
        q.start();
        let mut id = 0;
        for _ in 1..config::MAX_CONSECUTIVE_UNAVAILABLE {
            id += 1;
            q.on_request_id(id);
            assert!(matches!(q.on_unavailable(id, false), Step::Skip { .. }));
        }
        id += 1;
        q.on_request_id(id);
        assert!(matches!(
            q.on_unavailable(id, false),
            Step::Fail(AppError::Network(_))
        ));
    }

    #[test]
    fn un_tema_que_suena_reinicia_el_contador() {
        let list = tracks(10);
        let mut q = Queue::new(list.clone(), false, &mut rng());
        q.start();
        let mut id = 0;
        for _ in 0..3 {
            for _ in 1..config::MAX_CONSECUTIVE_UNAVAILABLE {
                id += 1;
                q.on_request_id(id);
                assert!(matches!(q.on_unavailable(id, false), Step::Skip { .. }));
            }
            id += 1;
            q.on_request_id(id);
            q.on_started(id);
            q.on_end(id);
        }
    }

    #[test]
    fn ultimo_tema_termina_la_cola() {
        let list = tracks(1);
        let mut q = started(&list);
        assert!(matches!(q.on_preload_time(1), Step::Nothing));
        assert!(matches!(q.on_end(1), Step::Done));
    }

    #[test]
    fn siguiente_y_en_el_ultimo_termina() {
        let list = tracks(2);
        let mut q = started(&list);
        assert!(is_play(&q.next(), &list[1]));
        assert!(matches!(q.next(), Step::Done));
    }

    #[test]
    fn siguiente_varias_veces_rapido_ignora_eventos_viejos() {
        let list = tracks(5);
        let mut q = started(&list);
        assert!(is_play(&q.next(), &list[1]));
        assert!(is_play(&q.next(), &list[2]));
        // Eventos del tema 0 y del 1 (nunca confirmados) llegan tarde.
        assert!(matches!(q.on_end(1), Step::Nothing));
        assert!(!q.on_started(1));
        q.on_request_id(3);
        assert!(q.on_started(3));
        assert_eq!(q.position(), "[3/5] ");
        assert!(is_play(&q.on_end(3), &list[3]));
    }

    #[test]
    fn anterior_segun_el_tiempo_sonando() {
        let list = tracks(3);
        let mut q = started(&list);
        // En el primer tema siempre reinicia.
        assert!(matches!(q.previous(SHORT), Step::Restart));
        q.next();
        assert!(matches!(q.previous(LONG), Step::Restart));
        assert!(is_play(&q.previous(SHORT), &list[0]));
        // Después de volver, "siguiente" retoma donde estaba.
        assert!(is_play(&q.next(), &list[1]));
        assert!(is_play(&q.next(), &list[2]));
    }

    #[test]
    fn anterior_con_shuffle_vuelve_a_lo_que_sono() {
        let list = tracks(10);
        let mut q = started(&list);
        q.toggle_shuffle(&mut rng());
        let a = played(q.next());
        let b = played(q.next());
        assert!(is_play(&q.previous(SHORT), &a));
        assert!(is_play(&q.previous(SHORT), &list[0]));
        assert!(is_play(&q.next(), &a));
        assert!(is_play(&q.next(), &b));
    }

    /// Lo que suena desde el actual hasta el final, pasando con `next`.
    fn rest(q: &mut Queue) -> Vec<SpotifyUri> {
        let mut out = Vec::new();
        while let Step::Play(uri) = q.next() {
            out.push(uri);
        }
        out
    }

    #[test]
    fn shuffle_mezcla_lo_que_falta_sin_repetir_ni_saltear() {
        let list = tracks(20);
        let mut q = started(&list);
        q.next();
        q.next(); // suena list[2]
        assert_eq!(q.toggle_shuffle(&mut rng()), Some(true));
        assert_eq!(q.position(), "[3/20] ");
        let mut after = rest(&mut q);
        assert_ne!(after, list[3..], "con 17 temas, no debería quedar en orden");
        after.sort_by_key(|u| u.to_uri().unwrap());
        assert_eq!(after, list[3..]);
    }

    #[test]
    fn apagar_shuffle_retoma_el_orden_original() {
        let list = tracks(20);
        let mut q = started(&list);
        q.toggle_shuffle(&mut rng());
        let current = played(q.next());
        assert_eq!(q.toggle_shuffle(&mut rng()), Some(false));
        let i = list.iter().position(|u| *u == current).unwrap();
        let expected = list.get(i + 1).cloned();
        match q.next() {
            Step::Play(uri) => assert_eq!(Some(uri), expected),
            Step::Done => assert_eq!(expected, None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn arrancar_mezclado_usa_toda_la_lista() {
        let list = tracks(20);
        let mut seen = Vec::new();
        let mut q = Queue::new(list.clone(), true, &mut rng());
        seen.push(q.start().unwrap());
        seen.extend(rest(&mut q));
        assert!(q.shuffled());
        assert_ne!(seen, list);
        seen.sort_by_key(|u| u.to_uri().unwrap());
        assert_eq!(seen, list);
    }

    #[test]
    fn un_solo_tema_no_se_mezcla() {
        let list = tracks(1);
        let mut q = Queue::new(list.clone(), true, &mut rng());
        assert!(!q.shuffled());
        assert!(!q.can_shuffle());
        assert_eq!(q.toggle_shuffle(&mut rng()), None);
    }

    #[test]
    fn lo_encolado_suena_despues_del_actual_en_orden() {
        let list = tracks(3);
        let mut q = started(&list);
        q.enqueue(track("91"));
        q.enqueue(track("92"));
        assert_eq!(q.queued(), 2);
        assert!(is_play(&q.next(), &track("91")));
        assert_eq!(q.position(), "[cola] ");
        assert!(is_play(&q.next(), &track("92")));
        assert!(is_play(&q.next(), &list[1]));
        assert_eq!(q.queued(), 0);
    }

    #[test]
    fn el_shuffle_no_mezcla_lo_encolado() {
        let list = tracks(10);
        let mut q = started(&list);
        q.enqueue(track("91"));
        q.enqueue(track("92"));
        q.toggle_shuffle(&mut rng());
        assert!(is_play(&q.next(), &track("91")));
        assert!(is_play(&q.next(), &track("92")));
    }

    #[test]
    fn tema_unico_con_encolados_no_termina() {
        let list = tracks(1);
        let mut q = started(&list);
        q.enqueue(track("91"));
        assert!(is_play(&q.on_end(1), &track("91")));
        q.on_request_id(2);
        assert!(matches!(q.on_end(2), Step::Done));
    }

    #[test]
    fn encolar_despues_de_volver_suena_despues_del_actual() {
        let list = tracks(3);
        let mut q = started(&list);
        q.next();
        q.previous(SHORT); // vuelve a list[0]; list[1] queda adelante
        q.enqueue(track("91"));
        assert!(is_play(&q.next(), &track("91")));
        assert!(is_play(&q.next(), &list[1]));
    }

    #[test]
    fn encolar_o_mezclar_despues_del_aviso_de_precarga_precarga_de_nuevo() {
        let list = tracks(3);
        let mut q = started(&list);
        assert!(matches!(q.repreload(), Step::Nothing));
        assert!(matches!(q.on_preload_time(1), Step::Preload(u) if u == list[1]));
        q.enqueue(track("91"));
        assert!(matches!(q.repreload(), Step::Preload(u) if u == track("91")));
        // Con el tema nuevo, hasta su propio aviso no se precarga nada.
        q.next();
        q.on_request_id(2);
        assert!(matches!(q.repreload(), Step::Nothing));
    }

    #[test]
    fn reloj_del_tema() {
        let mut clock = Clock::default();
        assert_eq!(clock.elapsed(), Duration::ZERO);
        clock.paused(5_000);
        assert_eq!(clock.elapsed(), Duration::from_secs(5));
        clock.seeked(0);
        assert_eq!(clock.elapsed(), Duration::ZERO);
        clock.playing(2_000);
        assert!(clock.elapsed() >= Duration::from_secs(2));
    }
}
