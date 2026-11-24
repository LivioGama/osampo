//! Source « activite » (SP5h) : la jauge « sage » du chiot, calculee depuis
//! l inactivite X. Emet `sage` (0..100), `actif` (0/1) et `absence_min` ; garde
//! l etat de la jauge et l historique de la journee sous $XDG_STATE_HOME.

use crate::config::Reglages;
use crate::model::{Keys, Value};
use crate::source::Source;
use crate::sources::jauge::{Etat, Jauge, ReglagesJauge, ABSENT_DES_S};
use anyhow::Result;
use async_trait::async_trait;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{mpsc, watch, Notify};

/// Historique garde ce nombre de jours (fichiers plus vieux supprimes au demarrage).
const HISTORIQUE_JOURS: u64 = 30;

extern "C" {
    fn dlopen(nom: *const std::os::raw::c_char, flags: std::os::raw::c_int) -> *mut std::ffi::c_void;
    fn dlsym(h: *mut std::ffi::c_void, nom: *const std::os::raw::c_char) -> *mut std::ffi::c_void;
}

type Alloc = unsafe extern "C" fn() -> *mut x11_dl::xss::XScreenSaverInfo;
type Query = unsafe extern "C" fn(
    *mut x11_dl::xlib::Display,
    x11_dl::xlib::Window,
    *mut x11_dl::xss::XScreenSaverInfo,
) -> std::os::raw::c_int;
type Ext = unsafe extern "C" fn(
    *mut x11_dl::xlib::Display,
    *mut std::os::raw::c_int,
    *mut std::os::raw::c_int,
) -> std::os::raw::c_int;

/// Active la protection multi-fils d Xlib, une seule fois, avant le premier appel
/// Xlib du processus : le fil `ecoute-x` et les lectures XScreenSaver (spawn_blocking)
/// ouvrent chacun un display ; sans XInitThreads, la liste globale des extensions
/// est corrompue (malloc(): unaligned tcache chunk detected au demarrage).
/// Le Once bloque le second appelant tant que le premier n a pas fini.
fn init_xlib_threads(xlib: &x11_dl::xlib::Xlib) {
    static INIT: std::sync::Once = std::sync::Once::new();
    // SAFETY: XInitThreads n a aucun pre-requis hormis d etre appele avant tout autre appel Xlib.
    INIT.call_once(|| unsafe {
        (xlib.XInitThreads)();
    });
}

/// Verrou global des phases d ouverture, requete d extension et fermeture d un
/// display. XInitThreads ne suffit pas : les listes globales de libXext / libXi
/// (une entree par display) sont modifiees sans verrou par XOpenDisplay,
/// XScreenSaverQueryExtension, XIQueryVersion et XCloseDisplay. Deux fils qui
/// les executent en meme temps corrompent le tas (SIGABRT au demarrage, mesure
/// 4 a 8 % des lancements). XNextEvent, lui, n y touche pas : il tourne hors verrou.
static X_VERROU: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn verrou_x() -> std::sync::MutexGuard<'static, ()> {
    X_VERROU.lock().unwrap_or_else(|e| e.into_inner())
}

/// Symboles Xlib et Xss, charges une seule fois pour le processus.
struct Symboles {
    xlib: x11_dl::xlib::Xlib,
    ext: Ext,
    alloc: Alloc,
    query: Query,
}

/// Le crate x11-dl cherche libXss.so.2 et libXss.so, mais Debian/Ubuntu livre
/// libXss.so.1 sans lien -dev : on charge nous-memes les trois noms.
fn charger_symboles() -> Option<Symboles> {
    let xlib = x11_dl::xlib::Xlib::open().ok()?;
    // SAFETY: les symboles Xss ont les signatures declarees ci-dessus (Xss.h).
    unsafe {
        let mut h = std::ptr::null_mut();
        for nom in [c"libXss.so.1", c"libXss.so.2", c"libXss.so"] {
            h = dlopen(nom.as_ptr(), 1); // RTLD_LAZY
            if !h.is_null() {
                break;
            }
        }
        if h.is_null() {
            return None;
        }
        let (pe, pa, pq) = (
            dlsym(h, c"XScreenSaverQueryExtension".as_ptr()),
            dlsym(h, c"XScreenSaverAllocInfo".as_ptr()),
            dlsym(h, c"XScreenSaverQueryInfo".as_ptr()),
        );
        if pe.is_null() || pa.is_null() || pq.is_null() {
            return None;
        }
        Some(Symboles {
            xlib,
            ext: std::mem::transmute::<*mut std::ffi::c_void, Ext>(pe),
            alloc: std::mem::transmute::<*mut std::ffi::c_void, Alloc>(pa),
            query: std::mem::transmute::<*mut std::ffi::c_void, Query>(pq),
        })
    }
}

