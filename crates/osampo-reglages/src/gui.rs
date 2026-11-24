//! Fenetre Slint de `osampo-reglages` (SPEC-007) : une seule instance par utilisateur,
//! quatre onglets par usage (`pages.rs`), validation par les regles du daemon (`Edition::valider`).
//!
//! `Modele` porte toute la logique (sans Slint, donc testable) ; `lancer` ne fait que relier
//! la fenetre au modele.

use std::cell::RefCell;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use osampo_config::bornes::{self, Cle, Genre};
use toml_edit::Value;

use crate::daemon::{self, Evt};
use crate::edition::{Edition, Rappel};
use crate::pages::{self, Page};

slint::include_modules!();

/// Rappel tel que saisi : trois textes (les jours sont separes par des espaces ou virgules).
#[derive(Clone, Debug, PartialEq)]
pub struct RappelTexte {
    pub categorie: String,
    pub heure: String,
    pub jours: String,
}

impl RappelTexte {
    fn depuis(r: &Rappel) -> Self {
        RappelTexte {
            categorie: r.categorie.clone(),
            heure: r.heure.clone(),
            jours: r.jours.join(" "),
        }
    }

    fn rappel(&self) -> Rappel {
        Rappel {
            categorie: self.categorie.clone(),
            heure: pages::heure_normalisee(&self.heure),
            jours: self
                .jours
                .split(|c: char| c == ' ' || c == ',')
                .filter(|j| !j.is_empty())
                .map(|j| j.to_lowercase())
                .collect(),
        }
    }
}

/// Erreurs a afficher, une par champ.
#[derive(Debug, Default, PartialEq)]
pub struct Erreurs {
    pub champs: Vec<String>,
    pub rappels: Vec<String>,
    pub globale: String,
}

impl Erreurs {
    pub fn vide(&self) -> bool {
        self.globale.is_empty()
            && self.champs.iter().all(String::is_empty)
            && self.rappels.iter().all(String::is_empty)
    }
}

pub struct Modele {
    ed: Edition,
    /// Une entree par cle de `bornes::CLES` : texte saisi (affichage, pas TOML).
    pub champs: Vec<String>,
    /// Erreur de syntaxe de la saisie (jamais appliquee au document), par cle.
    syntaxe: Vec<String>,
    pub rappels: Vec<RappelTexte>,
}

