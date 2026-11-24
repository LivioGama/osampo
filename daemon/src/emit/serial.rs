// Emetteur serie (spec SP5d1 D9) : les trames du daemon partent sur le port
// USB de la board, une ligne JSON par trame, comme sur le socket.
//
// Le port est ouvert non bloquant et servi par tokio::io::unix::AsyncFd : pas
// de fil dedie, et une ecriture qui attend ne bloque que cette tache. La boucle
// d emission n en sait rien, elle ne fait que diffuser sur le broadcast.

use crate::model::Frame;
use anyhow::{Context, Result};
use nix::fcntl::{open, OFlag};
use nix::sys::stat::Mode;
use nix::sys::termios::{cfmakeraw, tcflush, tcgetattr, tcsetattr, ControlFlags, FlushArg, SetArg};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::unix::AsyncFd;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{broadcast, watch};

/// Pause avant de rouvrir le port apres une erreur, une fin de flux ou un
/// echec d ouverture (board debranchee, reset en cours de re-enumeration).
pub const DELAI_REOUVERTURE: Duration = Duration::from_secs(5);

/// Une trame qui ne part pas en 10 s : la board ne lit plus (plantee, en
/// chargeur). On ferme et on rouvre, ce qui la relance.
const DELAI_ECRITURE: Duration = Duration::from_secs(10);

/// Au-dela, une ligne de journal de la board est coupee : le tampon ne croit
/// jamais sans borne sur un flux sans saut de ligne.
const LIGNE_MAX: usize = 4096;

/// Sert le port jusqu a la fermeture du broadcast. Ne s arrete jamais sur une
/// erreur du port : on journalise et on rouvre toutes les 5 s.
/// cfg : la ligne de reglages de la board (None : widget sans reglages). Elle
/// part apres chaque ouverture, avant toute trame, puis a chaque changement.
/// echo : chaque ligne recue de la board part sur stderr, prefixee "[board] ".
pub async fn servir(
    chemin: PathBuf,
    rx: broadcast::Receiver<Frame>,
    cfg: watch::Receiver<Option<String>>,
    echo: bool,
) {
    servir_avec(chemin, rx, cfg, echo, DELAI_REOUVERTURE).await
}

async fn servir_avec(
    chemin: PathBuf,
    mut rx: broadcast::Receiver<Frame>,
    mut cfg: watch::Receiver<Option<String>>,
    echo: bool,
    delai: Duration,
) {
    // Board debranchee : un seul message par cause, pas un toutes les 5 s.
    let mut deja_signale: Option<String> = None;
    loop {
        match ouvrir(&chemin) {
            Ok(port) => {
                eprintln!("serie: {} ouvert", chemin.display());
                deja_signale = None;
                let fin = tokio::select! {
                    r = ecrire(&port, &mut rx, &mut cfg) => r,
                    r = lire(&port, echo) => r,
                };
                fermer(port);
                match fin {
                    Ok(()) => return, // broadcast ferme : le daemon s arrete
                    Err(e) => eprintln!(
                        "serie: {} perdu ({e:#}), nouvel essai dans {delai:?}",
                        chemin.display()
                    ),
                }
            }
            Err(e) => {
                let cause = format!("{e:#}");
                if deja_signale.as_deref() != Some(cause.as_str()) {
                    eprintln!("serie: {cause}, nouvel essai toutes les {delai:?}");
                    deja_signale = Some(cause);
                }
            }
        }
        tokio::time::sleep(delai).await;
    }
}

/// Ouvre le port en mode brut : pas d echo, pas de traduction de fin de ligne,
/// pas de controle de flux, CLOCAL (pas d attente de porteuse), sans HUPCL
/// (la fermeture ne touche pas aux lignes), puis DTR et RTS a 0.
fn ouvrir(chemin: &Path) -> Result<AsyncFd<File>> {
    let fd = open(
        chemin,
        OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .with_context(|| format!("ouverture de {}", chemin.display()))?;
    let mut t = tcgetattr(&fd).with_context(|| format!("{} n est pas un terminal", chemin.display()))?;
    cfmakeraw(&mut t);
    t.control_flags |= ControlFlags::CLOCAL | ControlFlags::CREAD;
    t.control_flags &= !(ControlFlags::HUPCL | ControlFlags::CRTSCTS);
    tcsetattr(&fd, SetArg::TCSANOW, &t).with_context(|| format!("termios de {}", chemin.display()))?;
    // Les lignes modem n existent pas sur un pty : un refus ici n est pas une erreur.
    let lignes: libc::c_int = libc::TIOCM_DTR | libc::TIOCM_RTS;
    // SAFETY: fd ouvert et valide, TIOCMBIC lit un c_int a l adresse donnee.
    unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCMBIC, &lignes) };
    // SAFETY: le File possede le fd et le garde ouvert tant que l AsyncFd vit ;
    // as_raw_fd rend toujours ce meme fd.
    unsafe { AsyncFd::register(File::from(fd)) }
        .map_err(std::io::Error::from)
        .context("enregistrement du port aupres de tokio")
}

