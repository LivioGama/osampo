use crate::manifest::Manifest;
use crate::model::{Frame, Keys};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::VecDeque;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc, oneshot, watch, Notify};

/// Taille de la file de chaque connexion. Pleine, elle jette sa plus ancienne
/// trame : un client lent ne gele ni le planificateur ni les autres clients.
pub const FILE_PAR_CONNEXION: usize = 16;

/// Au-dela de ce nombre de trames jetees d affilee, un client est considere
/// comme incapable de suivre et sa connexion est fermee. Le compteur revient a
/// zero a chaque ecriture reussie : quelques pertes par heure ne coupent rien.
const TRAMES_JETEES_MAX: u64 = 64;

/// Longueur maximale d une ligne, protocole. Au-dela, la connexion est fermee
/// plutot que de laisser un tampon croitre sans borne sur un flux corrompu.
const LIGNE_MAX: usize = 4096;

/// Une ecriture bloquee plus longtemps que ce delai ferme la connexion, meme si
/// plus aucune trame n arrive pour faire monter le compteur de pertes.
const DELAI_ECRITURE: Duration = Duration::from_secs(5);

/// Delai accorde au client pour envoyer son hello. Le widget l ecrit aussitot
/// apres connect : 3 s couvrent une machine chargee, et liberent vite le fd
/// d un client qui ne dira jamais rien.
const DELAI_HELLO: Duration = Duration::from_secs(3);

/// Tampon d emission demande au noyau pour chaque connexion. Le defaut, plus
/// de 200 Kio, absorberait des milliers de trames et rendrait la file de 16
/// fictive : un client qui ne lit plus ne serait detecte qu apres des minutes.
const TAMPON_NOYAU: i32 = 4096;

/// Nom de hello des programmes du PC (SPEC-002) : pas de manifeste de widget.
const HELLO_PC: &str = "osampo-pc";

/// Cadence minimale de la ligne d etat envoyee aux programmes du PC (SPEC-002).
const REPETITION_ETAT: Duration = Duration::from_secs(10);

/// Une commande d un programme du PC et le canal de sa reponse (une ligne).
pub type CommandePc = (String, oneshot::Sender<String>);

/// Ce que la tache « pc » du daemon offre aux clients `osampo-pc` : la
/// ligne d etat courante (watch) et le passage des commandes.
#[derive(Clone)]
pub struct LienPc {
    pub etat: watch::Receiver<String>,
    pub commandes: mpsc::Sender<CommandePc>,
}

#[derive(Debug, Deserialize)]
struct Hello {
    hello: String,
}

/// Le chemin du socket. $XDG_RUNTIME_DIR quand il existe, sinon un repertoire
/// par utilisateur sous /tmp : un /tmp nu serait squattable par un autre compte.
pub fn socket_path() -> PathBuf {
    if let Ok(d) = std::env::var("XDG_RUNTIME_DIR") {
        if !d.is_empty() {
            return PathBuf::from(d).join("osampo.sock");
        }
    }
    repertoire_de_repli().join("osampo.sock")
}

/// Le seul repertoire que le daemon cree et protege lui-meme.
fn repertoire_de_repli() -> PathBuf {
    let uid = unsafe { libc_getuid() };
    PathBuf::from(format!("/tmp/osampo-{uid}"))
}

// Evite une dependance a la crate libc pour deux appels.
extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
    fn setsockopt(fd: i32, level: i32, name: i32, val: *const std::ffi::c_void, len: u32) -> i32;
}
const SOL_SOCKET: i32 = 1;
const SO_SNDBUF: i32 = 7;

