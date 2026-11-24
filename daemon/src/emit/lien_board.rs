//! Contrat commun aux transports vers la board (BLE reel, board simulee) :
//! SPEC-001. Pas de logique du chiot ici, seulement trois choses :
//! - les lignes montantes (notifications `ETAT`) recollees jusqu au `\n` ;
//! - les lignes de commande descendantes ;
//! - l etat du lien (connecte = connectee et abonnee).
//!
//! `creer()` rend les deux moities : `LienBoard` pour le consommateur (la
//! tache « pc » d ACT-007), `CoteTransport` pour le transport.

use tokio::sync::{mpsc, watch};

/// Au-dela, une ligne montante est jetee entiere (SPEC-001).
pub const LIGNE_MAX: usize = 4096;

/// Capacite des deux files (lignes montantes, commandes).
const CAPACITE: usize = 64;

/// Recolle des morceaux d octets en lignes terminees par `\n`.
#[derive(Default)]
pub struct Recolleur {
    tampon: Vec<u8>,
    trop_longue: bool,
}

impl Recolleur {
    /// Les lignes completes apportees par `octets` (sans `\n`). Une ligne de
    /// plus de LIGNE_MAX octets est jetee jusqu a son `\n` ; une ligne vide
    /// ou non UTF-8 est ignoree. Les lignes suivantes passent normalement.
    pub fn pousser(&mut self, octets: &[u8]) -> Vec<String> {
        let mut lignes = Vec::new();
        for &o in octets {
            if o == b'\n' {
                let jetee = std::mem::take(&mut self.trop_longue);
                let brute = std::mem::take(&mut self.tampon);
                if jetee {
                    continue;
                }
                if let Ok(s) = String::from_utf8(brute) {
                    let s = s.trim_end_matches('\r');
                    if !s.trim().is_empty() {
                        lignes.push(s.to_string());
                    }
                }
            } else if self.trop_longue {
                // jetee jusqu au saut de ligne
            } else if self.tampon.len() >= LIGNE_MAX {
                self.trop_longue = true;
                self.tampon.clear();
            } else {
                self.tampon.push(o);
            }
        }
        lignes
    }

    pub fn vider(&mut self) {
        self.tampon.clear();
        self.trop_longue = false;
    }
}

/// Moitie du consommateur.
pub struct LienBoard {
    /// Lignes montantes (etat, evenements), une ligne par message, sans `\n`.
    pub lignes: mpsc::Receiver<String>,
    /// Lignes de commande a ecrire sur la board, sans `\n`.
    pub commandes: mpsc::Sender<String>,
    /// Vrai quand la board est connectee et abonnee a `ETAT`.
    pub connecte: watch::Receiver<bool>,
}

/// Moitie du transport.
pub struct CoteTransport {
    montantes: mpsc::Sender<String>,
    commandes: mpsc::Receiver<String>,
    connecte: watch::Sender<bool>,
    recolleur: Recolleur,
}

pub fn creer() -> (LienBoard, CoteTransport) {
    let (montantes, lignes) = mpsc::channel(CAPACITE);
    let (commandes_tx, commandes) = mpsc::channel(CAPACITE);
    let (connecte_tx, connecte) = watch::channel(false);
    (
        LienBoard { lignes, commandes: commandes_tx, connecte },
        CoteTransport { montantes, commandes, connecte: connecte_tx, recolleur: Recolleur::default() },
    )
}

impl CoteTransport {
    /// Nouvelle liaison : une ligne montante a moitie recue et les commandes
    /// restees en file pendant la coupure sont perimees.
    pub fn nouvelle_liaison(&mut self) {
        self.recolleur.vider();
        while self.commandes.try_recv().is_ok() {}
    }

    /// Octets montants recus (une notification, un bout de socket).
    pub fn recevoir(&mut self, octets: &[u8]) {
        for l in self.recolleur.pousser(octets) {
            // ponytail: file pleine (consommateur bloque) = ligne perdue plutot
            // qu un transport bloque ; l etat est remis au moins toutes les 30 s.
            let _ = self.montantes.try_send(l);
        }
    }

    /// Prochaine commande a ecrire ; None quand le consommateur a disparu.
    /// Annulable sans perte (recv d un mpsc).
    pub async fn commande(&mut self) -> Option<String> {
        self.commandes.recv().await
    }

    pub fn lien(&self, connecte: bool) {
        self.connecte.send_replace(connecte);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recolle_les_morceaux_de_20_octets() {
        let ligne = format!("{{\"etat\":\"repos\",\"pile\":83,\"x\":\"{}\"}}\n", "a".repeat(60));
        let mut r = Recolleur::default();
        let mut vues = Vec::new();
        for m in ligne.as_bytes().chunks(20) {
            vues.extend(r.pousser(m));
        }
        assert_eq!(vues, vec![ligne.trim_end().to_string()]);
    }

    #[test]
    fn plusieurs_lignes_dans_un_morceau_et_vides_ignorees() {
        let mut r = Recolleur::default();
        assert_eq!(r.pousser(b"a\n\nb\r\nc"), vec!["a", "b"]);
        assert_eq!(r.pousser(b"d\n"), vec!["cd"]);
    }

    #[test]
    fn ligne_de_4096_gardee_4097_jetee_sans_couper_la_suivante() {
        let mut r = Recolleur::default();
        let ok = "x".repeat(LIGNE_MAX);
        assert_eq!(r.pousser(format!("{ok}\n").as_bytes()), vec![ok.clone()]);
        let trop = "y".repeat(LIGNE_MAX + 1);
        let mut vues = Vec::new();
        for m in format!("{trop}\nsuite\n").as_bytes().chunks(20) {
            vues.extend(r.pousser(m));
        }
        assert_eq!(vues, vec!["suite"]);
    }

    #[test]
    fn ligne_non_utf8_ignoree() {
        let mut r = Recolleur::default();
        assert_eq!(r.pousser(b"\xff\xfe\nok\n"), vec!["ok"]);
    }

    #[tokio::test]
    async fn nouvelle_liaison_oublie_le_partiel_et_les_commandes() {
        let (lien, mut cote) = creer();
        cote.recevoir(b"parti");
        lien.commandes.send("{\"cmd\":\"os\"}".into()).await.unwrap();
        cote.nouvelle_liaison();
        cote.recevoir(b"el\n");
        let mut lien = lien;
        assert_eq!(lien.lignes.recv().await.as_deref(), Some("el"));
        lien.commandes.send("{\"cmd\":\"chatouille\"}".into()).await.unwrap();
        assert_eq!(cote.commande().await.as_deref(), Some("{\"cmd\":\"chatouille\"}"));
    }

    #[test]
    fn lien_suit_l_etat() {
        let (lien, cote) = creer();
        assert!(!*lien.connecte.borrow());
        cote.lien(true);
        assert!(*lien.connecte.borrow());
        drop(lien);
        cote.lien(false); // sans consommateur : pas de panique
    }
}