/// Ferme le port sans bloquer l executeur. close() sur un tty attend que la
/// sortie se vide (closing_wait, jusqu a ~30 s) : apres une ecriture bloquee,
/// ce serait un fil de tokio gele. On jette donc la sortie en attente, puis la
/// fermeture se fait dans le pool bloquant, sans l attendre.
fn fermer(port: AsyncFd<File>) {
    let _ = tcflush(port.get_ref(), FlushArg::TCOFLUSH);
    let f = port.into_inner();
    tokio::task::spawn_blocking(move || drop(f));
}

/// La trame la plus recente du broadcast. Une trame en retard ne sert a rien a
/// la board : apres une perte (Lagged) ou une reouverture, on saute a la
/// derniere. None : broadcast ferme.
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

/// Ecrit chaque trame sur le port. Ok(()) : broadcast ferme ; Err : port perdu.
/// Un saut de ligne part d abord : la board jette ainsi une ligne coupee par
/// une fermeture precedente au lieu de la coller a la premiere trame. Puis la
/// ligne cfg courante, avant toute trame (la board vient peut-etre de
/// redemarrer), et de nouveau a chaque changement ; un changement survenu port
/// ferme est donc pris a l ouverture suivante.
async fn ecrire(
    port: &AsyncFd<File>,
    rx: &mut broadcast::Receiver<Frame>,
    cfg: &mut watch::Receiver<Option<String>>,
) -> Result<()> {
    ecrire_sous_delai(port, b"\n").await?;
    ecrire_cfg(port, cfg).await?;
    // Emetteur de la ligne cfg abandonne (widget sans reglages) : plus rien a guetter.
    let mut cfg_vivante = true;
    loop {
        tokio::select! {
            f = la_plus_recente(rx) => match f {
                Some(f) => ecrire_sous_delai(port, f.to_line().as_bytes()).await?,
                None => return Ok(()),
            },
            r = cfg.changed(), if cfg_vivante => match r {
                Ok(()) => ecrire_cfg(port, cfg).await?,
                Err(_) => cfg_vivante = false,
            },
        }
    }
}

/// La ligne cfg courante, si le widget en a une ; marquee vue.
async fn ecrire_cfg(port: &AsyncFd<File>, cfg: &mut watch::Receiver<Option<String>>) -> Result<()> {
    let ligne = cfg.borrow_and_update().clone();
    if let Some(l) = ligne {
        ecrire_sous_delai(port, format!("{l}\n").as_bytes()).await?;
    }
    Ok(())
}

async fn ecrire_sous_delai(port: &AsyncFd<File>, octets: &[u8]) -> Result<()> {
    tokio::time::timeout(DELAI_ECRITURE, ecrire_tout(port, octets))
        .await
        .map_err(|_| anyhow::anyhow!("ecriture bloquee depuis {DELAI_ECRITURE:?}"))?
}

async fn ecrire_tout(port: &AsyncFd<File>, mut octets: &[u8]) -> Result<()> {
    while !octets.is_empty() {
        let mut garde = port.writable().await?;
        match garde.try_io(|fd| fd.get_ref().write(octets)) {
            Ok(Ok(0)) => anyhow::bail!("ecriture de 0 octet"),
            Ok(Ok(n)) => octets = &octets[n..],
            Ok(Err(e)) => return Err(e).context("ecriture"),
            Err(_bloquerait) => continue,
        }
    }
    Ok(())
}

/// Lit tout ce que la board envoie (ses journaux) : sans lecteur, elle
/// bloquerait sur sa console. Ne rend que sur erreur ou fin de flux, c est a
/// dire quand le port a disparu (debranchement, reset).
async fn lire(port: &AsyncFd<File>, echo: bool) -> Result<()> {
    let mut lignes = Lignes::default();
    let mut tampon = [0u8; 512];
    loop {
        let mut garde = port.readable().await?;
        match garde.try_io(|fd| fd.get_ref().read(&mut tampon)) {
            Ok(Ok(0)) => anyhow::bail!("fin de flux"),
            Ok(Ok(n)) => {
                if echo {
                    for l in lignes.pousser(&tampon[..n]) {
                        eprintln!("[board] {l}");
                    }
                }
            }
            Ok(Err(e)) => return Err(e).context("lecture"),
            Err(_bloquerait) => continue,
        }
    }
}

/// Decoupe le flux de la board en lignes, sans \r final, bornees a LIGNE_MAX.
#[derive(Default)]
struct Lignes {
    tampon: Vec<u8>,
}