/// Prepare le fichier socket : un daemon qui a plante laisse le fichier en
/// place, et bind echouerait en EADDRINUSE. On ne supprime qu un socket que
/// plus personne n ecoute (ECONNREFUSED), jamais un autre type de fichier.
fn preparer_le_fichier(path: &Path) -> Result<()> {
    // Seul le repli sous /tmp est cree et passe en 0700 : un repertoire donne
    // par --socket (le $HOME, /tmp) appartient a l utilisateur, pas au daemon.
    let repli = repertoire_de_repli();
    if path.parent() == Some(repli.as_path()) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(&repli)
            .with_context(|| format!("repertoire du socket: {}", repli.display()))?;
        std::fs::set_permissions(&repli, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("chmod 0700 sur {}", repli.display()))?;
    }

    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("socket: {}", path.display())),
    };
    use std::os::unix::fs::FileTypeExt;
    if !meta.file_type().is_socket() {
        anyhow::bail!("{} existe et n est pas un socket, refus de l effacer", path.display());
    }
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => anyhow::bail!("un daemon ecoute deja sur {}", path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            std::fs::remove_file(path)
                .with_context(|| format!("socket residuel: {}", path.display()))?;
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("socket existant inutilisable: {}", path.display())),
    }
}

/// Prepare le fichier et se lie au socket. Separe de serve pour que l appelant
/// sache, avant tout signal, que le fichier est bien le sien : il ne doit
/// jamais supprimer le socket d un autre daemon.
pub fn bind(path: &Path) -> Result<UnixListener> {
    preparer_le_fichier(path)?;
    let listener =
        UnixListener::bind(path).with_context(|| format!("bind sur {}", path.display()))?;
    eprintln!("socket a l ecoute: {}", path.display());
    Ok(listener)
}

/// Pause apres un accept en echec. EMFILE se repete tant qu aucun fd n est
/// libere : sans pause, la boucle tournerait a vide.
const PAUSE_APRES_ACCEPT: Duration = Duration::from_millis(100);

/// Accepte les connexions et sert chacune dans sa propre tache. Ne s arrete
/// jamais : un accept en echec (EMFILE, ECONNABORTED) est journalise, puis on
/// reessaie, plutot que de couper tous les widgets.
/// cfg : la ligne de reglages du widget osampo (None : aucun reglage).
pub async fn serve(
    listener: UnixListener,
    widgets_dir: PathBuf,
    rx: broadcast::Receiver<Frame>,
    cfg: watch::Receiver<Option<String>>,
    pc: Option<LienPc>,
) {
    loop {
        let flux = match listener.accept().await {
            Ok((f, _)) => f,
            Err(e) => {
                eprintln!("accept en echec, nouvel essai: {e}");
                tokio::time::sleep(PAUSE_APRES_ACCEPT).await;
                continue;
            }
        };
        reduire_le_tampon_noyau(&flux);
        let rx = rx.resubscribe();
        let dir = widgets_dir.clone();
        let cfg = cfg.clone();
        let pc = pc.clone();
        tokio::spawn(async move {
            if let Err(e) = servir_un_client(flux, dir, rx, cfg, pc).await {
                eprintln!("connexion terminee: {e}");
            }
        });
    }
}

fn reduire_le_tampon_noyau(flux: &UnixStream) {
    let rc = unsafe {
        setsockopt(
            flux.as_raw_fd(),
            SOL_SOCKET,
            SO_SNDBUF,
            &TAMPON_NOYAU as *const i32 as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        )
    };
    if rc != 0 {
        eprintln!("SO_SNDBUF refuse: {}", std::io::Error::last_os_error());
    }
}

/// Une reponse de refus, puis la connexion se ferme.
async fn refuser(ecriture: &mut OwnedWriteHalf, motif: String) -> Result<()> {
    let refus = serde_json::json!({ "error": motif });
    let _ = ecriture.write_all(format!("{refus}\n").as_bytes()).await;
    anyhow::bail!("hello refuse: {motif}")
}

