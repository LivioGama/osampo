//! Emetteur BLE (SP5d2) : les trames du daemon partent vers la board par
//! Bluetooth, la meme ligne JSON que sur l USB, ecrite en morceaux sur une
//! caracteristique GATT ; la board les recolle jusqu au saut de ligne.
//!
//! Verifie par SP0b (2026-11-20, exemple jetable du commit abe39a0) : btleplug 0.13.1
//! sur hci0 ecrit sur la 1.83 (NimBLE), sans et avec reponse, 300 octets en
//! morceaux de 20 recus intacts ; MTU negocie 247 (R3 leve, R4 sans objet).
//!
//! Appairage (B8) : la premiere board trouvee par son nom et son service est
//! retenue dans un fichier d etat ; ensuite seule cette adresse est acceptee.
//! Supprimer le fichier pour en appairer une autre.

use super::lien_board::CoteTransport;
use crate::model::Frame;
use anyhow::{Context, Result};
use btleplug::api::{Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Manager, Peripheral};
use futures::{Stream, StreamExt};
use std::future::Future;
use std::pin::Pin;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{broadcast, watch, OnceCell};

/// Service et caracteristique du chiot (memes valeurs que platform/board_183_sq/src/ble.cpp).
pub const SERVICE: u128 = 0x7a6d0001_5c3e_4b8e_9d2a_74616d6f7368;
pub const LIGNES: u128 = 0x7a6d0002_5c3e_4b8e_9d2a_74616d6f7368;
/// Voie montante (SPEC-001) : la board y notifie ses lignes d etat et ses evenements.
pub const ETAT: u128 = 0x7a6d0003_5c3e_4b8e_9d2a_74616d6f7368;

/// btleplug ne rend pas le MTU negocie : 20 octets passent avec tout MTU, et
/// 300 octets partent en 8 ms (SP0b), largement assez pour 2 trames par seconde.
pub const TAILLE_MORCEAU: usize = 20;

/// Pause apres `echecs` essais rates d affilee : 5 s, puis de plus en plus
/// espaces jusqu a 30 s, pour ne pas scanner en continu quand la board est
/// eteinte (le scan gene l audio et les peripheriques Bluetooth du PC).
pub fn delai_apres_echecs(echecs: u32) -> Duration {
    Duration::from_secs(match echecs {
        0 | 1 => 5,
        2 => 10,
        3 => 20,
        _ => 30,
    })
}

/// Verrou exclusif : un seul emetteur BLE par board sur ce PC. Deux daemons
/// ecriraient leurs morceaux sur la meme liaison BlueZ et les melangeraient.
/// Le verrou tombe avec le File (fin du processus comprise).
pub fn prendre_verrou(chemin: &Path) -> Result<std::fs::File> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("creation de {}", d.display()))?;
    }
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(chemin)
        .with_context(|| format!("ouverture de {}", chemin.display()))?;
    // SAFETY: fd valide tant que f vit ; flock ne lit ni n ecrit de memoire.
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        anyhow::bail!("un autre daemon emet deja vers cette board ({})", chemin.display());
    }
    Ok(f)
}

/// $XDG_RUNTIME_DIR/osampo/ble_<widget>.lock, repli /tmp.
fn chemin_verrou(widget: &str) -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("osampo").join(format!("ble_{widget}.lock"))
}

/// Duree d un scan quand la board n est pas encore dans le cache de BlueZ.
const DUREE_SCAN: Duration = Duration::from_secs(5);

/// Une ecriture qui ne revient pas : la liaison est consideree perdue.
const DELAI_ECRITURE: Duration = Duration::from_secs(10);

