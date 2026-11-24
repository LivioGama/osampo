//! Connexion au socket du daemon (SPEC-002) : poignée de main, lecture des lignes d'état,
//! envoi des commandes. Daemon injoignable : nouvel essai toutes les 5 s.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};

use crate::statuts::Etat;

pub const DELAI_ESSAI: Duration = Duration::from_secs(5);
const DELAI_PREMIERS_ESSAIS: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum Evt {
    Etat(Box<Etat>),
    Injoignable,
}

/// Réponse du daemon : `Ok(())` ou le texte de `erreur`.
pub type Reponse = Result<(), String>;
/// Une ligne de commande JSON et le canal de sa réponse.
pub type Commande = (String, oneshot::Sender<Reponse>);

extern "C" {
    fn getuid() -> u32;
}

/// Le socket par défaut du daemon (même règle que `daemon/src/emit/socket.rs`).
pub fn socket_par_defaut() -> PathBuf {
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(d) if !d.is_empty() => PathBuf::from(d).join("osampo.sock"),
        _ => PathBuf::from(format!("/tmp/osampo-{}", unsafe { getuid() }))
            .join("osampo.sock"),
    }
}

/// `{"cmd":"chatouille"}`, `{"cmd":"accepter","rappel":"sortie,1230"}`.
pub fn ligne_commande(cmd: &str, rappel: Option<&str>) -> String {
    match rappel {
        Some(r) => serde_json::json!({"cmd": cmd, "rappel": r}).to_string(),
        None => serde_json::json!({"cmd": cmd}).to_string(),
    }
}

/// Commande `jauge` de SPEC-009, par exemple avec la valeur 100.
pub fn ligne_jauge(valeur: u8) -> String {
    serde_json::json!({"cmd": "jauge", "valeur": valeur}).to_string()
}

/// `Some(réponse)` si la ligne est une `reponse` du daemon.
pub fn lire_reponse(ligne: &str) -> Option<Reponse> {
    let v: serde_json::Value = serde_json::from_str(ligne).ok()?;
    v.get("reponse")?;
    if v.get("ok").and_then(|o| o.as_bool()) == Some(true) {
        Some(Ok(()))
    } else {
        Some(Err(v.get("erreur").and_then(|e| e.as_str()).unwrap_or("erreur inconnue").to_string()))
    }
}

pub async fn boucle(
    socket: PathBuf,
    evt: mpsc::UnboundedSender<Evt>,
    mut cmds: mpsc::UnboundedReceiver<Commande>,
) {
    // Au lancement avec la session, le daemon peut être à quelques instants de son `listen` :
    // les premiers essais sont rapprochés, ensuite toutes les 5 s.
    let mut rapides = 8;
    loop {
        match UnixStream::connect(&socket).await {
            Ok(flux) => {
                if session(flux, &evt, &mut cmds).await.is_none() {
                    return; // le programme s'arrête
                }
            }
            Err(e) => eprintln!("daemon injoignable ({}) : {e}", socket.display()),
        }
        if evt.send(Evt::Injoignable).is_err() {
            return;
        }
        let delai = if rapides > 0 {
            rapides -= 1;
            DELAI_PREMIERS_ESSAIS
        } else {
            DELAI_ESSAI
        };
        // Attente avant le nouvel essai ; une commande reçue entre-temps échoue aussitôt.
        let fin = tokio::time::sleep(delai);
        tokio::pin!(fin);
        loop {
            tokio::select! {
                _ = &mut fin => break,
                c = cmds.recv() => match c {
                    Some((_, rep)) => { let _ = rep.send(Err("daemon injoignable".into())); }
                    None => return,
                },
            }
        }
    }
}