async fn servir_un_client(
    flux: UnixStream,
    widgets_dir: PathBuf,
    rx: broadcast::Receiver<Frame>,
    mut cfg: watch::Receiver<Option<String>>,
    pc: Option<LienPc>,
) -> Result<()> {
    let (lecture, mut ecriture) = flux.into_split();

    // Lecture bornee : au plus LIGNE_MAX + 1 octets. Une ligne qui n a pas
    // fini a la limite est refusee, le tampon ne croit jamais au-dela.
    // La moitie lecture est gardee apres le hello : c est elle qui voit le
    // depart du client quand aucune trame n est ecrite.
    let mut lecteur = BufReader::new(lecture);
    let mut premiere = String::new();
    let lus = tokio::time::timeout(
        DELAI_HELLO,
        (&mut lecteur).take(LIGNE_MAX as u64 + 1).read_line(&mut premiere),
    )
    .await;
    let lus = match lus {
        Ok(r) => r,
        Err(_) => {
            return refuser(&mut ecriture, format!("aucun hello en {DELAI_HELLO:?}")).await;
        }
    };
    match lus {
        Ok(0) => anyhow::bail!("le client a ferme avant d envoyer son hello"),
        Ok(_) if !premiere.ends_with('\n') => {
            return refuser(
                &mut ecriture,
                format!("hello sans saut de ligne dans les {LIGNE_MAX} premiers octets"),
            )
            .await;
        }
        Ok(_) => {}
        Err(e) => return refuser(&mut ecriture, format!("hello illisible: {e}")).await,
    }

    let hello: Hello = match serde_json::from_str(premiere.trim_end()) {
        Ok(h) => h,
        Err(e) => return refuser(&mut ecriture, format!("hello illisible: {e}")).await,
    };

    // Programmes du PC : ni manifeste, ni ack, ni trames de widget (SPEC-002).
    if hello.hello == HELLO_PC {
        return match pc {
            Some(pc) => servir_pc(lecteur, ecriture, pc).await,
            None => refuser(&mut ecriture, "le daemon ne sert pas les programmes du PC".into()).await,
        };
    }

    let manifeste = match Manifest::load(&widgets_dir, &hello.hello) {
        Ok(m) => m,
        Err(e) => return refuser(&mut ecriture, format!("{e:#}")).await,
    };

    let cles = manifeste.key_names();
    let ack = serde_json::json!({
        "ack": manifeste.id,
        "cadence_ms": manifeste.refresh_ms,
        "stale_s": manifeste.stale_s,
        "keys": cles,
    });
    ecriture.write_all(format!("{ack}\n").as_bytes()).await?;

    // Quatre moities dans la meme tache : l une remplit la file depuis le
    // broadcast, une autre la vide sur le socket, une autre guette le depart
    // du client, la derniere y range la ligne cfg a chaque relecture. La
    // premiere qui s arrete emporte les autres, et la connexion se ferme avec elles.
    let file = File::default();
    // C14, C15 : la ligne cfg ne va qu au client osampo, juste apres l ack.
    // Rangee ici, avant le select : une trame deja en attente dans rx ne peut
    // pas passer devant.
    let suivre_cfg = hello.hello == "osampo";
    if suivre_cfg {
        let ligne = cfg.borrow_and_update().clone();
        if let Some(l) = ligne {
            file.ranger(format!("{l}\n"))?;
        }
    }
    tokio::select! {
        r = alimenter(rx, &cles, &file) => r,
        r = vider(ecriture, &file) => r,
        r = guetter_le_depart(lecteur) => r,
        r = alimenter_cfg(cfg, &file), if suivre_cfg => r,
    }
}

/// Range chaque nouvelle ligne cfg dans la file ; la ligne courante y est deja,
/// rangee apres l ack. Ne rend jamais d elle-meme : un emetteur abandonne
/// laisse les autres moities servir.
async fn alimenter_cfg(mut cfg: watch::Receiver<Option<String>>, file: &File) -> Result<()> {
    loop {
        if cfg.changed().await.is_err() {
            return std::future::pending().await;
        }
        let ligne = cfg.borrow_and_update().clone();
        if let Some(l) = ligne {
            file.ranger(format!("{l}\n"))?;
        }
    }
}