/// Lecteur d inactivite X : les symboles sont en cache, le display est ouvert et
/// ferme a chaque lecture (toutes les 15 s) pour se remettre seul d un X absent.
/// Appel bloquant : poll() le lance via spawn_blocking avec un delai maximal.
///
/// Risque connu : si X tombe au milieu d un appel, le gestionnaire d erreur IO
/// de Xlib appelle exit() et emporte le daemon. Pas de parade simple sans
/// installer un gestionnaire (XSetIOErrorHandler) qui ne peut pas revenir.
fn lire_inactivite_xss_s() -> Option<u64> {
    use std::os::raw::c_int;
    static SYMBOLES: OnceLock<Option<Symboles>> = OnceLock::new();
    // Un echec de chargement (X absent) est aussi memorise : les libs ne
    // reapparaissent pas en cours d execution.
    let sym = SYMBOLES.get_or_init(charger_symboles).as_ref()?;
    init_xlib_threads(&sym.xlib);
    let _verrou = verrou_x();
    // SAFETY: display ouvert et ferme ici ; info alloue par Xss, libere par XFree.
    unsafe {
        let dpy = (sym.xlib.XOpenDisplay)(std::ptr::null());
        if dpy.is_null() {
            return None;
        }
        let (mut ev, mut err): (c_int, c_int) = (0, 0);
        let mut idle = None;
        if (sym.ext)(dpy, &mut ev, &mut err) != 0 {
            let info = (sym.alloc)();
            if !info.is_null() {
                let racine = (sym.xlib.XDefaultRootWindow)(dpy);
                if (sym.query)(dpy, racine, info) != 0 {
                    idle = Some((*info).idle / 1000);
                }
                (sym.xlib.XFree)(info as *mut _);
            }
        }
        (sym.xlib.XCloseDisplay)(dpy);
        idle
    }
}

/// Epoch de la derniere vraie entree vue par l ecoute XInput2 ; 0 : ecoute pas prete.
static DERNIERE_ENTREE_S: AtomicU64 = AtomicU64::new(0);

/// L inactivite XScreenSaver ne suffit pas : relacher un XScreenSaverSuspend la
/// remet a zero sans aucune entree (Electron, dont Claude Desktop, le fait toutes
/// les ~5 min quand ses agents travaillent ; mesure le 2026-11-22, voir le handoff).
/// Les compteurs XSync (IDLETIME, DEVICEIDLETIME) et Muffin sont remis a zero pareil.
/// On date donc soi-meme la derniere entree brute (touche, clic, mouvement) ;
/// XScreenSaver ne sert que de repli tant que l ecoute n est pas prete.
fn lire_inactivite_s() -> Option<u64> {
    let xss = lire_inactivite_xss_s()?; // None : pas de X, l ecoute ne vaut rien non plus
    Some(inactivite(DERNIERE_ENTREE_S.load(Ordering::Acquire), horloge_epoch_s(), xss))
}

fn inactivite(derniere_entree: u64, maintenant: u64, xss: u64) -> u64 {
    if derniere_entree == 0 {
        xss
    } else {
        maintenant.saturating_sub(derniere_entree)
    }
}

/// Lance une fois pour le processus le fil d ecoute des entrees X.
fn demarrer_ecoute_entrees() {
    static LANCE: std::sync::Once = std::sync::Once::new();
    LANCE.call_once(|| {
        let _ = std::thread::Builder::new().name("ecoute-x".into()).spawn(|| loop {
            if !ecouter_entrees() {
                DERNIERE_ENTREE_S.store(0, Ordering::Release);
            }
            std::thread::sleep(Duration::from_secs(15));
        });
    });
}

