//! Board simulee pour les tests (SPEC-001, option `--faux-ble <chemin>`) : au
//! lieu de BlueZ, le daemon ecoute un socket Unix. Un client connecte joue la
//! board : il recoit ligne par ligne tout ce que le daemon ecrirait sur
//! `LIGNES` (ligne cfg, trames, commandes) ; ce qu il ecrit est traite comme
//! les notifications `ETAT`. Client connecte = lien connecte ; deconnecte =
//! coupe. Un seul client a la fois : un second est ferme aussitot.
//!
//! Meme boucle d emission que le BLE reel (`ble::emettre`), sans le saut de
//! ligne de resynchronisation : le flux Unix ne coupe pas les lignes.

use super::ble::{attendre_fermeture, emettre, Canal};
use super::lien_board::CoteTransport;
use crate::model::Frame;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, watch};

/// Le client connecte, vu comme une liaison. Il garde l ecouteur pour fermer
/// tout second client pendant que celui-ci est la.
struct Client {
    lecture: OwnedReadHalf,
    ecriture: OwnedWriteHalf,
    ecouteur: Arc<UnixListener>,
}

#[async_trait::async_trait]
impl Canal for Client {
    async fn envoyer(&mut self, octets: &[u8]) -> Result<()> {
        self.ecriture.write_all(octets).await.context("ecriture vers la board simulee")
    }

    async fn notification(&mut self) -> Result<Vec<u8>> {
        let mut tampon = [0u8; 512];
        loop {
            tokio::select! {
                n = self.lecture.read(&mut tampon) => {
                    let n = n.context("lecture de la board simulee")?;
                    if n == 0 {
                        anyhow::bail!("board simulee deconnectee");
                    }
                    return Ok(tampon[..n].to_vec());
                }
                // Un second client : refuse, le premier n est pas touche.
                r = self.ecouteur.accept() => drop(r),
            }
        }
    }

    fn abonne(&self) -> bool {
        true
    }
}