/// Un client `osampo-pc` : la ligne d etat aussitot, a chaque changement et au
/// moins toutes les 10 s ; ses commandes passent a la tache « pc », la reponse
/// revient sur la meme file. Ligne illisible : reponse `commande inconnue`,
/// connexion gardee. Comme pour un widget, une file pleine perd sa plus ancienne
/// ligne et un client qui ne lit plus est deconnecte.
async fn servir_pc(
    lecteur: BufReader<OwnedReadHalf>,
    ecriture: OwnedWriteHalf,
    pc: LienPc,
) -> Result<()> {
    let file = File::default();
    tokio::select! {
        r = publier_etat(pc.etat, &file) => r,
        r = vider(ecriture, &file) => r,
        r = lire_commandes(lecteur, pc.commandes, &file) => r,
    }
}

/// Range l etat courant, puis chaque changement, puis une repetition toutes les 10 s.
async fn publier_etat(mut etat: watch::Receiver<String>, file: &File) -> Result<()> {
    let ligne = etat.borrow_and_update().clone();
    file.ranger(format!("{ligne}\n"))?;
    let mut repetition = tokio::time::interval_at(
        tokio::time::Instant::now() + REPETITION_ETAT,
        REPETITION_ETAT,
    );
    repetition.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            r = etat.changed() => {
                if r.is_err() {
                    return Ok(()); // la tache « pc » a disparu : la connexion se ferme
                }
                let ligne = etat.borrow_and_update().clone();
                file.ranger(format!("{ligne}\n"))?;
                repetition.reset();
            }
            _ = repetition.tick() => {
                let ligne = etat.borrow().clone();
                file.ranger(format!("{ligne}\n"))?;
            }
        }
    }
}

/// Lit les commandes ligne par ligne (jamais dans un select : read_line n y est pas
/// annulable) et range chaque reponse. Rend Ok a la fermeture du client.
async fn lire_commandes(
    mut lecteur: BufReader<OwnedReadHalf>,
    commandes: mpsc::Sender<CommandePc>,
    file: &File,
) -> Result<()> {
    loop {
        let mut ligne = String::new();
        let lu = (&mut lecteur).take(LIGNE_MAX as u64 + 1).read_line(&mut ligne).await;
        let texte = match lu {
            Ok(0) => return Ok(()),
            Ok(_) if !ligne.ends_with('\n') => {
                if ligne.len() > LIGNE_MAX {
                    anyhow::bail!("commande de plus de {LIGNE_MAX} octets");
                }
                return Ok(()); // fin de flux au milieu d une ligne
            }
            Ok(_) => ligne.trim_end().to_string(),
            // pas de l UTF-8 : une ligne illisible, la connexion reste
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => String::new(),
            Err(e) => return Err(e.into()),
        };
        let (rep_tx, rep_rx) = oneshot::channel();
        commandes
            .send((texte, rep_tx))
            .await
            .map_err(|_| anyhow::anyhow!("la tache pc a disparu"))?;
        let reponse = rep_rx.await.map_err(|_| anyhow::anyhow!("la tache pc a disparu"))?;
        file.ranger(format!("{reponse}\n"))?;
    }
}

/// Lit et jette tout ce que le client envoie apres son hello, jusqu a sa
/// fermeture. Sans elle, un client parti pendant un demarrage a froid garde
/// sa tache et son fd jusqu a la prochaine ecriture, peut-etre jamais.
async fn guetter_le_depart(mut lecteur: BufReader<OwnedReadHalf>) -> Result<()> {
    let mut poubelle = [0u8; 512];
    loop {
        if lecteur.read(&mut poubelle).await? == 0 {
            return Ok(());
        }
    }
}

/// La file propre a une connexion, partagee entre ses deux moities.
#[derive(Default)]
struct File {
    etat: Mutex<EtatFile>,
    reveil: Notify,
}