/// Ecoute bloquante des evenements bruts XInput2 sur la racine. Revient faux si X
/// ou XInput2 manque (le fil reessaie plus tard). Meme risque que la lecture
/// XScreenSaver : si X tombe pendant XNextEvent, Xlib appelle exit().
fn ecouter_entrees() -> bool {
    use std::os::raw::c_int;
    use x11_dl::xinput2 as xi2;
    static LIBS: OnceLock<Option<(x11_dl::xlib::Xlib, xi2::XInput2)>> = OnceLock::new();
    let Some((xlib, xi)) =
        LIBS.get_or_init(|| Some((x11_dl::xlib::Xlib::open().ok()?, xi2::XInput2::open().ok()?))).as_ref()
    else {
        return false;
    };
    init_xlib_threads(xlib);
    // Depart : la derniere entree connue est celle de XScreenSaver.
    let Some(idle) = lire_inactivite_xss_s() else { return false };
    // SAFETY: display ouvert et utilise par ce seul fil ; le masque vit jusqu a XISelectEvents.
    unsafe {
        let verrou = verrou_x();
        let dpy = (xlib.XOpenDisplay)(std::ptr::null());
        if dpy.is_null() {
            return false;
        }
        let (mut opcode, mut ev, mut err): (c_int, c_int, c_int) = (0, 0, 0);
        // 2.2 et non 2.0 : un client 2.0 ne recoit pas les evenements bruts des
        // peripheriques maitres (verifie avec XTest le 2026-11-22).
        let (mut maj, mut min): (c_int, c_int) = (2, 2);
        if (xlib.XQueryExtension)(dpy, c"XInputExtension".as_ptr(), &mut opcode, &mut ev, &mut err) == 0
            || (xi.XIQueryVersion)(dpy, &mut maj, &mut min) != 0
        {
            (xlib.XCloseDisplay)(dpy);
            return false;
        }
        let mut octets = [0u8; ((xi2::XI_LASTEVENT >> 3) + 1) as usize];
        for e in [xi2::XI_RawKeyPress, xi2::XI_RawButtonPress, xi2::XI_RawMotion] {
            xi2::XISetMask(&mut octets, e);
        }
        let mut masque = xi2::XIEventMask {
            deviceid: xi2::XIAllMasterDevices,
            mask_len: octets.len() as c_int,
            mask: octets.as_mut_ptr(),
        };
        (xi.XISelectEvents)(dpy, (xlib.XDefaultRootWindow)(dpy), &mut masque, 1);
        (xlib.XFlush)(dpy);
        DERNIERE_ENTREE_S.store(horloge_epoch_s().saturating_sub(idle).max(1), Ordering::Release);
        drop(verrou); // la boucle d attente ne touche plus aux listes globales
        let mut evt: x11_dl::xlib::XEvent = std::mem::zeroed();
        loop {
            (xlib.XNextEvent)(dpy, &mut evt);
            let c = evt.generic_event_cookie;
            if c.type_ == x11_dl::xlib::GenericEvent && c.extension == opcode {
                DERNIERE_ENTREE_S.store(horloge_epoch_s(), Ordering::Release);
            }
        }
    }
}

/// Epoch en secondes : la jauge survit a un redemarrage du daemon et une veille
/// du PC (l horloge avance, X non) compte comme absence.
fn horloge_epoch_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Delai maximal d une lecture X ; au-dela, ce poll est traite comme "sans X".
const DELAI_LECTURE: Duration = Duration::from_secs(2);

type Lecteur = Arc<dyn Fn() -> Option<u64> + Send + Sync>;
type Horloge = Box<dyn FnMut() -> u64 + Send>;

/// L option --unite-s : une "minute" de la jauge dure unite_s secondes (essais).
#[derive(Clone, Copy, Debug)]
pub struct Surcharge {
    pub unite_s: u64,
}

/// $XDG_STATE_HOME/osampo, repli ~/.local/state.
pub fn dossier_etat() -> PathBuf {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(x) if !x.is_empty() => PathBuf::from(x),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"),
    };
    base.join("osampo")
}

/// Heure locale "AAAA-MM-JJ" et "HH:MM" d un epoch.
fn date_heure_locales(epoch: u64) -> (String, String) {
    let t = epoch as libc::time_t;
    // SAFETY: tm est entierement rempli par localtime_r a partir de t.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    (
        format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday),
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min),
    )
}

/// Cote PC du reglage de la jauge (SPEC-009) : la valeur part dans le canal et la source est
/// reveillee, sans attendre son intervalle (jusqu a 15 s).
#[derive(Clone)]
pub struct EmetteurJauge {
    tx: mpsc::UnboundedSender<i64>,
    reveil: Arc<Notify>,
}

impl EmetteurJauge {
    /// Faux si la source n existe plus.
    pub fn regler(&self, valeur: i64) -> bool {
        let ok = self.tx.send(valeur).is_ok();
        self.reveil.notify_one();
        ok
    }
}

/// Cote source du reglage de la jauge.
pub struct RecepteurJauge {
    rx: mpsc::UnboundedReceiver<i64>,
    reveil: Arc<Notify>,
}

#[cfg(test)]
impl RecepteurJauge {
    /// Le prochain reglage en attente, s'il y en a un.
    pub fn recu(&mut self) -> Option<i64> {
        self.rx.try_recv().ok()
    }
}