/// Ce dont la boucle d emission a besoin d une liaison ; un faux en tests.
#[async_trait::async_trait]
pub trait Canal: Send {
    /// Ecrit un morceau d au plus TAILLE_MORCEAU octets. Err : liaison perdue.
    async fn envoyer(&mut self, octets: &[u8]) -> Result<()>;
    /// Prochaine notification `ETAT` (octets bruts). Err : liaison perdue.
    /// Annulable sans perte. Sans abonnement, elle ne rend jamais.
    async fn notification(&mut self) -> Result<Vec<u8>>;
    /// Vrai si la liaison est abonnee a `ETAT` (le lien est alors « connecte »).
    fn abonne(&self) -> bool;
}

/// Les morceaux successifs d une ligne, d au plus `taille` octets.
pub fn morceaux(ligne: &[u8], taille: usize) -> impl Iterator<Item = &[u8]> {
    ligne.chunks(taille.max(1))
}

/// $XDG_STATE_HOME/osampo/ble_<widget>, repli ~/.local/state.
pub fn chemin_adresse(widget: &str) -> PathBuf {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"),
    };
    base.join("osampo").join(format!("ble_{widget}"))
}

/// L adresse retenue, None si le fichier manque ou est vide.
pub fn lire_adresse(chemin: &Path) -> Option<String> {
    let s = std::fs::read_to_string(chemin).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

pub fn ecrire_adresse(chemin: &Path, adresse: &str) -> Result<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("creation de {}", d.display()))?;
    }
    std::fs::write(chemin, format!("{adresse}\n")).with_context(|| format!("ecriture de {}", chemin.display()))
}

/// Vrai si `cause` differe de la derniere signalee (un message par cause, pas un toutes les 5 s).
fn a_signaler(deja: &mut Option<String>, cause: String) -> bool {
    if deja.as_deref() == Some(cause.as_str()) {
        return false;
    }
    *deja = Some(cause);
    true
}

/// Sert la board `nom` jusqu a la fermeture du broadcast, avec le contrat commun (lignes
/// montantes, commandes, lien). Ne s arrete jamais sur une erreur : journal une fois par cause,
/// nouvel essai de plus en plus espace (delai_apres_echecs). Un seul emetteur par board
/// (prendre_verrou).
pub async fn servir_lien(
    nom: String,
    mut rx: broadcast::Receiver<Frame>,
    cfg: watch::Receiver<Option<String>>,
    cote: CoteTransport,
) {
    let verrou = chemin_verrou(&nom);
    let mut refus_signale = false;
    let _garde = loop {
        match prendre_verrou(&verrou) {
            Ok(f) => break f,
            Err(e) => {
                if !refus_signale {
                    eprintln!("ble: {e:#} ; pas d emission Bluetooth tant qu il tourne (essai chaque minute)");
                    refus_signale = true;
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                    r = attendre_fermeture(&mut rx) => if r { return },
                }
            }
        }
    };
    let fichier = chemin_adresse(&nom);
    // Un seul gestionnaire pour toute la vie du daemon : chaque Manager::new
    // ouvre une connexion D-Bus que btleplug ne referme jamais (~256 au plus).
    let manager: Arc<OnceCell<Manager>> = Arc::new(OnceCell::new());
    servir_avec(
        move || connecter(manager.clone(), nom.clone(), fichier.clone()),
        rx,
        cfg,
        cote,
        delai_apres_echecs,
    )
    .await
}

async fn servir_avec<C, F, Fut>(
    mut connecter: F,
    mut rx: broadcast::Receiver<Frame>,
    mut cfg: watch::Receiver<Option<String>>,
    mut cote: CoteTransport,
    pause: impl Fn(u32) -> Duration,
) where
    C: Canal,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<C>>,
{
    let mut deja_signale: Option<String> = None;
    let mut echecs: u32 = 0;
    loop {
        match connecter().await {
            Ok(mut canal) => {
                eprintln!("ble: connecte");
                deja_signale = None;
                echecs = 0;
                cote.nouvelle_liaison();
                cote.lien(canal.abonne());
                let fin = emettre(&mut canal, true, &mut rx, &mut cfg, &mut cote).await;
                cote.lien(false);
                match fin {
                    Ok(()) => return, // broadcast ferme : le daemon s arrete
                    Err(e) => eprintln!("ble: liaison perdue ({e:#}), nouvel essai dans {:?}", pause(0)),
                }
            }
            Err(e) => {
                echecs = echecs.saturating_add(1);
                let cause = format!("{e:#}");
                if a_signaler(&mut deja_signale, cause.clone()) {
                    eprintln!("ble: {cause}, nouvel essai dans {:?} puis de plus en plus espace", pause(echecs));
                }
            }
        }
        // broadcast ferme pendant la pause : on s arrete sans attendre.
        tokio::select! {
            _ = tokio::time::sleep(pause(echecs)) => {}
            r = attendre_fermeture(&mut rx) => if r { return },
        }
    }
}

