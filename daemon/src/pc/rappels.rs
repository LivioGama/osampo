//! Rappels tenus par le daemon (SPEC-003) : declenchement des rappels `sortie` / `sport` /
//! `dodo`, jour du dernier declenchement persiste, texte `prochain_rappel`, heures calmes
//! (SPEC-008). Pur : l heure est injectee.

use super::{semaine_de, Heure};
use crate::config::Reglages;
use std::collections::HashMap;
use std::path::PathBuf;

/// Categories qui declenchent quelque chose sur le PC (ADR-001, QB-3).
pub const PORTEES: [&str; 3] = ["sortie", "sport", "dodo"];

const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

const FICHIER: &str = "rappels_osampo_pc";

#[derive(Clone, Debug, PartialEq)]
pub struct Rappel {
    pub categorie: String,
    /// Minute du jour 0..1439.
    pub minute: i64,
    /// Bit k = jour k, 0 = lundi.
    pub masque: u32,
    /// `categorie,HHMM` (SPEC-001).
    pub id: String,
}

impl Rappel {
    pub fn heure(&self) -> String {
        format!("{:02}:{:02}", self.minute / 60, self.minute % 60)
    }
}

/// Un rappel du : le rappel et le jour (de sa programmation) a enregistrer.
#[derive(Clone, Debug, PartialEq)]
pub struct Du {
    pub rappel: Rappel,
    pub jour: i64,
}

/// Texte du fil `categorie,HHMM,masque;...` (deja valide par `config`).
pub fn lire_texte(t: &str) -> Vec<Rappel> {
    t.split(';')
        .filter_map(|e| {
            let mut p = e.split(',');
            let (categorie, hhmm, masque) = (p.next()?, p.next()?, p.next()?);
            if hhmm.len() != 4 {
                return None;
            }
            let h: i64 = hhmm[..2].parse().ok()?;
            let m: i64 = hhmm[2..].parse().ok()?;
            Some(Rappel {
                categorie: categorie.to_string(),
                minute: h * 60 + m,
                masque: masque.parse().ok()?,
                id: format!("{categorie},{hhmm}"),
            })
        })
        .collect()
}

pub struct Rappels {
    liste: Vec<Rappel>,
    rattrapage_min: i64,
    calme_debut: i64,
    calme_fin: i64,
    /// id -> jour (programmation) du dernier declenchement.
    declenches: HashMap<String, i64>,
    fichier: Option<PathBuf>,
}

impl Rappels {
    /// `dossier` : `sources::activite::dossier_etat()` ; `None` en test sans persistance.
    pub fn new(dossier: Option<PathBuf>, reglages: &Reglages) -> Rappels {
        let fichier = dossier.map(|d| d.join(FICHIER));
        let declenches = fichier.as_ref().map(lire_fichier).unwrap_or_default();
        let mut r = Rappels {
            liste: Vec::new(),
            rattrapage_min: 0,
            calme_debut: 0,
            calme_fin: 0,
            declenches,
            fichier,
        };
        r.reglages(reglages);
        r
    }

    /// Releve des reglages (valables des la relecture du daemon).
    pub fn reglages(&mut self, r: &Reglages) {
        self.liste = lire_texte(&r.rappels);
        self.rattrapage_min = r.rattrapage_min;
        self.calme_debut = r.calme_debut;
        self.calme_fin = r.calme_fin;
    }

    /// Heures calmes : `calme_debut` inclus a `calme_fin` exclu, minuit passe si debut > fin ;
    /// debut = fin : jamais calme (SPEC-008).
    pub fn calme(&self, minute: i64) -> bool {
        let (d, f) = (self.calme_debut, self.calme_fin);
        if d == f {
            false
        } else if d < f {
            minute >= d && minute < f
        } else {
            minute >= d || minute < f
        }
    }

    pub fn rappel(&self, id: &str) -> Option<&Rappel> {
        self.liste.iter().find(|r| r.id == id)
    }