impl File {
    /// Range une ligne ; une file pleine perd sa plus ancienne.
    fn ranger(&self, ligne: String) -> Result<()> {
        if ligne.len() > LIGNE_MAX {
            anyhow::bail!("ligne de {} octets, au-dela de la limite", ligne.len());
        }
        {
            let mut etat = self.etat.lock().unwrap();
            if etat.lignes.len() >= FILE_PAR_CONNEXION {
                etat.lignes.pop_front();
                etat.perdre(1)?;
            }
            etat.lignes.push_back(ligne);
        }
        self.reveil.notify_one();
        Ok(())
    }
}

#[derive(Default)]
struct EtatFile {
    lignes: VecDeque<String>,
    jetees_d_affilee: u64,
}

impl EtatFile {
    /// Une ecriture a reussi : le client suit, les pertes passees sont soldees.
    fn ecrite(&mut self) {
        self.jetees_d_affilee = 0;
    }

    /// Compte des trames perdues ; erreur quand le client est juge decroche.
    fn perdre(&mut self, n: u64) -> Result<()> {
        self.jetees_d_affilee += n;
        if self.jetees_d_affilee > TRAMES_JETEES_MAX {
            anyhow::bail!("client trop lent, {} trames jetees d affilee", self.jetees_d_affilee);
        }
        Ok(())
    }
}

/// Filtre chaque trame selon le manifeste de la connexion et la range dans la
/// file. Ne bloque jamais sur le client : une file pleine perd son plus ancien.
async fn alimenter(
    mut rx: broadcast::Receiver<Frame>,
    cles: &[String],
    file: &File,
) -> Result<()> {
    loop {
        match rx.recv().await {
            Ok(frame) => {
                // filtrage par connexion : chaque client ne voit que les cles
                // declarees par SON manifeste.
                let filtree: Keys = frame
                    .0
                    .into_iter()
                    .filter(|(nom, _)| cles.iter().any(|c| c == nom))
                    .collect();
                file.ranger(Frame(filtree).to_line())?;
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                // Ne devrait pas arriver : cette moitie ne fait que copier.
                // Si l executeur l a privee de temps, ce sont des pertes aussi.
                file.etat.lock().unwrap().perdre(n)?;
            }
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
        }
    }
}