/// Vrai si le broadcast est ferme ; sinon ne rend jamais (les trames vues
/// pendant la pause sont jetees : seule la plus recente servira).
pub(crate) async fn attendre_fermeture(rx: &mut broadcast::Receiver<Frame>) -> bool {
    loop {
        match rx.recv().await {
            Err(RecvError::Closed) => return true,
            _ => continue,
        }
    }
}

/// Meme contrat que l emetteur serie : un saut de ligne si `resynchro` (la
/// board jette une ligne coupee), la ligne cfg, puis chaque trame, chaque
/// changement de cfg et chaque commande. Les notifications `ETAT` vont au
/// consommateur du lien. Partage avec la board simulee.
pub(crate) async fn emettre<C: Canal>(
    canal: &mut C,
    resynchro: bool,
    rx: &mut broadcast::Receiver<Frame>,
    cfg: &mut watch::Receiver<Option<String>>,
    cote: &mut CoteTransport,
) -> Result<()> {
    if resynchro {
        envoyer_ligne(canal, b"\n").await?;
    }
    envoyer_cfg(canal, cfg).await?;
    let mut cfg_vivante = true;
    let mut commandes_vivantes = true;
    loop {
        tokio::select! {
            f = la_plus_recente(rx) => match f {
                Some(f) => envoyer_ligne(canal, f.to_line().as_bytes()).await?,
                None => return Ok(()),
            },
            r = cfg.changed(), if cfg_vivante => match r {
                Ok(()) => envoyer_cfg(canal, cfg).await?,
                Err(_) => cfg_vivante = false,
            },
            c = cote.commande(), if commandes_vivantes => match c {
                Some(l) => envoyer_ligne(canal, format!("{l}\n").as_bytes()).await?,
                None => commandes_vivantes = false,
            },
            n = canal.notification() => cote.recevoir(&n?),
        }
    }
}

async fn envoyer_cfg<C: Canal>(canal: &mut C, cfg: &mut watch::Receiver<Option<String>>) -> Result<()> {
    let ligne = cfg.borrow_and_update().clone();
    if let Some(l) = ligne {
        envoyer_ligne(canal, format!("{l}\n").as_bytes()).await?;
    }
    Ok(())
}

async fn envoyer_ligne<C: Canal>(canal: &mut C, ligne: &[u8]) -> Result<()> {
    for m in morceaux(ligne, TAILLE_MORCEAU) {
        tokio::time::timeout(DELAI_ECRITURE, canal.envoyer(m))
            .await
            .map_err(|_| anyhow::anyhow!("ecriture bloquee depuis {DELAI_ECRITURE:?}"))??;
    }
    Ok(())
}

/// La trame la plus recente du broadcast (meme regle que l emetteur serie).
async fn la_plus_recente(rx: &mut broadcast::Receiver<Frame>) -> Option<Frame> {
    let mut derniere = loop {
        match rx.recv().await {
            Ok(f) => break f,
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => return None,
        }
    };
    loop {
        match rx.try_recv() {
            Ok(f) => derniere = f,
            Err(TryRecvError::Lagged(_)) => continue,
            Err(_) => return Some(derniere),
        }
    }
}