pub fn canal_jauge() -> (EmetteurJauge, RecepteurJauge) {
    let (tx, rx) = mpsc::unbounded_channel();
    let reveil = Arc::new(Notify::new());
    (EmetteurJauge { tx, reveil: reveil.clone() }, RecepteurJauge { rx, reveil })
}

pub struct SourceActivite {
    jauge: Jauge,
    /// Reglages de la jauge demandes depuis le PC, appliques au prochain poll.
    reglages_jauge: RecepteurJauge,
    /// Reglages courants, relus a chaque poll : une relecture du fichier
    /// s applique sans redemarrer la source.
    reglages: watch::Receiver<Reglages>,
    surcharge: Surcharge,
    lecteur: Lecteur,
    horloge: Horloge,
    intervalle: Duration,
    x_signale: bool,
    /// Dernier etat issu d une vraie lecture X (None : X jamais lu).
    dernier: Option<Etat>,
    /// Vrai tant qu une lecture lancee par spawn_blocking n est pas revenue :
    /// au plus un fil bloque si X se fige.
    en_cours: Arc<AtomicBool>,
    /// None : ni persistance ni historique.
    dossier: Option<PathBuf>,
    /// Derniere minute ecrite dans l historique, derniere sauvegarde de la jauge.
    minute_historique: Option<u64>,
    sauvegarde: Option<(u64, bool)>,
}

impl SourceActivite {
    pub fn new(reglages: watch::Receiver<Reglages>, surcharge: Surcharge, jauge: RecepteurJauge) -> Self {
        demarrer_ecoute_entrees();
        Self::avec(
            reglages,
            surcharge,
            jauge,
            Arc::new(lire_inactivite_s),
            Box::new(horloge_epoch_s),
            Some(dossier_etat()),
        )
    }

    fn avec(
        reglages: watch::Receiver<Reglages>,
        surcharge: Surcharge,
        jauge_pc: RecepteurJauge,
        lecteur: Lecteur,
        horloge: Horloge,
        dossier: Option<PathBuf>,
    ) -> Self {
        let unite_s = surcharge.unite_s.max(1);
        let jauge = dossier.as_ref().and_then(|d| lire_jauge(&d.join("jauge_osampo")));
        SourceActivite {
            jauge: jauge.unwrap_or_else(|| Jauge::new(100.0)),
            reglages_jauge: jauge_pc,
            intervalle: Duration::from_secs((unite_s / 2).clamp(1, 15)),
            reglages,
            surcharge: Surcharge { unite_s },
            lecteur,
            horloge,
            x_signale: false,
            dernier: None,
            en_cours: Arc::new(AtomicBool::new(false)),
            dossier,
            minute_historique: None,
            sauvegarde: None,
        }
    }

    /// Temps de la jauge : une "minute" dure unite_s secondes.
    fn virtuel(&self, s: u64) -> u64 {
        s.saturating_mul(60) / self.surcharge.unite_s
    }

    fn ecrire_jauge(&self, d: &std::path::Path) {
        let debut = self.jauge.debut_absence().map_or("-".to_string(), |x| x.to_string());
        let _ = std::fs::create_dir_all(d);
        let _ = std::fs::write(d.join("jauge_osampo"), format!("{} {debut}\n", self.jauge.niveau()));
    }

    /// Dernier reglage recu du PC, s il y en a un : applique avant l echantillon, fichier
    /// ecrit aussitot (SPEC-009). `absent` : une absence est en cours.
    fn appliquer_reglage(&mut self, epoch: u64, absent: bool) -> Option<i64> {
        let mut valeur = None;
        while let Ok(v) = self.reglages_jauge.rx.try_recv() {
            valeur = Some(v);
        }
        let v = valeur?;
        self.jauge.regler(v as f64, self.virtuel(epoch), absent);
        if let Some(d) = &self.dossier {
            self.ecrire_jauge(d);
        }
        Some(v)
    }

    fn noter(&mut self, epoch: u64, e: &Etat) {
        let Some(d) = self.dossier.clone() else { return };
        let minute = epoch / 60;
        if self.sauvegarde.is_none_or(|(m, a)| m != minute || a != e.actif) {
            self.sauvegarde = Some((minute, e.actif));
            self.ecrire_jauge(&d);
        }
        if self.minute_historique.is_none() {
            purger_historique(&d.join("activite"));
        }
        if self.minute_historique != Some(minute) {
            self.minute_historique = Some(minute);
            let (jour, hm) = date_heure_locales(epoch);
            let dh = d.join("activite");
            let _ = std::fs::create_dir_all(&dh);
            if let Ok(mut f) =
                std::fs::OpenOptions::new().create(true).append(true).open(dh.join(format!("{jour}.csv")))
            {
                let _ = writeln!(f, "{hm},{},{}", e.actif as u8, e.sage);
            }
        }
    }
}