/// Valeur TOML -> texte du champ : guillemets retires pour une heure ou une touche, crochets
/// retires pour une liste.
fn afficher(genre: Genre, toml: &str) -> String {
    let Ok(v) = toml.parse::<Value>() else { return toml.to_string() };
    match (genre, &v) {
        (_, Value::String(s)) => s.value().to_string(),
        (Genre::Liste { .. }, Value::Array(a)) => a
            .iter()
            .map(|x| x.to_string().trim().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        _ => toml.to_string(),
    }
}

/// Texte du champ -> valeur TOML (inverse de `afficher`).
fn vers_toml(genre: Genre, texte: &str) -> String {
    let t = texte.trim();
    match genre {
        Genre::Heure => Value::from(pages::heure_normalisee(t)).to_string(),
        Genre::Touche => Value::from(t).to_string(),
        Genre::Liste { .. } if !t.starts_with('[') => format!("[{t}]"),
        _ => t.to_string(),
    }
}

impl Modele {
    pub fn new(ed: Edition) -> Self {
        let champs = bornes::CLES.iter().map(|c| afficher(c.genre, &ed.valeur(c))).collect();
        let rappels = ed.rappels().iter().map(RappelTexte::depuis).collect();
        let mut m = Modele { ed, champs, syntaxe: vec![String::new(); bornes::CLES.len()], rappels };
        m.trier_rappels();
        m
    }

    /// Bouton « Valeurs par defaut » : le document devient le modele (`osampo_config::MODELE`,
    /// valeurs par defaut et liste de rappels d'origine), pour le meme fichier ; rien n'est ecrit
    /// avant `enregistrer`.
    pub fn valeurs_par_defaut(&mut self) {
        let ed = Edition::depuis_texte(osampo_config::MODELE, self.ed.chemin())
            .expect("le modele integre se lit toujours");
        *self = Modele::new(ed);
    }

    /// Range les rappels par heure dans la journee (heure invalide en fin, ordre garde sinon) et
    /// complete les heures `H:MM`. Rend `true` si la liste affichee doit etre recreee ; sinon le
    /// fichier n'est pas touche.
    pub fn trier_rappels(&mut self) -> bool {
        let avant = self.rappels.clone();
        for r in &mut self.rappels {
            r.heure = pages::heure_normalisee(&r.heure);
        }
        self.rappels.sort_by_key(|r| pages::minutes(&r.heure).unwrap_or(u32::MAX));
        if self.rappels == avant {
            return false;
        }
        for (n, r) in self.rappels.iter().enumerate() {
            let _ = self.ed.remplacer_rappel(n + 1, &r.rappel());
        }
        true
    }

    pub fn cle(i: usize) -> &'static Cle {
        &bornes::CLES[i]
    }

    /// Saisie dans le champ de la cle `i`.
    pub fn saisir(&mut self, i: usize, texte: &str) {
        let c = &bornes::CLES[i];
        self.champs[i] = texte.to_string();
        match self.ed.modifier(&c.nom(), &vers_toml(c.genre, texte)) {
            Ok(()) => self.syntaxe[i].clear(),
            Err(m) => self.syntaxe[i] = m,
        }
    }

    /// Saisie dans une colonne (0 categorie, 1 heure, 2 jours) du rappel `n` (a partir de 0).
    pub fn saisir_rappel(&mut self, n: usize, colonne: i32, texte: &str) {
        let Some(r) = self.rappels.get_mut(n) else { return };
        match colonne {
            0 => r.categorie = texte.to_string(),
            1 => r.heure = texte.to_string(),
            _ => r.jours = texte.to_string(),
        }
        let rappel = r.rappel();
        let _ = self.ed.remplacer_rappel(n + 1, &rappel);
    }

    pub fn ajouter_rappel(&mut self) {
        if self.rappels.len() >= 16 {
            return;
        }
        let r = Rappel::nouveau("repas", "12:00");
        self.ed.ajouter_rappel(&r);
        self.rappels.push(RappelTexte::depuis(&r));
        self.trier_rappels(); // le nouveau rappel prend sa place a 12:00
    }

    /// Le dernier rappel ne se supprime pas (le daemon en veut 1 a 16).
    pub fn supprimer_rappel(&mut self, n: usize) {
        if self.ed.supprimer_rappel(n + 1).is_ok() {
            self.rappels.remove(n);
        }
    }

    /// Erreurs courantes : syntaxe de saisie, puis message du daemon rattache a son champ.
    pub fn erreurs(&self) -> Erreurs {
        let mut e = Erreurs {
            champs: self.syntaxe.clone(),
            rappels: vec![String::new(); self.rappels.len()],
            globale: String::new(),
        };
        let Err(m) = self.ed.valider() else { return e };
        if let Some(i) = bornes::CLES.iter().position(|c| m.starts_with(&format!("{}:", c.nom()))) {
            if e.champs[i].is_empty() {
                e.champs[i] = m;
            }
        } else if let Some(n) = numero_rappel(&m).filter(|n| (1..=e.rappels.len()).contains(n)) {
            e.rappels[n - 1] = m;
        } else {
            e.globale = m;
        }
        e
    }

    pub fn enregistrable(&self) -> bool {
        self.erreurs().vide()
    }

    pub fn enregistrer(&self) -> Result<(), String> {
        if !self.enregistrable() {
            return Err("des champs sont invalides".into());
        }
        self.ed.enregistrer()
    }
}

/// `rappel 3: heure ...` -> 3.
fn numero_rappel(m: &str) -> Option<usize> {
    m.strip_prefix("rappel ")?.split(':').next()?.trim().parse().ok()
}

/// Verrou `flock` par utilisateur dans `$XDG_RUNTIME_DIR`. `Ok(Some)` : verrou pris (a garder
/// jusqu'a la fin du programme) ; `Ok(None)` : une autre instance tourne deja.
pub fn verrouiller(dossier: &Path) -> Result<Option<File>, String> {
    let chemin = dossier.join("osampo-reglages.lock");
    let f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&chemin)
        .map_err(|e| format!("verrou {}: {e}", chemin.display()))?;
    match f.try_lock() {
        Ok(()) => Ok(Some(f)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(format!("verrou {}: {e}", chemin.display())),
    }
}