/// La liaison reelle : un peripherique btleplug connecte et sa caracteristique.
struct Liaison {
    board: Peripheral,
    caracteristique: Characteristic,
    /// None : board sans caracteristique `ETAT` (ancien micrologiciel), lien jamais « connecte ».
    notifications: Option<Pin<Box<dyn Stream<Item = btleplug::api::ValueNotification> + Send>>>,
}

/// Cadence de verification de la connexion : un lien coupe est vu en moins d une seconde
/// (SPEC-003), sans attendre une ecriture ni une notification.
const SONDE_LIEN: Duration = Duration::from_millis(500);

#[async_trait::async_trait]
impl Canal for Liaison {
    async fn envoyer(&mut self, octets: &[u8]) -> Result<()> {
        self.board
            .write(&self.caracteristique, octets, WriteType::WithoutResponse)
            .await
            .context("ecriture GATT")
    }

    async fn notification(&mut self) -> Result<Vec<u8>> {
        let Some(flux) = self.notifications.as_mut() else {
            return std::future::pending().await;
        };
        let mut sonde = tokio::time::interval(SONDE_LIEN);
        loop {
            tokio::select! {
                n = flux.next() => match n {
                    Some(n) if n.uuid.as_u128() == ETAT => return Ok(n.value),
                    Some(_) => {}
                    None => anyhow::bail!("flux de notifications ferme"),
                },
                _ = sonde.tick() => {
                    if !self.board.is_connected().await.unwrap_or(false) {
                        anyhow::bail!("board deconnectee");
                    }
                }
            }
        }
    }

    fn abonne(&self) -> bool {
        self.notifications.is_some()
    }
}

impl Drop for Liaison {
    fn drop(&mut self) {
        let b = self.board.clone();
        tokio::spawn(async move {
            let _ = b.disconnect().await;
        });
    }
}

/// Trouve la board (adresse retenue, sinon nom et service), s y connecte et
/// rend la liaison. Une premiere board trouvee est retenue dans `fichier`.
async fn connecter(manager: Arc<OnceCell<Manager>>, nom: String, fichier: PathBuf) -> Result<Liaison> {
    let manager = manager
        .get_or_try_init(|| async { Manager::new().await.context("gestionnaire BLE indisponible") })
        .await?;
    let adaptateur = manager
        .adapters()
        .await
        .context("liste des adaptateurs")?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("aucun adaptateur BLE"))?;
    let retenue = lire_adresse(&fichier);

    let mut board = chercher(&adaptateur, &nom, retenue.as_deref()).await?;
    if board.is_none() {
        // Scan filtre sur le service du chiot, toujours arrete, meme sur erreur.
        let filtre = ScanFilter { services: vec![uuid::Uuid::from_u128(SERVICE)] };
        adaptateur.start_scan(filtre).await.context("demarrage du scan")?;
        tokio::time::sleep(DUREE_SCAN).await;
        let trouvee = chercher(&adaptateur, &nom, retenue.as_deref()).await;
        let _ = adaptateur.stop_scan().await;
        board = trouvee?;
    }
    let board = board.ok_or_else(|| match &retenue {
        Some(a) => anyhow::anyhow!("board {nom} ({a}) introuvable"),
        None => anyhow::anyhow!("aucune board {nom} a portee"),
    })?;

    if !board.is_connected().await.unwrap_or(false) {
        board.connect().await.context("connexion")?;
    }
    board.discover_services().await.context("decouverte des services")?;
    let caracteristique = board
        .characteristics()
        .into_iter()
        .find(|c| c.uuid.as_u128() == LIGNES && c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE))
        .ok_or_else(|| anyhow::anyhow!("caracteristique du chiot absente"))?;

    // Voie montante : abonnement a ETAT. Absente (ancien micrologiciel) : les
    // trames partent quand meme, seul le lien reste « coupe ».
    let notifications = match board.characteristics().into_iter().find(|c| {
        c.uuid.as_u128() == ETAT && c.properties.contains(CharPropFlags::NOTIFY)
    }) {
        Some(etat) => {
            board.subscribe(&etat).await.context("abonnement a ETAT")?;
            Some(board.notifications().await.context("flux de notifications")?)
        }
        None => {
            eprintln!("ble: caracteristique ETAT absente, voie montante inactive");
            None
        }
    };

    let adresse = board.address().to_string();
    if retenue.is_none() {
        ecrire_adresse(&fichier, &adresse)?;
        eprintln!("ble: board {nom} appairee ({adresse}), retenue dans {}", fichier.display());
    }
    Ok(Liaison { board, caracteristique, notifications })
}

