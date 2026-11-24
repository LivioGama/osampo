//! Relecture a chaud de osampo.toml (spec SP5c C4, C5) : la date de
//! modification et la taille sont regardees toutes les 2 s, sans dependance.
//! Une relecture acceptee publie les reglages et la ligne cfg ; une relecture
//! refusee est journalisee avec la cle fautive et ne publie rien.

use crate::config::{self, Reglages};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tokio::sync::watch;

/// Intervalle entre deux regards sur le fichier (C4).
pub const INTERVALLE: Duration = Duration::from_secs(2);

/// Ce qui distingue deux versions du fichier sans le lire. La taille rattrape
/// deux ecritures dans la meme seconde sur un systeme de fichiers a mtime grossiere.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Empreinte {
    modifie: SystemTime,
    taille: u64,
}

fn empreinte(chemin: &std::path::Path) -> Option<Empreinte> {
    let m = std::fs::metadata(chemin).ok()?;
    Some(Empreinte { modifie: m.modified().ok()?, taille: m.len() })
}

/// Issue d un regard sur le fichier.
#[derive(Debug, PartialEq)]
pub enum Relecture {
    Inchangee,
    /// Relu et valide ; publie seulement s il differe des reglages courants.
    Acceptee,
    /// Relu et refuse : l erreur nomme la cle ; les reglages courants restent.
    Refusee(String),
    /// Fichier disparu : les reglages courants restent, rien n est recree.
    Disparu,
}

/// Le fichier et sa derniere empreinte vue.
pub struct Surveillant {
    chemin: PathBuf,
    vue: Option<Empreinte>,
}

impl Surveillant {
    /// Prend l empreinte tout de suite : a creer AVANT la lecture de
    /// demarrage, pour qu une ecriture entre les deux ne passe pas inapercue.
    pub fn new(chemin: PathBuf) -> Self {
        let vue = empreinte(&chemin);
        Surveillant { chemin, vue }
    }

    /// Un regard : relit si l empreinte a change, publie si c est accepte.
    pub fn etape(
        &mut self,
        reglages: &watch::Sender<Reglages>,
        cfg: &watch::Sender<Option<String>>,
    ) -> Relecture {
        let maintenant = empreinte(&self.chemin);
        if maintenant == self.vue {
            return Relecture::Inchangee;
        }
        self.vue = maintenant;
        if maintenant.is_none() {
            eprintln!(
                "config: {} disparu, reglages courants gardes jusqu au redemarrage",
                self.chemin.display()
            );
            return Relecture::Disparu;
        }
        // Un editeur qui ecrit sur place tronque d abord : un fichier vide est
        // un etat transitoire, pas une demande de revenir aux defauts. `vue`
        // garde la taille 0, le contenu sera relu des qu il arrive.
        if maintenant.is_some_and(|e| e.taille == 0) {
            eprintln!(
                "config: {} vide (ecriture en cours ?), reglages courants gardes",
                self.chemin.display()
            );
            return Relecture::Refusee("fichier vide".into());
        }
        let lus = std::fs::read_to_string(&self.chemin)
            .map_err(|e| format!("lecture de {}: {e}", self.chemin.display()))
            .and_then(|texte| config::lire(&texte));
        match lus {
            Ok(r) => {
                let ligne = r.ligne_cfg();
                eprintln!("config: {} relue", self.chemin.display());
                // send_if_modified : un fichier touche sans changement, ou un
                // changement de [poste] seul, ne renvoie rien a la board.
                cfg.send_if_modified(|c| {
                    let neuf = c.as_deref() != Some(ligne.as_str());
                    if neuf {
                        *c = Some(ligne);
                    }
                    neuf
                });
                reglages.send_if_modified(|c| {
                    let neuf = *c != r;
                    if neuf {
                        *c = r;
                    }
                    neuf
                });
                Relecture::Acceptee
            }
            Err(e) => {
                eprintln!(
                    "config: {} refusee: {} ; reglages precedents gardes",
                    self.chemin.display(),
                    e.trim_end()
                );
                Relecture::Refusee(e)
            }
        }
    }
}