    /// Le rappel `sortie` / `sport` / `dodo` du, le plus tot programme (SPEC-003) : de sa
    /// minute a `rattrapage_min` minutes apres (minuit passe), un jour de son masque, hors
    /// heures calmes, pas deja declenche pour ce jour.
    pub fn du(&self, now: Heure) -> Option<Du> {
        if self.calme(now.minute) {
            return None;
        }
        let mut meilleur: Option<(i64, Du)> = None;
        for r in self.liste.iter().filter(|r| PORTEES.contains(&r.categorie.as_str())) {
            for jour in [now.jour - 1, now.jour] {
                if r.masque & (1 << semaine_de(jour)) == 0 {
                    continue;
                }
                let retard = (now.jour - jour) * 1440 + now.minute - r.minute;
                if !(0..=self.rattrapage_min).contains(&retard) {
                    continue;
                }
                if self.declenches.get(&r.id).is_some_and(|&j| j >= jour) {
                    continue;
                }
                if meilleur.as_ref().is_none_or(|(b, _)| retard > *b) {
                    meilleur = Some((retard, Du { rappel: r.clone(), jour }));
                }
            }
        }
        meilleur.map(|(_, d)| d)
    }

    /// Enregistre un declenchement (ou une adoption) et le persiste.
    pub fn marquer(&mut self, id: &str, jour: i64) {
        self.declenches.insert(id.to_string(), jour);
        if let Some(f) = &self.fichier {
            let mut t = String::new();
            let mut v: Vec<_> = self.declenches.iter().collect();
            v.sort();
            for (id, j) in v {
                t.push_str(&format!("{j} {id}\n"));
            }
            if let Some(d) = f.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            if let Err(e) = std::fs::write(f, t) {
                eprintln!("rappels: ecriture de {} impossible: {e}", f.display());
            }
        }
    }

    /// Texte `prochain_rappel` : parmi les rappels pas encore declenches aujourd hui et ceux
    /// des 7 jours suivants, hors heures calmes, toutes categories ; `en_cours` exclu.
    pub fn prochain(&self, now: Heure, en_cours: Option<&str>) -> String {
        let mut meilleur: Option<(i64, i64, &Rappel)> = None;
        for d in 0..=7 {
            let jour = now.jour + d;
            for r in &self.liste {
                if r.masque & (1 << semaine_de(jour)) == 0 || self.calme(r.minute) {
                    continue;
                }
                if d == 0
                    && (r.minute < now.minute
                        || en_cours == Some(r.id.as_str())
                        || self.declenches.get(&r.id).is_some_and(|&j| j >= jour))
                {
                    continue;
                }
                if meilleur.is_none_or(|(bd, bm, _)| (d, r.minute) < (bd, bm)) {
                    meilleur = Some((d, r.minute, r));
                }
            }
        }
        match meilleur {
            None => "aucun".into(),
            Some((0, _, r)) => format!("{} à {}", r.categorie, r.heure()),
            Some((1, _, r)) => format!("{} demain à {}", r.categorie, r.heure()),
            Some((d, _, r)) => {
                format!("{} {} à {}", r.categorie, JOURS[semaine_de(now.jour + d)], r.heure())
            }
        }
    }
}