impl Lignes {
    fn pousser(&mut self, octets: &[u8]) -> Vec<String> {
        let mut sortie = Vec::new();
        for &o in octets {
            if o == b'\n' || self.tampon.len() >= LIGNE_MAX {
                let l = String::from_utf8_lossy(&self.tampon);
                sortie.push(l.trim_end_matches('\r').to_string());
                self.tampon.clear();
                if o == b'\n' {
                    continue;
                }
            }
            self.tampon.push(o);
        }
        sortie
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Keys, Value};
    use nix::pty::openpty;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::path::Path;
    use std::time::Instant;

    /// Un widget sans reglages : la ligne cfg reste None, l emetteur de la
    /// valeur est abandonne comme dans main.
    fn sans_cfg() -> watch::Receiver<Option<String>> {
        watch::channel(None).1
    }

    fn trame(n: i64) -> Frame {
        let mut k = Keys::new();
        k.insert("n".into(), Value::Int(n));
        Frame(k)
    }

    /// Un pseudo-terminal ; le lien `lien` pointe vers son esclave.
    /// Rend le maitre (cote « board ») et l esclave garde ouvert par le test.
    fn pty_au_lien(lien: &Path) -> (File, OwnedFd) {
        let p = openpty(None, None).unwrap();
        let nom = nix::unistd::ttyname(&p.slave).unwrap();
        let _ = std::fs::remove_file(lien);
        std::os::unix::fs::symlink(&nom, lien).unwrap();
        (File::from(p.master), p.slave)
    }