fn dossier_verrou() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn texte(v: &[String]) -> ModelRc<SharedString> {
    ModelRc::from(Rc::new(VecModel::from(v.iter().map(SharedString::from).collect::<Vec<_>>())))
}

/// Indices (dans `CLES`) affiches par chaque liste de la fenetre, calcules une fois.
struct Disposition {
    rappels_fins: Vec<usize>,
    plages: Vec<pages::Plage>,
    horaires: Vec<usize>,
    promenade: Vec<usize>,
    board: Vec<usize>,
}

impl Disposition {
    fn new() -> Self {
        let plages = pages::plages();
        let dans_plage = |i: &usize| plages.iter().any(|p| p.debut == *i || p.fin == *i);
        let horaires = pages::indices(Page::Horaires).into_iter().filter(|i| !dans_plage(i)).collect();
        Disposition {
            rappels_fins: pages::indices(Page::Rappels),
            horaires,
            plages,
            promenade: pages::indices(Page::Promenade),
            board: pages::indices(Page::Board),
        }
    }
}

fn champ(m: &Modele, i: usize, bloc: &str) -> Champ {
    let c = Modele::cle(i);
    Champ {
        index: i as i32,
        nom: c.nom().into(),
        libelle: c.libelle.into(),
        aide: c.aide.into(),
        bornes: pages::bornes_courtes(c).into(),
        bornes_longues: c.bornes.into(),
        bloc: bloc.into(),
        heure: c.genre == Genre::Heure,
        valeur: m.champs[i].clone().into(),
    }
}