/// Une connexion, jusqu'à sa perte (`Some`) ou la fin du programme (`None`).
async fn session(
    flux: UnixStream,
    evt: &mpsc::UnboundedSender<Evt>,
    cmds: &mut mpsc::UnboundedReceiver<Commande>,
) -> Option<()> {
    let (lecture, mut ecriture) = flux.into_split();
    let mut lignes = BufReader::new(lecture).lines();
    if ecriture.write_all(b"{\"hello\":\"osampo-pc\"}\n").await.is_err() {
        return Some(());
    }
    let mut attente: VecDeque<oneshot::Sender<Reponse>> = VecDeque::new();
    let fin = loop {
        tokio::select! {
            l = lignes.next_line() => match l {
                Ok(Some(l)) => {
                    if let Some(e) = Etat::lire(&l) {
                        evt.send(Evt::Etat(Box::new(e))).ok()?;
                    } else if let Some(r) = lire_reponse(&l) {
                        if let Some(tx) = attente.pop_front() {
                            let _ = tx.send(r);
                        }
                    }
                }
                _ => break Some(()),
            },
            c = cmds.recv() => match c {
                Some((ligne, rep)) => {
                    let envoi = ecriture.write_all(format!("{ligne}\n").as_bytes()).await;
                    if envoi.is_err() {
                        let _ = rep.send(Err("daemon injoignable".into()));
                        break Some(());
                    }
                    attente.push_back(rep);
                }
                None => break None,
            },
        }
    };
    for tx in attente {
        let _ = tx.send(Err("daemon injoignable".into()));
    }
    fin
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lignes_de_commande() {
        assert_eq!(ligne_commande("os", None), r#"{"cmd":"os"}"#);
        assert_eq!(
            ligne_commande("accepter", Some("sortie,1230")),
            r#"{"cmd":"accepter","rappel":"sortie,1230"}"#
        );
    }

    #[test]
    fn ligne_de_jauge() {
        assert_eq!(ligne_jauge(100), r#"{"cmd":"jauge","valeur":100}"#);
        assert_eq!(ligne_jauge(0), r#"{"cmd":"jauge","valeur":0}"#);
    }

    #[test]
    fn reponses() {
        assert_eq!(lire_reponse(r#"{"reponse":"os","ok":true}"#), Some(Ok(())));
        assert_eq!(
            lire_reponse(r#"{"reponse":"os","ok":false,"erreur":"board hors ligne"}"#),
            Some(Err("board hors ligne".to_string()))
        );
        assert_eq!(lire_reponse(r#"{"pc":{}}"#), None);
    }

    #[tokio::test]
    async fn boucle_relaie_etat_et_reponses() {
        use tokio::io::AsyncReadExt;
        use tokio::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("tgpc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("s.sock");
        let _ = std::fs::remove_file(&sock);
        let ecoute = UnixListener::bind(&sock).unwrap();
        let (etx, mut erx) = mpsc::unbounded_channel();
        let (ctx, crx) = mpsc::unbounded_channel();
        tokio::spawn(boucle(sock.clone(), etx, crx));
        let (mut s, _) = ecoute.accept().await.unwrap();
        let mut buf = vec![0u8; 64];
        let n = s.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"{\"hello\":\"osampo-pc\"}\n");
        s.write_all(b"{\"pc\":{\"icone\":\"niche\",\"board\":\"connectee\"}}\n").await.unwrap();
        match erx.recv().await.unwrap() {
            Evt::Etat(e) => assert_eq!(e.icone, "niche"),
            Evt::Injoignable => panic!("injoignable"),
        }
        let (rtx, rrx) = oneshot::channel();
        ctx.send((ligne_commande("os", None), rtx)).unwrap();
        let n = s.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"{\"cmd\":\"os\"}\n");
        s.write_all(b"{\"reponse\":\"os\",\"ok\":true}\n").await.unwrap();
        assert_eq!(rrx.await.unwrap(), Ok(()));
        drop(s);
        assert!(matches!(erx.recv().await.unwrap(), Evt::Injoignable));
        let _ = std::fs::remove_dir_all(dir);
    }
}