/// "niveau debut" ("-" sans absence en cours) ; None si absent ou illisible.
fn lire_jauge(chemin: &std::path::Path) -> Option<Jauge> {
    let t = std::fs::read_to_string(chemin).ok()?;
    let mut it = t.split_whitespace();
    let niveau: f64 = it.next()?.parse().ok()?;
    let debut = match it.next()? {
        "-" => None,
        x => Some(x.parse().ok()?),
    };
    niveau.is_finite().then(|| Jauge::restaurer(niveau, debut))
}

fn purger_historique(dossier: &std::path::Path) {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return };
    let limite = Duration::from_secs(HISTORIQUE_JOURS * 86400);
    for e in entrees.flatten() {
        let vieux = e.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok());
        if vieux.is_some_and(|a| a > limite) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[async_trait]
impl Source for SourceActivite {
    fn id(&self) -> &str {
        "activite"
    }

    fn keys(&self) -> &'static [&'static str] {
        &["sage", "actif", "absence_min", "absence_finie_min", "absence_finie_age_min"]
    }

    fn interval(&self) -> Duration {
        self.intervalle
    }

    fn reveil(&self) -> Option<Arc<Notify>> {
        Some(self.reglages_jauge.reveil.clone())
    }

    async fn poll(&mut self) -> Result<Keys> {
        let r = {
            let g = self.reglages.borrow();
            ReglagesJauge { vide_apres_min: g.vide_apres_min, courte_min: g.courte_min, longue_min: g.longue_min }
        };
        let epoch = (self.horloge)();
        // Lecture precedente encore pendue : on ne relance pas, ce poll vaut echec.
        let lu = if self.en_cours.swap(true, Ordering::AcqRel) {
            None
        } else {
            let lecteur = self.lecteur.clone();
            let en_cours = self.en_cours.clone();
            let tache = tokio::task::spawn_blocking(move || {
                // Remis a faux au retour, meme si le lecteur panique.
                struct Fin(Arc<AtomicBool>);
                impl Drop for Fin {
                    fn drop(&mut self) {
                        self.0.store(false, Ordering::Release);
                    }
                }
                let _fin = Fin(en_cours);
                lecteur()
            });
            tokio::time::timeout(DELAI_LECTURE, tache).await.ok().and_then(|r| r.ok()).flatten()
        };
        let e = match lu {
            Some(idle_s) => {
                self.x_signale = false;
                let (iv, nv) = (self.virtuel(idle_s), self.virtuel(epoch));
                self.appliquer_reglage(epoch, iv >= ABSENT_DES_S);
                let e = self.jauge.echantillon(iv, nv, &r);
                self.dernier = Some(e);
                self.noter(epoch, &e);
                e
            }
            None => {
                // Echec passager : on reemet le dernier etat connu, sans inventer
                // d absence (elle rechargerait la jauge). Sans X jamais lu : jauge
                // courante, actif. Un seul message par panne.
                let absent = self.jauge.debut_absence().is_some();
                let regle = self.appliquer_reglage(epoch, absent);
                self.jauge.sans_lecture(self.virtuel(epoch));
                // La valeur reglee remplace aussi le dernier etat reemis.
                if let (Some(v), Some(d)) = (regle, self.dernier.as_mut()) {
                    d.sage = v;
                }
                let e = self.dernier.unwrap_or(Etat {
                    sage: self.jauge.niveau().round() as i64,
                    actif: true,
                    absence_min: 0,
                    absence_finie_min: 0,
                    absence_finie_age_min: -1,
                });
                if !self.x_signale {
                    eprintln!(
                        "activite: inactivite X illisible (DISPLAY={:?}, XAUTHORITY={:?}), sage = {} ; \
                         verifier DISPLAY et XAUTHORITY de l unite (systemctl --user edit osampo-daemon)",
                        std::env::var("DISPLAY").unwrap_or_default(),
                        std::env::var("XAUTHORITY").unwrap_or_default(),
                        e.sage
                    );
                    self.x_signale = true;
                }
                e
            }
        };
        let mut k = Keys::new();
        k.insert("sage".into(), Value::Int(e.sage));
        k.insert("actif".into(), Value::Int(e.actif as i64));
        k.insert("absence_min".into(), Value::Int(e.absence_min));
        k.insert("absence_finie_min".into(), Value::Int(e.absence_finie_min));
        k.insert("absence_finie_age_min".into(), Value::Int(e.absence_finie_age_min));
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    const T0: u64 = 1_800_000_000;

    fn sur(unite_s: u64) -> (watch::Receiver<Reglages>, Surcharge) {
        let (_tx, rx) = watch::channel(Reglages::default());
        (rx, Surcharge { unite_s })
    }

    fn dossier_temp(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sp5h_activite_{}_{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Source a horloge (epoch) et inactivite pilotees par le test.
    fn pilotee(dossier: Option<PathBuf>) -> (SourceActivite, Arc<AtomicU64>, Arc<AtomicU64>) {
        let (s, t, idle, _pc) = pilotee_pc(dossier);
        (s, t, idle)
    }

    fn pilotee_pc(
        dossier: Option<PathBuf>,
    ) -> (SourceActivite, Arc<AtomicU64>, Arc<AtomicU64>, EmetteurJauge) {
        let t = Arc::new(AtomicU64::new(T0));
        let idle = Arc::new(AtomicU64::new(0));
        let (t2, i2) = (t.clone(), idle.clone());
        let (r, su) = sur(60);
        let (pc, recepteur) = canal_jauge();
        let s = SourceActivite::avec(
            r,
            su,
            recepteur,
            Arc::new(move || Some(i2.load(Ordering::SeqCst))),
            Box::new(move || t2.load(Ordering::SeqCst)),
            dossier,
        );
        (s, t, idle, pc)
    }

    fn int(k: &Keys, cle: &str) -> i64 {
        match k.get(cle) {
            Some(Value::Int(v)) => *v,
            autre => panic!("{cle}: {autre:?}"),
        }
    }

    /// Absence de `min` minutes, un echantillon toutes les 30 s.
    async fn absence(s: &mut SourceActivite, t: &AtomicU64, idle: &AtomicU64, min: u64) -> Keys {
        let mut k = Keys::new();
        for _ in 0..(min * 2) {
            t.fetch_add(30, Ordering::SeqCst);
            idle.fetch_add(30, Ordering::SeqCst);
            k = s.poll().await.unwrap();
        }
        k
    }

    #[tokio::test]
    async fn emet_sage_actif_et_absence() {
        let (mut s, t, idle) = pilotee(None);
        let k = s.poll().await.unwrap();
        assert_eq!((int(&k, "sage"), int(&k, "actif"), int(&k, "absence_min")), (100, 1, 0));
        let k = absence(&mut s, &t, &idle, 7).await;
        assert_eq!((int(&k, "actif"), int(&k, "absence_min")), (0, 7));
        assert_eq!(s.keys(), &["sage", "actif", "absence_min", "absence_finie_min", "absence_finie_age_min"]);
        assert_eq!(s.id(), "activite");
    }

    #[tokio::test]
    async fn jauge_relue_apres_redemarrage() {
        let d = dossier_temp("relue");
        let (mut s, t, idle) = pilotee(Some(d.clone()));
        // vider la jauge : 90 min d activite
        for _ in 0..180 {
            t.fetch_add(30, Ordering::SeqCst);
            s.poll().await.unwrap();
        }
        let k = absence(&mut s, &t, &idle, 10).await;
        assert_eq!(int(&k, "sage"), 33);
        drop(s);
        let (mut s2, t2, idle2) = pilotee(Some(d.clone()));
        t2.store(t.load(Ordering::SeqCst), Ordering::SeqCst);
        idle2.store(idle.load(Ordering::SeqCst), Ordering::SeqCst);
        let k = absence(&mut s2, &t2, &idle2, 5).await;
        assert_eq!(int(&k, "absence_min"), 15);
        assert_eq!(int(&k, "sage"), 50);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn x_illisible_ne_recharge_pas() {
        use std::sync::atomic::AtomicBool;
        let t = Arc::new(AtomicU64::new(T0));
        let t2 = t.clone();
        let x_ok = Arc::new(AtomicBool::new(true));
        let x2 = x_ok.clone();
        let (r, su) = sur(60);
        let mut s = SourceActivite::avec(
            r,
            su,
            canal_jauge().1,
            Arc::new(move || x2.load(Ordering::SeqCst).then_some(0)),
            Box::new(move || t2.load(Ordering::SeqCst)),
            None,
        );
        for _ in 0..90 {
            t.fetch_add(30, Ordering::SeqCst);
            s.poll().await.unwrap();
        }
        let avant = s.poll().await.unwrap();
        x_ok.store(false, Ordering::SeqCst);
        let mut k = Keys::new();
        for _ in 0..80 {
            t.fetch_add(30, Ordering::SeqCst);
            k = s.poll().await.unwrap();
        }
        assert_eq!(int(&k, "sage"), int(&avant, "sage"));
        assert_eq!(int(&k, "actif"), 1);
        // I3 : au retour de X, la panne n'est pas comptee comme absence.
        x_ok.store(true, Ordering::SeqCst);
        t.fetch_add(30, Ordering::SeqCst);
        let k = s.poll().await.unwrap();
        assert!(int(&k, "sage") <= int(&avant, "sage"), "la panne a recharge la jauge");
    }

    #[tokio::test]
    async fn historique_une_ligne_par_minute() {
        let d = dossier_temp("historique");
        let (mut s, t, _idle) = pilotee(Some(d.clone()));
        t.store(T0 - T0 % 60, Ordering::SeqCst);
        for _ in 0..6 {
            s.poll().await.unwrap();
            t.fetch_add(30, Ordering::SeqCst);
        }
        let (jour, _) = date_heure_locales(T0);
        let texte = std::fs::read_to_string(d.join("activite").join(format!("{jour}.csv"))).unwrap();
        let lignes: Vec<&str> = texte.lines().collect();
        assert_eq!(lignes.len(), 3, "{texte}");
        for l in &lignes {
            let c: Vec<&str> = l.split(',').collect();
            let hm = c[0].as_bytes();
            assert!(c.len() == 3 && hm.len() == 5 && hm[2] == b':', "{texte}");
            assert_eq!(c[1], "1", "{texte}");
            assert!(c[2].parse::<i64>().is_ok_and(|v| (0..=100).contains(&v)), "{texte}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn intervalle_suit_l_unite() {
        let a = |u: u64| {
            let (r, s) = sur(u);
            SourceActivite::avec(r, s, canal_jauge().1, Arc::new(|| None), Box::new(|| 0), None)
        };
        assert_eq!(a(60).interval(), Duration::from_secs(15));
        assert_eq!(a(10).interval(), Duration::from_secs(5));
        assert_eq!(a(1).interval(), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn lecture_x_pendue_vaut_sans_x() {
        let mut s = SourceActivite::avec(
            sur(60).0,
            sur(60).1,
            canal_jauge().1,
            Arc::new(|| {
                std::thread::sleep(Duration::from_secs(3));
                Some(0)
            }),
            Box::new(|| 0),
            None,
        );
        let k = s.poll().await.unwrap();
        assert_eq!((int(&k, "sage"), int(&k, "actif")), (100, 1));
    }

    #[tokio::test]
    async fn sans_x_emet_la_jauge_sans_absence() {
        let mut s = {
            let (r, su) = sur(60);
            SourceActivite::avec(r, su, canal_jauge().1, Arc::new(|| None), Box::new(|| 0), None)
        };
        for _ in 0..3 {
            let k = s.poll().await.unwrap();
            assert_eq!((int(&k, "sage"), int(&k, "actif"), int(&k, "absence_min")), (100, 1, 0));
        }
        assert_eq!(s.interval(), Duration::from_secs(15));
    }

    #[tokio::test]
    async fn echec_apres_lecture_valide_reemet_la_derniere_valeur() {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        let t = Arc::new(AtomicU64::new(0));
        let t2 = t.clone();
        let x_ok = Arc::new(AtomicBool::new(true));
        let x2 = x_ok.clone();
        let mut s = SourceActivite::avec(
            sur(60).0,
            sur(60).1,
            canal_jauge().1,
            Arc::new(move || x2.load(Ordering::SeqCst).then_some(0)),
            Box::new(move || t2.load(Ordering::SeqCst)),
            None,
        );
        let mut avant = Keys::new();
        for i in 0..=9 {
            t.store(60 * i, Ordering::SeqCst);
            avant = s.poll().await.unwrap();
        }
        assert_eq!(int(&avant, "sage"), 90);
        x_ok.store(false, Ordering::SeqCst);
        t.store(900, Ordering::SeqCst);
        let k = s.poll().await.unwrap();
        assert_eq!(int(&k, "sage"), 90);
    }

    #[tokio::test]
    async fn pas_de_seconde_lecture_tant_que_la_premiere_est_pendue() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let appels = Arc::new(AtomicU32::new(0));
        let a2 = appels.clone();
        let mut s = SourceActivite::avec(
            sur(60).0,
            sur(60).1,
            canal_jauge().1,
            Arc::new(move || {
                a2.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(4));
                Some(0)
            }),
            Box::new(|| 0),
            None,
        );
        s.poll().await.unwrap(); // delai depasse au bout de 2 s, le fil dort encore
        s.poll().await.unwrap();
        assert_eq!(appels.load(Ordering::SeqCst), 1, "un seul fil bloque");
    }

    // Verification manuelle : cargo test -- --ignored reel --nocapture
    #[test]
    #[ignore]
    fn reel() {
        println!("inactivite X reelle: {:?} s", lire_inactivite_s());
    }

    #[test]
    fn inactivite_suit_l_ecoute_et_ignore_xss() {
        // Ecoute pas prete : repli sur XScreenSaver.
        assert_eq!(inactivite(0, 1000, 42), 42);
        // Ecoute prete : un XScreenSaver remis a zero (xss = 1) ne compte pas.
        assert_eq!(inactivite(400, 1000, 1), 600);
        // Horloge en retard sur la derniere entree : pas de debordement.
        assert_eq!(inactivite(1000, 900, 5), 0);
    }

    // Verification manuelle : lacher clavier et souris, l ecoute doit rester
    // croissante pendant qu un XScreenSaverSuspend remet xss a zero.
    // cargo test -- --ignored ecoute_reelle --nocapture
    #[test]
    #[ignore]
    fn ecoute_reelle() {
        demarrer_ecoute_entrees();
        for _ in 0..40 {
            std::thread::sleep(Duration::from_secs(1));
            println!("ecoute={:?} s  xss={:?} s", lire_inactivite_s(), lire_inactivite_xss_s());
        }
    }

    /// SPEC-009 : le reglage est applique au poll, ecrit aussitot dans le fichier d'etat,
    /// et la jauge repart de la valeur reglee.
    #[tokio::test]
    async fn reglage_applique_ecrit_et_repart_de_la_valeur() {
        let d = dossier_temp("reglage");
        let (mut s, t, _idle, pc) = pilotee_pc(Some(d.clone()));
        assert_eq!(int(&s.poll().await.unwrap(), "sage"), 100);
        assert!(pc.regler(0));
        t.fetch_add(1, Ordering::SeqCst);
        let k = s.poll().await.unwrap();
        assert_eq!(int(&k, "sage"), 0);
        let f = std::fs::read_to_string(d.join("jauge_osampo")).unwrap();
        assert!(f.starts_with("0 "), "{f}");
        // On regle a 60, puis 45 min d'activite (90 min pour tout vider) : 60 - 50 = 10.
        assert!(pc.regler(60));
        t.fetch_add(1, Ordering::SeqCst);
        assert_eq!(int(&s.poll().await.unwrap(), "sage"), 60);
        for _ in 0..90 {
            t.fetch_add(30, Ordering::SeqCst);
            s.poll().await.unwrap();
        }
        let k = s.poll().await.unwrap();
        assert_eq!(int(&k, "sage"), 10);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn reglage_pendant_une_absence_la_recommence() {
        let (mut s, t, idle, pc) = pilotee_pc(None);
        s.poll().await.unwrap();
        let k = absence(&mut s, &t, &idle, 10).await;
        assert_eq!(int(&k, "absence_min"), 10);
        assert!(pc.regler(0));
        let k = s.poll().await.unwrap();
        assert_eq!((int(&k, "sage"), int(&k, "absence_min")), (0, 0));
        let k = absence(&mut s, &t, &idle, 15).await;
        assert_eq!((int(&k, "sage"), int(&k, "absence_min")), (50, 15));
    }

    #[tokio::test]
    async fn reglage_remplace_le_dernier_etat_quand_x_est_illisible() {
        use std::sync::atomic::AtomicBool;
        let x_ok = Arc::new(AtomicBool::new(true));
        let x2 = x_ok.clone();
        let (r, su) = sur(60);
        let (pc, recepteur) = canal_jauge();
        let mut s = SourceActivite::avec(
            r,
            su,
            recepteur,
            Arc::new(move || x2.load(Ordering::SeqCst).then_some(0)),
            Box::new(|| T0),
            None,
        );
        assert_eq!(int(&s.poll().await.unwrap(), "sage"), 100);
        x_ok.store(false, Ordering::SeqCst);
        assert!(pc.regler(30));
        assert_eq!(int(&s.poll().await.unwrap(), "sage"), 30);
        assert_eq!(int(&s.poll().await.unwrap(), "sage"), 30);
    }

    #[tokio::test]
    async fn le_reglage_reveille_la_source() {
        let (s, _t, _idle, pc) = pilotee_pc(None);
        let reveil = s.reveil().expect("la source activite se laisse reveiller");
        pc.regler(10);
        tokio::time::timeout(Duration::from_millis(200), reveil.notified())
            .await
            .expect("un reglage reveille la source");
    }
}