/// Champs d'une liste ; le sous-titre n'est porte que par le premier champ de chaque bloc
/// (aucun sur la page Rappels : le depliant « Reglages fins » en tient lieu).
fn champs(m: &Modele, indices: &[usize]) -> ModelRc<Champ> {
    let mut precedent = "";
    let v: Vec<Champ> = indices
        .iter()
        .map(|&i| {
            let (page, bloc) = pages::place(Modele::cle(i));
            let bloc = if page == Page::Rappels { "" } else { bloc };
            let titre = if bloc != precedent { bloc } else { "" };
            precedent = bloc;
            champ(m, i, titre)
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
}

fn plages_h(m: &Modele, d: &Disposition) -> ModelRc<PlageH> {
    let v: Vec<PlageH> = d
        .plages
        .iter()
        .map(|p| PlageH {
            libelle: p.libelle.into(),
            note: p.note.clone().into(),
            debut: champ(m, p.debut, ""),
            fin: champ(m, p.fin, ""),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
}

fn lignes_rappels(m: &Modele) -> ModelRc<LigneRappel> {
    let v: Vec<LigneRappel> = m
        .rappels
        .iter()
        .map(|r| LigneRappel {
            categorie: r.categorie.clone().into(),
            heure: r.heure.clone().into(),
            jours: ModelRc::from(Rc::new(VecModel::from(pages::cases(&r.jours).to_vec()))),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
}

/// Libelle et aide d'un champ de rappel, pour le bandeau d'aide.
fn aide_rappel(t: &bornes::TexteRappel) -> AideTexte {
    AideTexte { titre: t.libelle.into(), texte: t.aide.into() }
}

/// Reporte les erreurs, l'etat du bouton et le statut de blocage dans la fenetre. Seules les
/// proprietes d'erreur changent : les listes de champs ne sont pas recreees pendant la frappe.
fn rafraichir(w: &Reglages, m: &Modele, d: &Disposition) {
    let e = m.erreurs();
    let de = |v: &[usize]| -> Vec<String> { v.iter().map(|&i| e.champs[i].clone()).collect() };
    let fins = de(&d.rappels_fins);
    w.set_fins_en_erreur(fins.iter().any(|x| !x.is_empty()));
    w.set_erreurs_rappels_fins(texte(&fins));
    let plages: Vec<String> =
        d.plages.iter().flat_map(|p| [e.champs[p.debut].clone(), e.champs[p.fin].clone()]).collect();
    w.set_erreurs_plages(texte(&plages));
    w.set_erreurs_horaires(texte(&de(&d.horaires)));
    w.set_erreurs_promenade(texte(&de(&d.promenade)));
    w.set_erreurs_board(texte(&de(&d.board)));
    w.set_erreurs_rappels(texte(&e.rappels));
    w.set_pages_en_erreur(ModelRc::from(Rc::new(VecModel::from(pages::pages_en_erreur(&e).to_vec()))));
    w.set_erreur_globale(e.globale.clone().into());
    w.set_enregistrable(e.vide());
    w.set_statut(pages::texte_blocage(&e).into());
}

/// Recree toutes les listes de champs et de rappels depuis le modele, puis les erreurs.
fn remplir(w: &Reglages, m: &Modele, d: &Disposition) {
    w.set_champs_rappels(champs(m, &d.rappels_fins));
    w.set_plages(plages_h(m, d));
    w.set_champs_horaires(champs(m, &d.horaires));
    w.set_champs_promenade(champs(m, &d.promenade));
    w.set_champs_board(champs(m, &d.board));
    w.set_rappels(lignes_rappels(m));
    rafraichir(w, m, d);
}

/// Ouvre la fenetre. Rend le code de sortie du processus.
pub fn lancer(config: Option<PathBuf>, socket: Option<PathBuf>) -> i32 {
    let _verrou = match verrouiller(&dossier_verrou()) {
        Ok(Some(f)) => Some(f),
        // Une instance tourne deja pour cet utilisateur : on quitte aussitot.
        Ok(None) => return 0,
        Err(m) => {
            eprintln!("{m}");
            None
        }
    };
    let chemin = config.unwrap_or_else(osampo_config::chemin_par_defaut);
    let w = match Reglages::new() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("fenetre: {e}");
            return 1;
        }
    };
    w.set_chemin(chemin.display().to_string().into());
    let ed = match Edition::charger(&chemin) {
        Ok(ed) => ed,
        Err(m) => {
            w.set_erreur_lecture(m.into());
            w.on_fermer({
                let w = w.as_weak();
                move || {
                    let _ = w.upgrade().map(|w| w.hide());
                }
            });
            return sortie(w.run());
        }
    };
    let m = Rc::new(RefCell::new(Modele::new(ed)));
    let d = Rc::new(Disposition::new());
    remplir(&w, &m.borrow(), &d);
    {
        w.set_aide_categorie(aide_rappel(&bornes::RAPPEL_CATEGORIE));
        w.set_aide_heure(aide_rappel(&bornes::RAPPEL_HEURE));
        w.set_aide_jours(aide_rappel(&bornes::RAPPEL_JOURS));
    }
    w.on_valeurs_par_defaut({
        let (m, d, wf) = (m.clone(), d.clone(), w.as_weak());
        move || {
            if let Some(w) = wf.upgrade() {
                m.borrow_mut().valeurs_par_defaut();
                remplir(&w, &m.borrow(), &d);
                w.set_statut("Valeurs par défaut chargées : « Enregistrer » pour les garder.".into());
            }
        }
    });
    w.on_saisir({
        let (m, d, wf) = (m.clone(), d.clone(), w.as_weak());
        move |i, t| {
            if let Some(w) = wf.upgrade() {
                if (0..bornes::CLES.len() as i32).contains(&i) {
                    m.borrow_mut().saisir(i as usize, &t);
                }
                rafraichir(&w, &m.borrow(), &d);
            }
        }
    });
    w.on_saisir_rappel({
        let (m, d, wf) = (m.clone(), d.clone(), w.as_weak());
        move |n, col, t| {
            if let Some(w) = wf.upgrade() {
                m.borrow_mut().saisir_rappel(n as usize, col, &t);
                rafraichir(&w, &m.borrow(), &d);
            }
        }
    });
    // La case cochee se met a jour seule : `rappels` n'est pas reassigne, le focus reste.
    w.on_basculer_jour({
        let (m, d, wf) = (m.clone(), d.clone(), w.as_weak());
        move |n, k, coche| {
            let Some(w) = wf.upgrade() else { return };
            let texte = {
                let mm = m.borrow();
                let Some(r) = mm.rappels.get(n as usize) else { return };
                pages::basculer_jour(&r.jours, k as usize, coche)
            };
            m.borrow_mut().saisir_rappel(n as usize, 2, &texte);
            rafraichir(&w, &m.borrow(), &d);
        }
    });
    w.global::<Format>().on_heure(|t| pages::heure_normalisee(&t).into());
    // Recree la liste seulement si l'ordre ou un texte change : sinon le focus suivant reste.
    w.on_ranger_rappels({
        let (m, d, wf) = (m.clone(), d.clone(), w.as_weak());
        move || {
            if let Some(w) = wf.upgrade() {
                if m.borrow_mut().trier_rappels() {
                    w.set_rappels(lignes_rappels(&m.borrow()));
                }
                rafraichir(&w, &m.borrow(), &d);
            }
        }
    });
    w.on_ajouter_rappel({
        let (m, d, wf) = (m.clone(), d.clone(), w.as_weak());
        move || {
            if let Some(w) = wf.upgrade() {
                m.borrow_mut().ajouter_rappel();
                w.set_rappels(lignes_rappels(&m.borrow()));
                rafraichir(&w, &m.borrow(), &d);
            }
        }
    });
    w.on_supprimer_rappel({
        let (m, d, wf) = (m.clone(), d.clone(), w.as_weak());
        move |n| {
            if let Some(w) = wf.upgrade() {
                m.borrow_mut().supprimer_rappel(n as usize);
                w.set_rappels(lignes_rappels(&m.borrow()));
                rafraichir(&w, &m.borrow(), &d);
            }
        }
    });
    w.on_enregistrer({
        let (m, wf) = (m.clone(), w.as_weak());
        move || {
            if let Some(w) = wf.upgrade() {
                match m.borrow().enregistrer() {
                    Ok(()) => w.set_statut("Enregistré. Le chiot relit le fichier dans 2 s.".into()),
                    Err(e) => {
                        // « Enregistré » d'une fois precedente ne doit pas rester affiche
                        w.set_statut("".into());
                        w.set_erreur_globale(e.into());
                    }
                }
            }
        }
    });
    relier_jauge(&w, socket.unwrap_or_else(daemon::socket_par_defaut));
    w.on_fermer({
        let wf = w.as_weak();
        move || {
            let _ = wf.upgrade().map(|w| w.hide());
        }
    });
    sortie(w.run())
}

/// Cadre `Jauge sage` (SPEC-009) : suivi des lignes d'etat dans un fil a part, actions
/// envoyees aussitot au daemon, sans lien avec `Enregistrer`.
fn relier_jauge(w: &Reglages, socket: PathBuf) {
    w.set_jauge_texte(daemon::texte_etat(None).into());
    let wf = w.as_weak();
    daemon::suivre(socket.clone(), move |evt| {
        let (joignable, texte) = match evt {
            Evt::Sage(s) => (true, daemon::texte_etat(Some(s))),
            Evt::Injoignable => (false, daemon::texte_etat(None)),
        };
        wf.upgrade_in_event_loop(move |w| {
            w.set_jauge_joignable(joignable);
            w.set_jauge_texte(texte.into());
            if !joignable {
                w.set_jauge_erreur("".into());
            }
        })
        .is_ok()
    });
    let envoyer = {
        let wf = w.as_weak();
        move |valeur: u8| {
            let (wf, socket) = (wf.clone(), socket.clone());
            std::thread::spawn(move || {
                let erreur = match daemon::envoyer_jauge(&socket, valeur) {
                    Ok(()) => String::new(),
                    Err(e) => e.texte(),
                };
                let _ = wf.upgrade_in_event_loop(move |w| w.set_jauge_erreur(erreur.into()));
            });
        }
    };
    w.on_remplir_jauge({
        let envoyer = envoyer.clone();
        move || envoyer(100)
    });
    w.on_vider_jauge({
        let envoyer = envoyer.clone();
        move || envoyer(0)
    });
    w.on_fixer_jauge(move |t| {
        if let Some(v) = daemon::valeur_entiere(t.as_str()) {
            envoyer(v);
        }
    });
    w.on_saisir_jauge({
        let wf = w.as_weak();
        move |t| {
            if let Some(w) = wf.upgrade() {
                let valide = daemon::valeur_entiere(t.as_str()).is_some();
                w.set_jauge_champ_valide(valide);
                w.set_jauge_champ_marque(!t.is_empty() && !valide);
            }
        }
    });
}

fn sortie(r: Result<(), slint::PlatformError>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fenetre: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modele(t: &str) -> Modele {
        Modele::new(Edition::depuis_texte(t, Path::new("/tmp/x.toml")).unwrap())
    }

    fn idx(nom: &str) -> usize {
        bornes::CLES.iter().position(|c| c.nom() == nom).unwrap()
    }

    #[test]
    fn heure_sans_zero_completee_dans_le_fichier() {
        let mut m = modele(osampo_config::MODELE);
        m.saisir_rappel(0, 1, "8:00");
        assert_eq!(m.ed.rappels()[0].heure, "08:00");
        assert!(m.enregistrable(), "{:?}", m.erreurs());
        let i = idx("heures.calme_debut");
        m.saisir(i, "7:30");
        assert_eq!(m.ed.valeur(Modele::cle(i)), "\"07:30\"");
    }

    #[test]
    fn rappel_ajoute_trie_par_heure() {
        let mut m = modele(osampo_config::MODELE);
        let avant = m.rappels.len();
        m.ajouter_rappel();
        // Le nouveau rappel (12:00) prend tout de suite sa place, liste et fichier.
        assert_eq!(m.rappels.len(), avant + 1);
        let minutes: Vec<u32> = m.rappels.iter().map(|r| pages::minutes(&r.heure).unwrap()).collect();
        assert!(minutes.windows(2).all(|w| w[0] <= w[1]), "{minutes:?}");
        let fichier: Vec<String> = m.ed.rappels().iter().map(|r| r.heure.clone()).collect();
        assert_eq!(fichier, m.rappels.iter().map(|r| r.heure.clone()).collect::<Vec<_>>());
        assert!(!m.trier_rappels(), "deja trie a l'ajout");
        let n = m.rappels.iter().rposition(|r| r.heure == "12:00").unwrap();
        m.saisir_rappel(n, 1, "8:00");
        assert!(m.trier_rappels());
        assert_eq!(m.rappels[0].heure, "08:00");
        assert_eq!(m.ed.rappels()[0].heure, "08:00");
        assert!(!m.trier_rappels(), "deja trie : rien ne change");
        // Une heure invalide passe en fin de liste, sans etre perdue.
        m.saisir_rappel(0, 1, "99:00");
        assert!(m.trier_rappels());
        assert_eq!(m.rappels.last().unwrap().heure, "99:00");
    }

    #[test]
    fn rappels_tries_a_l_ouverture() {
        let t = osampo_config::MODELE.to_string()
            + "\n[[rappel]]\ncategorie = \"sport\"\nheure = \"07:15\"\njours = [\"lun\"]\n";
        let m = modele(&t);
        assert_eq!(m.rappels[0].heure, "07:15");
        assert_eq!(m.ed.rappels()[0].heure, "07:15");
    }

    #[test]
    fn valeurs_par_defaut_remet_tout_sans_ecrire() {
        let t = osampo_config::MODELE.replace("chien_px = 64", "chien_px = 77");
        let mut m = modele(&t);
        let px = idx("pc.chien_px");
        assert_eq!(m.champs[px], "77");
        let gain = idx("besoins.gain");
        m.saisir(gain, "abc");
        m.saisir_rappel(0, 1, "07:00");
        m.ajouter_rappel();
        assert!(!m.enregistrable());
        m.valeurs_par_defaut();
        assert_eq!(m.champs[px], "64");
        assert_eq!(m.champs[gain], "20");
        assert_eq!(m.rappels.len(), 5);
        assert_eq!(m.rappels[0].heure, "12:00");
        assert!(m.enregistrable(), "{:?}", m.erreurs());
        assert_eq!(m.ed.chemin(), Path::new("/tmp/x.toml"), "meme fichier a l'enregistrement");
    }

    #[test]
    fn disposition_affiche_chaque_cle_une_fois() {
        let d = Disposition::new();
        let mut vus: Vec<usize> = d.rappels_fins.clone();
        vus.extend(d.plages.iter().flat_map(|p| [p.debut, p.fin]));
        vus.extend(&d.horaires);
        vus.extend(&d.promenade);
        vus.extend(&d.board);
        vus.sort();
        assert_eq!(vus, (0..bornes::CLES.len()).collect::<Vec<_>>());
    }

    #[test]
    fn aide_rappel_porte_libelle_et_phrase() {
        let t = aide_rappel(&bornes::RAPPEL_HEURE);
        assert_eq!(t.titre, "Heure du rappel");
        assert!(t.texte.starts_with("Heure (HH:MM)"), "{}", t.texte);
    }

    #[test]
    fn affichage_des_valeurs_du_modele() {
        let m = modele(osampo_config::MODELE);
        assert_eq!(m.champs[idx("heures.calme_debut")], "22:00");
        assert_eq!(m.champs[idx("rappels.relances_s")], "60, 180, 420");
        assert_eq!(m.champs[idx("clavier.lancer")], "t");
        assert_eq!(m.rappels.len(), 5);
        assert!(m.enregistrable());
    }

    #[test]
    fn champ_hors_bornes_marque_avec_le_message_du_daemon() {
        let mut m = modele(osampo_config::MODELE);
        m.saisir(idx("rappels.report_min"), "200");
        let e = m.erreurs();
        assert_eq!(e.champs[idx("rappels.report_min")], "rappels.report_min: 200 hors de 1..120");
        assert!(!m.enregistrable());
        assert!(m.enregistrer().is_err());
        m.saisir(idx("rappels.report_min"), "15");
        assert!(m.enregistrable());
    }

    #[test]
    fn saisie_illisible_bloque_sans_toucher_le_document() {
        let mut m = modele(osampo_config::MODELE);
        m.saisir(idx("rappels.report_min"), "abc");
        assert!(m.erreurs().champs[idx("rappels.report_min")].contains("rappels.report_min"));
        assert!(!m.enregistrable());
        m.saisir(idx("rappels.report_min"), "11");
        assert!(m.enregistrable());
    }

    #[test]
    fn heure_touche_et_liste_sont_requotees() {
        let mut m = modele(osampo_config::MODELE);
        m.saisir(idx("heures.calme_debut"), "23:30");
        m.saisir(idx("rappels.relances_s"), "30, 90");
        m.saisir(idx("clavier.lancer"), "x");
        assert!(m.enregistrable());
        let t = m.ed.texte();
        assert!(t.contains("calme_debut = \"23:30\""), "{t}");
        assert!(t.contains("relances_s = [30, 90]"), "{t}");
        m.saisir(idx("heures.calme_debut"), "25:00");
        assert!(m.erreurs().champs[idx("heures.calme_debut")].starts_with("heures.calme_debut:"));
    }

    #[test]
    fn regle_croisee_marquee_sur_la_cle_du_daemon() {
        let mut m = modele(osampo_config::MODELE);
        m.saisir(idx("promenade.longue_min"), "3");
        assert!(!m.enregistrable());
        let e = m.erreurs();
        assert!(e.champs.iter().any(|c| c.starts_with("promenade.")) || !e.globale.is_empty());
    }

    #[test]
    fn rappels_ajout_edition_suppression() {
        let mut m = modele(osampo_config::MODELE);
        m.ajouter_rappel();
        assert_eq!(m.rappels.len(), 6);
        m.saisir_rappel(5, 1, "25:61");
        let e = m.erreurs();
        assert!(e.rappels[5].starts_with("rappel 6:"), "{e:?}");
        assert!(!m.enregistrable());
        m.saisir_rappel(5, 1, "08:30");
        m.saisir_rappel(5, 2, "lun, mar");
        assert!(m.enregistrable());
        m.saisir_rappel(5, 2, "");
        assert!(m.erreurs().rappels[5].contains("jours vide"));
        m.supprimer_rappel(5);
        assert_eq!(m.rappels.len(), 5);
        assert!(m.enregistrable());
        while m.rappels.len() < 16 {
            m.ajouter_rappel();
        }
        m.ajouter_rappel();
        assert_eq!(m.rappels.len(), 16);
    }

    #[test]
    fn enregistrement_ecrit_le_fichier() {
        let d = std::env::temp_dir().join(format!("osampo-gui-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("t.toml");
        std::fs::write(&p, "# a moi\n[rappels]\nreport_min = 10 # dix\n").unwrap();
        let mut m = Modele::new(Edition::charger(&p).unwrap());
        m.saisir(idx("rappels.report_min"), "12");
        m.enregistrer().unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "# a moi\n[rappels]\nreport_min = 12 # dix\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_seul_verrou_par_dossier() {
        let d = std::env::temp_dir().join(format!("osampo-verrou-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let premier = verrouiller(&d).unwrap();
        assert!(premier.is_some());
        assert!(verrouiller(&d).unwrap().is_none(), "second lancement doit etre refuse");
        drop(premier);
        assert!(verrouiller(&d).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&d);
    }
}