/// Ecrit les lignes de la file sur le socket, chacune sous delai.
async fn vider(mut ecriture: OwnedWriteHalf, file: &File) -> Result<()> {
    loop {
        let suivante = file.etat.lock().unwrap().lignes.pop_front();
        let Some(ligne) = suivante else {
            // notify_one garde un jeton si personne n attend : une trame
            // rangee entre le pop et cette attente n est pas manquee.
            file.reveil.notified().await;
            continue;
        };
        match tokio::time::timeout(DELAI_ECRITURE, ecriture.write_all(ligne.as_bytes())).await {
            Err(_) => anyhow::bail!("ecriture bloquee depuis {DELAI_ECRITURE:?}"),
            Ok(Err(e)) => return Err(e.into()),
            Ok(Ok(())) => file.etat.lock().unwrap().ecrite(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_pertes_comptent_d_affilee() {
        let mut e = EtatFile::default();
        for _ in 0..TRAMES_JETEES_MAX {
            e.perdre(1).unwrap();
        }
        // une ecriture reussie remet le compteur a zero
        e.ecrite();
        for _ in 0..TRAMES_JETEES_MAX {
            e.perdre(1).unwrap();
        }
        assert!(e.perdre(1).is_err(), "la 65e perte d affilee ferme la connexion");
    }

    /// Un repertoire de widgets avec demo3 et osampo, pour servir_un_client en direct.
    fn widgets(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sp3_socket_unit_{nom}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for id in ["demo3", "osampo"] {
            std::fs::create_dir_all(d.join(id)).unwrap();
            std::fs::write(
                d.join(id).join("widget.toml"),
                format!("id = \"{id}\"\nformats = [\"bar\"]\n[[keys]]\nname = \"value\"\ntype = \"float\"\n"),
            )
            .unwrap();
        }
        d
    }

    fn sans_cfg() -> watch::Receiver<Option<String>> {
        watch::channel(None).1
    }

    /// Une ligne du client, sous delai.
    async fn ligne_sous_delai(lecteur: &mut BufReader<&mut UnixStream>, delai: Duration) -> Option<String> {
        let mut l = String::new();
        match tokio::time::timeout(delai, lecteur.read_line(&mut l)).await {
            Ok(Ok(n)) if n > 0 => Some(l),
            _ => None,
        }
    }

    #[tokio::test]
    async fn cfg_envoyee_apres_ack_au_client_osampo() {
        let (tx, rx) = broadcast::channel::<Frame>(16);
        let (cfg_tx, cfg_rx) = watch::channel(Some("{\"cfg\":1,\"lum_nuit\":20}".to_string()));
        // une relecture avant la connexion : le client recoit la ligne courante, une fois
        cfg_tx.send_replace(Some("{\"cfg\":1,\"lum_nuit\":25}".to_string()));
        let (serveur, mut client) = UnixStream::pair().unwrap();
        let tache = tokio::spawn(servir_un_client(serveur, widgets("cfg"), rx, cfg_rx, None));

        client.write_all(b"{\"hello\":\"osampo\"}\n").await.unwrap();
        let mut lecteur = BufReader::new(&mut client);
        let d = Duration::from_secs(2);
        let ack = ligne_sous_delai(&mut lecteur, d).await.expect("ack");
        assert!(ack.contains("\"ack\""), "ack attendu: {ack:?}");
        let l = ligne_sous_delai(&mut lecteur, d).await;
        assert_eq!(l.as_deref(), Some("{\"cfg\":1,\"lum_nuit\":25}\n"), "cfg juste apres l ack");

        // puis les trames, et une nouvelle ligne cfg a chaque relecture acceptee
        let mut k = Keys::new();
        k.insert("value".into(), crate::model::Value::Float(1.0));
        tx.send(Frame(k)).unwrap();
        let l = ligne_sous_delai(&mut lecteur, d).await.expect("trame");
        assert_eq!(l, "{\"value\":1.0}\n");
        cfg_tx.send_replace(Some("{\"cfg\":1,\"lum_nuit\":30}".to_string()));
        let l = ligne_sous_delai(&mut lecteur, d).await;
        assert_eq!(l.as_deref(), Some("{\"cfg\":1,\"lum_nuit\":30}\n"));
        assert_eq!(ligne_sous_delai(&mut lecteur, Duration::from_millis(200)).await, None, "pas de doublon");

        tache.abort();
    }

    #[tokio::test]
    async fn cfg_passe_devant_une_trame_deja_en_attente() {
        // select! tire ses branches au hasard : plusieurs essais pour que
        // l ordre ne doive rien a la chance.
        let dir = widgets("ordre");
        for essai in 0..20 {
            let (tx, rx) = broadcast::channel::<Frame>(16);
            let (_cfg_tx, cfg_rx) = watch::channel(Some("{\"cfg\":1}".to_string()));
            let (serveur, mut client) = UnixStream::pair().unwrap();
            // une trame en attente dans le recepteur avant meme le hello
            let mut k = Keys::new();
            k.insert("value".into(), crate::model::Value::Float(3.0));
            tx.send(Frame(k)).unwrap();
            let tache = tokio::spawn(servir_un_client(serveur, dir.clone(), rx, cfg_rx, None));

            client.write_all(b"{\"hello\":\"osampo\"}\n").await.unwrap();
            let mut lecteur = BufReader::new(&mut client);
            let d = Duration::from_secs(2);
            let ack = ligne_sous_delai(&mut lecteur, d).await.expect("ack");
            assert!(ack.contains("\"ack\""), "ack attendu: {ack:?}");
            let l = ligne_sous_delai(&mut lecteur, d).await;
            assert_eq!(l.as_deref(), Some("{\"cfg\":1}\n"), "essai {essai} : cfg d abord");
            let l = ligne_sous_delai(&mut lecteur, d).await;
            assert_eq!(l.as_deref(), Some("{\"value\":3.0}\n"), "essai {essai} : puis la trame");
            tache.abort();
        }
    }

    #[tokio::test]
    async fn pas_de_cfg_pour_un_autre_widget() {
        let (tx, rx) = broadcast::channel::<Frame>(16);
        let (cfg_tx, cfg_rx) = watch::channel(Some("{\"cfg\":1,\"lum_nuit\":20}".to_string()));
        let (serveur, mut client) = UnixStream::pair().unwrap();
        let tache = tokio::spawn(servir_un_client(serveur, widgets("autre"), rx, cfg_rx, None));

        client.write_all(b"{\"hello\":\"demo3\"}\n").await.unwrap();
        let mut lecteur = BufReader::new(&mut client);
        let d = Duration::from_secs(2);
        let ack = ligne_sous_delai(&mut lecteur, d).await.expect("ack");
        assert!(ack.contains("\"ack\""), "ack attendu: {ack:?}");
        cfg_tx.send_replace(Some("{\"cfg\":1,\"lum_nuit\":30}".to_string()));
        let mut k = Keys::new();
        k.insert("value".into(), crate::model::Value::Float(2.0));
        tx.send(Frame(k)).unwrap();
        let l = ligne_sous_delai(&mut lecteur, d).await;
        assert_eq!(l.as_deref(), Some("{\"value\":2.0}\n"), "la trame, jamais la ligne cfg");
        assert_eq!(ligne_sous_delai(&mut lecteur, Duration::from_millis(200)).await, None);

        tache.abort();
    }

    #[tokio::test]
    async fn un_client_parti_libere_sa_connexion_meme_sans_trame() {
        // Demarrage a froid : le broadcast reste ouvert mais muet. Rien ne
        // sera ecrit, donc seule la lecture peut voir le depart du client.
        let (tx, rx) = broadcast::channel::<Frame>(16);
        let (serveur, mut client) = UnixStream::pair().unwrap();
        let tache = tokio::spawn(servir_un_client(serveur, widgets("parti"), rx, sans_cfg(), None));

        client.write_all(b"{\"hello\":\"demo3\"}\n").await.unwrap();
        let mut ack = String::new();
        let mut lecteur = BufReader::new(&mut client);
        tokio::time::timeout(Duration::from_secs(2), lecteur.read_line(&mut ack))
            .await
            .expect("ack sous delai")
            .unwrap();
        assert!(ack.contains("\"ack\""), "ack attendu: {ack:?}");
        drop(client);

        let fin = tokio::time::timeout(Duration::from_secs(2), tache)
            .await
            .expect("la connexion d un client parti doit se terminer");
        assert!(fin.unwrap().is_ok(), "un depart du client est une fin normale");
        drop(tx);
    }

    #[tokio::test]
    async fn un_client_muet_est_ferme_apres_le_delai_du_hello() {
        let (_tx, rx) = broadcast::channel::<Frame>(16);
        let (serveur, client) = UnixStream::pair().unwrap();
        let tache = tokio::spawn(servir_un_client(serveur, widgets("muet"), rx, sans_cfg(), None));

        let fin = tokio::time::timeout(DELAI_HELLO + Duration::from_secs(1), tache)
            .await
            .expect("un client muet doit etre ferme apres le delai du hello");
        assert!(fin.unwrap().is_err(), "un hello absent est un refus");

        let mut rep = String::new();
        let mut lecteur = BufReader::new(client);
        tokio::time::timeout(Duration::from_secs(1), lecteur.read_line(&mut rep))
            .await
            .expect("refus sous delai")
            .unwrap();
        assert!(rep.contains("\"error\""), "un refus porte un message: {rep:?}");
    }
}