/// Lance la tache de relecture. Ne s arrete qu avec le daemon.
pub fn surveiller(
    mut s: Surveillant,
    reglages: watch::Sender<Reglages>,
    cfg: watch::Sender<Option<String>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tic = tokio::time::interval(INTERVALLE);
        tic.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tic.tick().await;
            s.etape(&reglages, &cfg);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fichier(nom: &str, texte: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sp5c_vivante_{}_{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("osampo.toml");
        std::fs::write(&f, texte).unwrap();
        f
    }

    type Canaux = (
        watch::Sender<Reglages>,
        watch::Receiver<Reglages>,
        watch::Sender<Option<String>>,
        watch::Receiver<Option<String>>,
    );

    /// Les deux canaux comme main les cree, a partir du fichier lu au demarrage.
    fn canaux(initiaux: Reglages) -> Canaux {
        let ligne = initiaux.ligne_cfg();
        let (rt, rr) = watch::channel(initiaux);
        let (ct, cr) = watch::channel(Some(ligne));
        (rt, rr, ct, cr)
    }

    #[test]
    fn relecture_acceptee_publie_cfg() {
        let f = fichier("acceptee", "[ecran]\nlum_nuit = 20\n");
        let mut s = Surveillant::new(f.clone());
        let (rt, mut rr, ct, mut cr) = canaux(config::lire("[ecran]\nlum_nuit = 20\n").unwrap());
        rr.borrow_and_update();
        cr.borrow_and_update();

        assert_eq!(s.etape(&rt, &ct), Relecture::Inchangee);
        assert!(!cr.has_changed().unwrap());

        std::fs::write(&f, "[ecran]\nlum_nuit = 30\n[promenade]\ncourte_min = 7\n").unwrap();
        assert_eq!(s.etape(&rt, &ct), Relecture::Acceptee);
        assert!(cr.has_changed().unwrap(), "une relecture acceptee publie la ligne cfg");
        let ligne = cr.borrow_and_update().clone().unwrap();
        assert!(ligne.contains("\"lum_nuit\":30"), "{ligne}");
        assert_eq!(rr.borrow_and_update().courte_min, 7);

        // rien de neuf au regard suivant
        assert_eq!(s.etape(&rt, &ct), Relecture::Inchangee);
        assert!(!cr.has_changed().unwrap());
    }

    #[test]
    fn relecture_daemon_seule_ne_renvoie_pas_cfg() {
        // vide_apres_min ne part pas sur le fil : la ligne cfg ne change pas
        let f = fichier("poste", "");
        let mut s = Surveillant::new(f.clone());
        let (rt, mut rr, ct, mut cr) = canaux(Reglages::default());
        cr.borrow_and_update();
        std::fs::write(&f, "[promenade]\nvide_apres_min = 45\n").unwrap();
        assert_eq!(s.etape(&rt, &ct), Relecture::Acceptee);
        assert_eq!(rr.borrow_and_update().vide_apres_min, 45);
        assert!(!cr.has_changed().unwrap(), "meme ligne cfg : rien a renvoyer");
    }

    #[test]
    fn relecture_fautive_garde_l_ancien() {
        let f = fichier("fautive", "[ecran]\nlum_nuit = 30\n");
        let mut s = Surveillant::new(f.clone());
        let initiaux = config::lire("[ecran]\nlum_nuit = 30\n").unwrap();
        let (rt, mut rr, ct, mut cr) = canaux(initiaux.clone());
        rr.borrow_and_update();
        cr.borrow_and_update();

        std::fs::write(&f, "[ecran]\nlum_nuitt = 40\n").unwrap();
        match s.etape(&rt, &ct) {
            Relecture::Refusee(e) => assert!(e.contains("lum_nuitt"), "la cle fautive est nommee: {e}"),
            autre => panic!("refus attendu, eu {autre:?}"),
        }
        assert!(!cr.has_changed().unwrap(), "rien publie");
        assert!(!rr.has_changed().unwrap());
        assert_eq!(*rr.borrow(), initiaux);
        // le refus n est pas repete tant que le fichier ne change pas
        assert_eq!(s.etape(&rt, &ct), Relecture::Inchangee);

        // la faute corrigee : accepte
        std::fs::write(&f, "[ecran]\nlum_nuit = 40\n").unwrap();
        assert_eq!(s.etape(&rt, &ct), Relecture::Acceptee);
        assert!(cr.borrow_and_update().as_deref().unwrap().contains("\"lum_nuit\":40"));
    }

    #[test]
    fn fichier_vide_ignore_le_temps_de_l_ecriture() {
        // Un editeur qui ecrit sur place tronque d abord le fichier : le
        // regard qui tombe entre les deux ne doit pas envoyer les defauts.
        let f = fichier("vide", "[ecran]\nlum_nuit = 30\n");
        let mut s = Surveillant::new(f.clone());
        let initiaux = config::lire("[ecran]\nlum_nuit = 30\n").unwrap();
        let (rt, mut rr, ct, mut cr) = canaux(initiaux.clone());
        rr.borrow_and_update();
        cr.borrow_and_update();

        std::fs::write(&f, "").unwrap();
        match s.etape(&rt, &ct) {
            Relecture::Refusee(e) => assert!(e.contains("vide"), "{e}"),
            autre => panic!("refus attendu, eu {autre:?}"),
        }
        assert!(!cr.has_changed().unwrap(), "rien publie");
        assert!(!rr.has_changed().unwrap());
        assert_eq!(*rr.borrow(), initiaux);

        // le contenu arrive : relu normalement
        std::fs::write(&f, "[ecran]\nlum_nuit = 40\n").unwrap();
        assert_eq!(s.etape(&rt, &ct), Relecture::Acceptee);
        assert!(cr.borrow_and_update().as_deref().unwrap().contains("\"lum_nuit\":40"));
    }

    #[test]
    fn fichier_supprime_garde_les_reglages_sans_le_recreer() {
        let f = fichier("supprime", "[ecran]\nlum_nuit = 30\n");
        let mut s = Surveillant::new(f.clone());
        let initiaux = config::lire("[ecran]\nlum_nuit = 30\n").unwrap();
        let (rt, rr, ct, mut cr) = canaux(initiaux.clone());
        cr.borrow_and_update();

        std::fs::remove_file(&f).unwrap();
        assert_eq!(s.etape(&rt, &ct), Relecture::Disparu);
        assert_eq!(s.etape(&rt, &ct), Relecture::Inchangee, "signale une seule fois");
        assert!(!f.exists(), "jamais recree en cours de route");
        assert_eq!(*rr.borrow(), initiaux);
        assert!(!cr.has_changed().unwrap());
    }
}