fn lire_fichier(f: &PathBuf) -> HashMap<String, i64> {
    let mut m = HashMap::new();
    for l in std::fs::read_to_string(f).unwrap_or_default().lines() {
        if let Some((j, id)) = l.split_once(' ') {
            if let Ok(j) = j.parse() {
                m.insert(id.to_string(), j);
            }
        }
    }
    m
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// 2026-11-22 (dimanche) a `h:m`, UTC.
    pub fn dim(h: i64, m: i64) -> Heure {
        jour_plus(0, h, m)
    }

    pub fn jour_plus(d: i64, h: i64, m: i64) -> Heure {
        Heure::avec_decalage(1_795_305_600 + d * 86400 + h * 3600 + m * 60, 0)
    }

    pub fn reglages(rappels: &str) -> Reglages {
        Reglages {
            rappels: rappels.into(),
            calme_debut: 22 * 60,
            calme_fin: 7 * 60,
            rattrapage_min: 60,
            ..Reglages::default()
        }
    }

    fn r(t: &str) -> Rappels {
        Rappels::new(None, &reglages(t))
    }

    #[test]
    fn jour_de_reference() {
        assert_eq!(semaine_de(dim(0, 0).jour), 6);
        assert_eq!(dim(14, 12).hhmm(), "14:12");
    }

    #[test]
    fn minute_rattrapage_et_une_fois_par_jour() {
        let mut r = r("sortie,1230,127");
        assert!(r.du(dim(12, 29)).is_none());
        let d = r.du(dim(12, 30)).unwrap();
        assert_eq!(d.rappel.id, "sortie,1230");
        assert!(r.du(dim(13, 30)).is_some()); // rattrapage_min = 60 inclus
        assert!(r.du(dim(13, 31)).is_none());
        r.marquer(&d.rappel.id, d.jour);
        assert!(r.du(dim(12, 31)).is_none());
        assert!(r.du(jour_plus(1, 12, 30)).is_some()); // le lendemain
    }

    #[test]
    fn jours_repas_et_calme() {
        // 21 = lundi + jeudi : dimanche exclu. repas ne declenche rien.
        let r2 = r("sortie,1230,9;repas,1200,127");
        assert!(r2.du(dim(12, 30)).is_none());
        assert!(r2.du(jour_plus(1, 12, 30)).is_some()); // lundi
        assert!(r("repas,1200,127").du(dim(12, 0)).is_none());
        // jamais en heures calmes, meme dans le rattrapage
        let c = r("dodo,2145,127");
        assert!(c.du(dim(21, 45)).is_some());
        assert!(c.du(dim(22, 10)).is_none());
        assert!(c.calme(22 * 60) && c.calme(6 * 60 + 59) && !c.calme(7 * 60) && !c.calme(21 * 60 + 59));
    }

    #[test]
    fn rattrapage_a_minuit_et_le_plus_tot_du() {
        let mut g = reglages("sortie,2350,127");
        g.calme_debut = 0;
        g.calme_fin = 0; // jamais calme
        let x = Rappels::new(None, &g);
        let d = x.du(jour_plus(1, 0, 20)).unwrap();
        assert_eq!(d.jour, dim(0, 0).jour);
        let y = Rappels::new(None, &{
            let mut g = reglages("sport,1200,127;sortie,1230,127");
            g.calme_debut = 0;
            g.calme_fin = 0;
            g
        });
        assert_eq!(y.du(dim(12, 40)).unwrap().rappel.categorie, "sport");
    }

    #[test]
    fn persistance_du_jour() {
        let dir = std::env::temp_dir().join(format!("pc_rappels_{}", std::process::id()));
        let g = reglages("sortie,1230,127");
        let mut a = Rappels::new(Some(dir.clone()), &g);
        let d = a.du(dim(12, 30)).unwrap();
        a.marquer(&d.rappel.id, d.jour);
        let b = Rappels::new(Some(dir.clone()), &g);
        assert!(b.du(dim(12, 40)).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prochain_rappel_formats() {
        let mut x = r("repas,1200,127;sortie,1230,127;sport,1800,1");
        assert_eq!(x.prochain(dim(11, 0), None), "repas à 12:00");
        assert_eq!(x.prochain(dim(12, 10), None), "sortie à 12:30");
        // en cours : plus « prochain »
        assert_eq!(x.prochain(dim(12, 10), Some("sortie,1230")), "repas demain à 12:00");
        x.marquer("sortie,1230", dim(0, 0).jour);
        x.marquer("repas,1200", dim(0, 0).jour);
        assert_eq!(x.prochain(dim(12, 10), None), "repas demain à 12:00");
        // sport lundi seulement (masque 1) : dimanche 19:00 -> repas demain
        assert_eq!(x.prochain(dim(19, 0), None), "repas demain à 12:00");
        let y = r("sport,1800,2"); // mardi
        assert_eq!(y.prochain(dim(19, 0), None), "sport mardi à 18:00");
        assert_eq!(r("sport,1800,64").prochain(dim(19, 0), None), "sport dimanche à 18:00");
        // un rappel en heures calmes n est pas propose
        assert_eq!(r("dodo,2300,127").prochain(dim(19, 0), None), "aucun");
        assert_eq!(r("repas,1200,127").prochain(dim(13, 0), None), "repas demain à 12:00");
    }
}