/// Sert la board simulee sur `chemin` jusqu a la fermeture du broadcast.
/// Err : le socket ne s ouvre pas.
pub async fn servir(
    chemin: PathBuf,
    mut rx: broadcast::Receiver<Frame>,
    mut cfg: watch::Receiver<Option<String>>,
    mut cote: CoteTransport,
) -> Result<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("creation de {}", d.display()))?;
    }
    let _ = std::fs::remove_file(&chemin); // reste d un daemon precedent
    let ecouteur = Arc::new(UnixListener::bind(&chemin).with_context(|| format!("ecoute sur {}", chemin.display()))?);
    eprintln!("faux-ble: board simulee sur {}", chemin.display());
    loop {
        // Sans client, les trames sont jetees comme celles d une board hors de portee.
        let flux: UnixStream = tokio::select! {
            r = ecouteur.accept() => match r {
                Ok((f, _)) => f,
                Err(e) => {
                    eprintln!("faux-ble: accept: {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            },
            _ = attendre_fermeture(&mut rx) => return Ok(()),
        };
        let (lecture, ecriture) = flux.into_split();
        let mut client = Client { lecture, ecriture, ecouteur: ecouteur.clone() };
        eprintln!("faux-ble: board connectee");
        cote.nouvelle_liaison();
        cote.lien(true);
        let fin = emettre(&mut client, false, &mut rx, &mut cfg, &mut cote).await;
        cote.lien(false);
        match fin {
            Ok(()) => return Ok(()), // broadcast ferme
            Err(e) => eprintln!("faux-ble: board perdue ({e:#})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::lien_board::{creer, LienBoard};
    use crate::model::{Keys, Value};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, BufReader};

    fn trame(n: i64) -> Frame {
        let mut k = Keys::new();
        k.insert("n".into(), Value::Int(n));
        Frame(k)
    }

    struct Banc {
        chemin: PathBuf,
        tx: broadcast::Sender<Frame>,
        lien: LienBoard,
        _cfg: watch::Sender<Option<String>>,
        tache: tokio::task::JoinHandle<Result<()>>,
    }

    fn demarrer(nom: &str) -> Banc {
        let chemin = std::env::temp_dir().join(format!("faux_ble_{nom}_{}.sock", std::process::id()));
        let (tx, rx) = broadcast::channel(8);
        let (cfg_tx, cfg) = watch::channel(Some("{\"cfg\":1}".to_string()));
        let (lien, cote) = creer();
        let tache = tokio::spawn(servir(chemin.clone(), rx, cfg, cote));
        Banc { chemin, tx, lien, _cfg: cfg_tx, tache }
    }

    async fn connecter(chemin: &PathBuf) -> UnixStream {
        for _ in 0..100 {
            if let Ok(s) = UnixStream::connect(chemin).await {
                return s;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("socket {} jamais ouvert", chemin.display());
    }

    async fn ligne<R: tokio::io::AsyncBufRead + Unpin>(r: &mut R) -> String {
        let mut s = String::new();
        tokio::time::timeout(Duration::from_secs(2), r.read_line(&mut s))
            .await
            .expect("ligne attendue")
            .unwrap();
        s
    }

    async fn attendre_lien(lien: &mut LienBoard, attendu: bool) {
        tokio::time::timeout(Duration::from_secs(2), lien.connecte.wait_for(|v| *v == attendu))
            .await
            .expect("changement du lien")
            .unwrap();
    }

    #[tokio::test]
    async fn la_board_recoit_cfg_trames_et_commandes_et_notifie() {
        let mut b = demarrer("flux");
        assert!(!*b.lien.connecte.borrow());
        let client = connecter(&b.chemin).await;
        let (lect, mut ecr) = client.into_split();
        let mut lect = BufReader::new(lect);
        attendre_lien(&mut b.lien, true).await;

        assert_eq!(ligne(&mut lect).await, "{\"cfg\":1}\n");
        b.tx.send(trame(7)).unwrap();
        assert_eq!(ligne(&mut lect).await, "{\"n\":7}\n");
        b.lien.commandes.send("{\"cmd\":\"os\"}".into()).await.unwrap();
        assert_eq!(ligne(&mut lect).await, "{\"cmd\":\"os\"}\n");

        // notification en morceaux de 20 octets, recollee
        let etat = b"{\"etat\":\"repos\",\"raison\":\"-\",\"pile\":83}\n";
        for m in etat.chunks(20) {
            ecr.write_all(m).await.unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let recue = tokio::time::timeout(Duration::from_secs(2), b.lien.lignes.recv()).await.unwrap();
        assert_eq!(recue.as_deref(), Some("{\"etat\":\"repos\",\"raison\":\"-\",\"pile\":83}"));
        b.tache.abort();
    }

    #[tokio::test]
    async fn un_second_client_est_ferme_et_le_premier_garde_le_lien() {
        let mut b = demarrer("second");
        let premier = connecter(&b.chemin).await;
        attendre_lien(&mut b.lien, true).await;
        let mut second = connecter(&b.chemin).await;
        let mut octet = [0u8; 1];
        let n = tokio::time::timeout(Duration::from_secs(2), second.read(&mut octet)).await.unwrap().unwrap();
        assert_eq!(n, 0, "le second client doit etre ferme");
        assert!(*b.lien.connecte.borrow(), "le premier garde le lien");
        // le premier reste servi
        let (lect, _ecr) = premier.into_split();
        let mut lect = BufReader::new(lect);
        assert_eq!(ligne(&mut lect).await, "{\"cfg\":1}\n");
        b.tache.abort();
    }

    #[tokio::test]
    async fn deconnexion_coupe_le_lien_puis_un_nouveau_client_le_retablit() {
        let mut b = demarrer("recon");
        let client = connecter(&b.chemin).await;
        attendre_lien(&mut b.lien, true).await;
        drop(client);
        attendre_lien(&mut b.lien, false).await;
        let client = connecter(&b.chemin).await;
        attendre_lien(&mut b.lien, true).await;
        let mut lect = BufReader::new(client);
        assert_eq!(ligne(&mut lect).await, "{\"cfg\":1}\n", "la cfg repart a chaque connexion");
        b.tache.abort();
    }

    #[tokio::test]
    async fn broadcast_ferme_arrete_le_service() {
        let b = demarrer("fin");
        let _client = connecter(&b.chemin).await;
        drop(b.tx);
        let r = tokio::time::timeout(Duration::from_secs(2), b.tache).await.unwrap().unwrap();
        assert!(r.is_ok());
    }
}