/// La board parmi les peripheriques connus de l adaptateur : par adresse si
/// une est retenue, sinon par nom annonce et service.
async fn chercher(
    adaptateur: &btleplug::platform::Adapter,
    nom: &str,
    retenue: Option<&str>,
) -> Result<Option<Peripheral>> {
    for p in adaptateur.peripherals().await.context("liste des peripheriques")? {
        let Ok(Some(props)) = p.properties().await else { continue };
        let ok = match retenue {
            Some(a) => props.address.to_string().eq_ignore_ascii_case(a),
            None => {
                props.local_name.as_deref() == Some(nom)
                    && (props.services.is_empty() || props.services.iter().any(|u| u.as_u128() == SERVICE))
            }
        };
        if ok {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// Scanne et rend les noms des peripheriques vus (option --scan-ble).
pub async fn scan(duree: Duration) -> Result<Vec<String>> {
    let manager = Manager::new().await.context("gestionnaire BLE indisponible")?;
    let adaptateurs = manager.adapters().await.context("liste des adaptateurs")?;
    let adaptateur = adaptateurs
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("aucun adaptateur BLE"))?;
    adaptateur
        .start_scan(ScanFilter::default())
        .await
        .context("demarrage du scan")?;
    tokio::time::sleep(duree).await;
    let mut noms = Vec::new();
    for p in adaptateur.peripherals().await? {
        if let Ok(Some(props)) = p.properties().await {
            noms.push(props.local_name.unwrap_or_else(|| props.address.to_string()));
        }
    }
    let _ = adaptateur.stop_scan().await;
    Ok(noms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::lien_board;
    use crate::model::{Keys, Value};
    use std::sync::{Arc, Mutex};

    fn trame(n: i64) -> Frame {
        let mut k = Keys::new();
        k.insert("n".into(), Value::Int(n));
        Frame(k)
    }

    #[test]
    fn morceaux_couvre_la_ligne() {
        let ligne: Vec<u8> = (0..300u32).map(|i| b'a' + (i % 26) as u8).collect();
        let m: Vec<&[u8]> = morceaux(&ligne, 20).collect();
        assert_eq!(m.len(), 15);
        assert!(m.iter().all(|x| x.len() <= 20));
        assert_eq!(m.concat(), ligne);
        assert_eq!(morceaux(&ligne, 244).count(), 2);
    }

    #[test]
    fn adresse_absente_puis_retenue() {
        let dir = std::env::temp_dir().join(format!("ble_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("osampo").join("ble_osampo");
        assert_eq!(lire_adresse(&f), None);
        ecrire_adresse(&f, "AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(lire_adresse(&f).as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        std::fs::write(&f, " \n").unwrap();
        assert_eq!(lire_adresse(&f), None, "un fichier vide vaut un nouvel appairage");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_seul_emetteur_par_board() {
        let dir = std::env::temp_dir().join(format!("ble_verrou_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("ble_osampo.lock");
        let premier = prendre_verrou(&f).expect("premier verrou");
        assert!(prendre_verrou(&f).is_err(), "un second emetteur doit etre refuse");
        drop(premier);
        assert!(prendre_verrou(&f).is_ok(), "verrou libere a la fin du premier");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn les_essais_s_espacent_quand_la_board_reste_absente() {
        let d: Vec<u64> = (0..6).map(|n| delai_apres_echecs(n).as_secs()).collect();
        assert_eq!(d, vec![5, 5, 10, 20, 30, 30]);
    }

    #[test]
    fn une_cause_n_est_signalee_qu_une_fois() {
        let mut deja = None;
        assert!(a_signaler(&mut deja, "aucun adaptateur BLE".into()));
        assert!(!a_signaler(&mut deja, "aucun adaptateur BLE".into()));
        assert!(a_signaler(&mut deja, "board introuvable".into()));
    }

    /// Canal factice : note les octets recus ; echoue a l envoi numero `panne`.
    struct Faux {
        recus: Arc<Mutex<Vec<u8>>>,
        envois: usize,
        panne: Option<usize>,
    }

    #[async_trait::async_trait]
    impl Canal for Faux {
        async fn envoyer(&mut self, octets: &[u8]) -> Result<()> {
            assert!(octets.len() <= TAILLE_MORCEAU, "morceau de {} octets", octets.len());
            self.envois += 1;
            if Some(self.envois) == self.panne {
                anyhow::bail!("deconnecte");
            }
            self.recus.lock().unwrap().extend_from_slice(octets);
            Ok(())
        }

        async fn notification(&mut self) -> Result<Vec<u8>> {
            std::future::pending().await
        }

        fn abonne(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn sans_adaptateur_on_reessaie() {
        let (tx, rx) = broadcast::channel::<Frame>(8);
        let (_cfg_tx, cfg) = watch::channel(None::<String>);
        let essais = Arc::new(Mutex::new(0));
        let e = essais.clone();
        let tache = tokio::spawn(servir_avec(
            move || {
                let e = e.clone();
                async move {
                    *e.lock().unwrap() += 1;
                    Err::<Faux, _>(anyhow::anyhow!("aucun adaptateur BLE"))
                }
            },
            rx,
            cfg,
            lien_board::creer().1,
            |_| Duration::from_millis(10),
        ));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!tache.is_finished(), "l emetteur ne doit jamais s arreter sur une erreur");
        assert!(*essais.lock().unwrap() >= 3);
        drop(tx);
        tache.abort();
    }

    #[tokio::test]
    async fn cfg_puis_trames_et_reprise_apres_deconnexion() {
        let (tx, rx) = broadcast::channel::<Frame>(8);
        let (_cfg_tx, cfg) = watch::channel(Some("{\"cfg\":1}".to_string()));
        let recus = Arc::new(Mutex::new(Vec::new()));
        let connexions = Arc::new(Mutex::new(0usize));
        let (r, c) = (recus.clone(), connexions.clone());
        let tache = tokio::spawn(servir_avec(
            move || {
                let (r, c) = (r.clone(), c.clone());
                async move {
                    let mut n = c.lock().unwrap();
                    *n += 1;
                    // premiere connexion : panne au 4e envoi (milieu de la 2e trame)
                    Ok(Faux { recus: r, envois: 0, panne: if *n == 1 { Some(4) } else { None } })
                }
            },
            rx,
            cfg,
            lien_board::creer().1,
            |_| Duration::from_millis(10),
        ));
        for n in 0..6 {
            tokio::time::sleep(Duration::from_millis(30)).await;
            let _ = tx.send(trame(n));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(tx);
        tokio::time::timeout(Duration::from_secs(2), tache).await.unwrap().unwrap();
        assert_eq!(*connexions.lock().unwrap(), 2);
        let texte = String::from_utf8(recus.lock().unwrap().clone()).unwrap();
        // chaque connexion commence par un saut de ligne puis la ligne cfg
        assert!(texte.starts_with("\n{\"cfg\":1}\n"), "{texte:?}");
        assert_eq!(texte.matches("{\"cfg\":1}\n").count(), 2, "{texte:?}");
        assert!(texte.ends_with("{\"n\":5}\n") || texte.contains("{\"n\":5}"), "{texte:?}");
    }
}