    /// Une ligne lue sur le maitre avant l echeance, sans jamais pendre.
    fn lire_ligne(maitre: &mut File, echeance: Duration) -> Option<String> {
        let limite = Instant::now() + echeance;
        let mut ligne = Vec::new();
        loop {
            let reste = limite.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                return None;
            }
            let mut pfd = libc::pollfd { fd: maitre.as_raw_fd(), events: libc::POLLIN, revents: 0 };
            // SAFETY: un seul pollfd valide, duree bornee.
            let n = unsafe { libc::poll(&mut pfd, 1, reste.as_millis() as i32) };
            if n <= 0 {
                continue;
            }
            let mut o = [0u8; 1];
            match maitre.read(&mut o) {
                Ok(1) if o[0] == b'\n' => {
                    return Some(String::from_utf8_lossy(&ligne).trim_end_matches('\r').to_string())
                }
                Ok(1) => ligne.push(o[0]),
                _ => return None,
            }
        }
    }

    /// Emet des trames 1000, 1001... jusqu a ce qu une trame arrive sur le
    /// maitre : le port est alors ouvert. Verifie le saut de ligne de
    /// resynchronisation ecrit a l ouverture, puis rend la premiere trame.
    async fn attendre_l_ouverture(tx: &broadcast::Sender<Frame>, maitre: File) -> (File, String) {
        let fil = std::thread::spawn(move || {
            let mut m = maitre;
            let vide = lire_ligne(&mut m, Duration::from_secs(5));
            assert_eq!(vide.as_deref(), Some(""), "un saut de ligne doit preceder la premiere trame");
            let l = lire_ligne(&mut m, Duration::from_secs(5));
            (m, l)
        });
        let mut n = 1000;
        while !fil.is_finished() {
            let _ = tx.send(trame(n));
            n += 1;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let (m, l) = fil.join().unwrap();
        (m, l.expect("aucune ligne sur le port en 5 s"))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn les_trames_partent_sur_le_port_et_reprennent_apres_fermeture() {
        let dir = std::env::temp_dir().join(format!("serial_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lien = dir.join("port");

        let (maitre, esclave) = pty_au_lien(&lien);
        let (tx, rx) = broadcast::channel::<Frame>(16);
        let tache =
            tokio::spawn(servir_avec(lien.clone(), rx, sans_cfg(), false, Duration::from_millis(100)));

        let (mut maitre, premiere) = attendre_l_ouverture(&tx, maitre).await;
        assert!(premiere.starts_with("{\"n\":"), "premiere ligne: {premiere:?}");
        // vide les trames d ouverture encore en vol
        tokio::time::sleep(Duration::from_millis(100)).await;
        while lire_ligne(&mut maitre, Duration::from_millis(100)).is_some() {}

        // 3 trames emises = 3 lignes lues, dans l ordre, a l identique ; chaque
        // trame part une fois la precedente lue, sans dependre d un delai.
        for n in 1..=3 {
            tx.send(trame(n)).unwrap();
            let l = lire_ligne(&mut maitre, Duration::from_secs(2)).expect("ligne attendue");
            assert_eq!(format!("{l}\n"), trame(n).to_line());
        }
        assert_eq!(lire_ligne(&mut maitre, Duration::from_millis(200)), None, "aucune ligne en trop");

        // la board fait un reset : le port disparait, un autre apparait au meme chemin
        drop(maitre);
        drop(esclave);
        let (maitre, _esclave2) = pty_au_lien(&lien);
        let (mut maitre, reprise) = attendre_l_ouverture(&tx, maitre).await;
        assert!(reprise.starts_with("{\"n\":"), "ligne apres reprise: {reprise:?}");
        // et l emission continue
        tx.send(trame(7)).unwrap();
        let mut vu = false;
        for _ in 0..10 {
            if lire_ligne(&mut maitre, Duration::from_millis(500)).as_deref() == Some("{\"n\":7}") {
                vu = true;
                break;
            }
        }
        assert!(vu, "la trame 7 doit passer apres la reprise");

        // les journaux de la board sont lus et jetes sans gener l emission
        maitre.write_all(b"I (123) chiot: t=1 etat=calme\n").unwrap();
        tx.send(trame(8)).unwrap();
        let mut vu = false;
        for _ in 0..10 {
            if lire_ligne(&mut maitre, Duration::from_millis(500)).as_deref() == Some("{\"n\":8}") {
                vu = true;
                break;
            }
        }
        assert!(vu, "la trame 8 doit passer malgre les journaux de la board");

        tache.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// La premiere ligne apres l ouverture est la ligne cfg, avant toute trame,
    /// et l est encore apres une fermeture et une reouverture.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cfg_renvoyee_a_la_reouverture() {
        let dir = std::env::temp_dir().join(format!("serial_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lien = dir.join("port");

        let (maitre, esclave) = pty_au_lien(&lien);
        let (tx, rx) = broadcast::channel::<Frame>(16);
        let (cfg_tx, cfg_rx) = watch::channel(Some("{\"cfg\":1,\"lum_nuit\":20}".to_string()));
        let tache =
            tokio::spawn(servir_avec(lien.clone(), rx, cfg_rx, false, Duration::from_millis(100)));

        // attendre_l_ouverture emet des trames pendant l attente : la ligne
        // cfg doit passer devant elles.
        let (mut maitre, premiere) = attendre_l_ouverture(&tx, maitre).await;
        assert_eq!(premiere, "{\"cfg\":1,\"lum_nuit\":20}");
        tokio::time::sleep(Duration::from_millis(100)).await;
        while lire_ligne(&mut maitre, Duration::from_millis(100)).is_some() {}

        // une relecture acceptee : la nouvelle ligne part aussitot
        cfg_tx.send_replace(Some("{\"cfg\":1,\"lum_nuit\":30}".to_string()));
        let l = lire_ligne(&mut maitre, Duration::from_secs(2));
        assert_eq!(l.as_deref(), Some("{\"cfg\":1,\"lum_nuit\":30}"));

        // la board fait un reset ; un changement pendant que le port est ferme
        // est pris a la reouverture
        drop(maitre);
        drop(esclave);
        tokio::time::sleep(Duration::from_millis(50)).await;
        cfg_tx.send_replace(Some("{\"cfg\":1,\"lum_nuit\":40}".to_string()));
        let (maitre, _esclave2) = pty_au_lien(&lien);
        let (_maitre, reprise) = attendre_l_ouverture(&tx, maitre).await;
        assert_eq!(reprise, "{\"cfg\":1,\"lum_nuit\":40}");

        tache.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn apres_un_retard_seule_la_plus_recente_est_ecrite() {
        let dir = std::env::temp_dir().join(format!("serial_lagged_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lien = dir.join("port");
        let (mut maitre, _esclave) = pty_au_lien(&lien);

        // capacite 2, 5 trames avant tout : le recepteur est en retard (Lagged)
        let (tx, rx) = broadcast::channel::<Frame>(2);
        for n in 1..=5 {
            tx.send(trame(n)).unwrap();
        }
        let tache =
            tokio::spawn(servir_avec(lien.clone(), rx, sans_cfg(), false, Duration::from_millis(100)));

        let lues = tokio::task::spawn_blocking(move || {
            let mut v = Vec::new();
            while let Some(l) = lire_ligne(&mut maitre, Duration::from_millis(500)) {
                v.push(l);
            }
            v
        })
        .await
        .unwrap();
        assert_eq!(lues, vec![String::new(), "{\"n\":5}".to_string()]);

        tache.abort();
        drop(tx);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn les_lignes_de_la_board_sont_decoupees() {
        let mut l = Lignes::default();
        assert_eq!(l.pousser(b"a\r\nb"), vec!["a".to_string()]);
        assert_eq!(l.pousser(b"c\n\n"), vec!["bc".to_string(), String::new()]);
        assert!(l.pousser(b"suite").is_empty());
    }
}
