//! Rangement des cles de `bornes::CLES` dans les 4 onglets de la fenetre (design
//! 2026-11-22-reglages-ux) : Rappels, Promenade, Horaires, Board. Sans Slint, donc testable.
//! Les onglets `temps` / `avance` de `bornes.rs` (utilises par `--lister`) ne changent pas :
//! Board ne porte que des cles `Avance` ; les autres pages des cles `Temps`, plus les besoins
//! (`avance`) rangees dans Promenade a la demande de l'auteur.

use osampo_config::bornes::{Cle, Genre, Onglet, CLES};

use crate::gui::Erreurs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Rappels,
    Horaires,
    Promenade,
    Board,
}

/// Pages dans l'ordre des onglets.
pub const PAGES: [Page; 4] = [Page::Rappels, Page::Promenade, Page::Horaires, Page::Board];

impl Page {
    pub fn titre(self) -> &'static str {
        match self {
            Page::Rappels => "Rappels",
            Page::Horaires => "Horaires",
            Page::Promenade => "Promenade",
            Page::Board => "Board",
        }
    }
}

/// Page et bloc (sous-titre, "" si aucun) d'une cle.
pub fn place(c: &Cle) -> (Page, &'static str) {
    match (c.onglet, c.section, c.cle) {
        (Onglet::Temps, "rappels", _) | (Onglet::Temps, "chiot", "patte_avant_aboie_min") => {
            (Page::Rappels, "Réglages fins")
        }
        (Onglet::Temps, "heures", _) => (Page::Horaires, ""),
        (Onglet::Temps, "promenade", _) | (Onglet::Temps, "chiot", "calme_apres_validation_min") => {
            (Page::Promenade, "")
        }
        (Onglet::Temps, _, _) => (Page::Promenade, "Autres"),
        (Onglet::Avance, "besoins", _) => (Page::Promenade, "Besoins"),
        (Onglet::Avance, s, _) => (
            Page::Board,
            match s {
                "ecran" => "Écran",
                "besoins" => "Besoins",
                "gestes" => "Gestes",
                "clavier" => "Clavier",
                "pc" => "PC",
                _ => "Autres",
            },
        ),
    }
}

/// Indices dans `CLES` des cles d'une page, dans l'ordre de `CLES`.
pub fn indices(p: Page) -> Vec<usize> {
    (0..CLES.len()).filter(|&i| place(&CLES[i]).0 == p).collect()
}

/// Plage horaire affichee sur une ligne : `<libelle> de [debut] a [fin]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Plage {
    pub libelle: &'static str,
    pub note: String,
    /// Indices dans `CLES`.
    pub debut: usize,
    pub fin: usize,
}

/// Plages de la page Horaires : paires `<x>_debut` / `<x>_fin` de genre `Heure`, meme section.
pub fn plages() -> Vec<Plage> {
    let mut v = Vec::new();
    for (debut, c) in CLES.iter().enumerate() {
        let Some(prefixe) = c.cle.strip_suffix("_debut") else { continue };
        if c.genre != Genre::Heure || place(c).0 != Page::Horaires {
            continue;
        }
        let fin_nom = format!("{}.{prefixe}_fin", c.section);
        let Some(fin) = CLES.iter().position(|k| k.nom() == fin_nom && k.genre == Genre::Heure) else {
            continue;
        };
        let libelle = match prefixe {
            "calme" => "Heures calmes",
            "nuit" => "Nuit de l'écran",
            _ => c.libelle,
        };
        let note = c.bornes.split_once("; ").map(|(_, n)| majuscule(n)).unwrap_or_default();
        v.push(Plage { libelle, note, debut, fin });
    }
    v
}

fn majuscule(t: &str) -> String {
    let mut it = t.chars();
    it.next().map(|p| p.to_uppercase().chain(it).collect()).unwrap_or_default()
}

/// Bornes affichees a cote du champ ; le texte complet de `c.bornes` va dans le bandeau d'aide.
pub fn bornes_courtes(c: &Cle) -> &'static str {
    match c.genre {
        Genre::Heure => "",
        Genre::Touche => "1 caractère",
        _ => c.bornes,
    }
}

/// `H:MM` -> `0H:MM` (espaces retires) ; tout autre texte est rendu tel quel (sans espaces).
pub fn heure_normalisee(t: &str) -> String {
    let t = t.trim();
    match t.split_once(':') {
        Some((h, m)) if h.len() == 1 && h.chars().all(|c| c.is_ascii_digit()) && !m.is_empty() => {
            format!("0{t}")
        }
        _ => t.to_string(),
    }
}

/// Minutes depuis minuit d'une heure `H:MM` ou `HH:MM` valide (0..24, 0..60), sinon `None`.
pub fn minutes(t: &str) -> Option<u32> {
    let (h, m) = heure_normalisee(t).split_once(':').map(|(h, m)| (h.to_string(), m.to_string()))?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub const JOURS: [&str; 7] = ["lun", "mar", "mer", "jeu", "ven", "sam", "dim"];

/// Texte des jours (separes par des espaces ou des virgules, casse ignoree) -> cases lun..dim.
pub fn cases(jours: &str) -> [bool; 7] {
    let mut c = [false; 7];
    for j in jours.split([' ', ',']).filter(|j| !j.is_empty()) {
        if let Some(i) = JOURS.iter().position(|&n| n.eq_ignore_ascii_case(j)) {
            c[i] = true;
        }
    }
    c
}

/// Cases lun..dim -> texte des jours, separes par un espace.
pub fn jours(cases: &[bool; 7]) -> String {
    JOURS.iter().zip(cases).filter(|(_, &c)| c).map(|(j, _)| *j).collect::<Vec<_>>().join(" ")
}

/// Case `k` (lun..dim) cochee ou non dans le texte des jours. Les mots non reconnus
/// (faute de frappe dans le fichier) sont gardes en fin, pour que la validation les signale
/// au lieu qu'une case les efface en silence.
pub fn basculer_jour(jours_texte: &str, k: usize, coche: bool) -> String {
    let mut c = cases(jours_texte);
    if let Some(x) = c.get_mut(k) {
        *x = coche;
    }
    let inconnus = jours_texte
        .split([' ', ','])
        .filter(|j| !j.is_empty() && !JOURS.iter().any(|n| n.eq_ignore_ascii_case(j)));
    std::iter::once(jours(&c)).chain(inconnus.map(str::to_string)).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Pages qui contiennent une erreur, dans l'ordre de `PAGES` (une erreur de rappel -> Rappels).
pub fn pages_en_erreur(e: &Erreurs) -> [bool; 4] {
    let mut r = PAGES.map(|p| indices(p).iter().any(|&i| e.champs.get(i).is_some_and(|m| !m.is_empty())));
    r[0] |= e.rappels.iter().any(|m| !m.is_empty());
    r
}

/// Statut quand `Enregistrer` est grise ; "" si aucune erreur de champ ni de rappel.
pub fn texte_blocage(e: &Erreurs) -> String {
    let titres: Vec<&str> =
        PAGES.iter().zip(pages_en_erreur(e)).filter(|(_, x)| *x).map(|(p, _)| p.titre()).collect();
    match titres.as_slice() {
        [] => String::new(),
        [t] => format!("Corrigez le champ en rouge dans l'onglet {t}."),
        _ => format!("Corrigez les champs en rouge dans les onglets {}.", titres.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edition::Edition;
    use crate::gui::Modele;
    use std::path::Path;

    fn idx(nom: &str) -> usize {
        CLES.iter().position(|c| c.nom() == nom).unwrap()
    }

    #[test]
    fn toute_cle_a_une_page_conforme_a_son_onglet() {
        // Seule exception : les besoins (cles `avance`) sont rangees dans Promenade.
        for c in CLES.iter().filter(|c| c.section != "besoins") {
            assert_eq!(place(c).0 == Page::Board, c.onglet == Onglet::Avance, "{}", c.nom());
        }
        let total: usize = PAGES.iter().map(|&p| indices(p).len()).sum();
        assert_eq!(total, CLES.len());
    }

    #[test]
    fn placements_attendus() {
        assert_eq!(place(&CLES[idx("rappels.relances_s")]), (Page::Rappels, "Réglages fins"));
        assert_eq!(place(&CLES[idx("heures.nuit_fin")]), (Page::Horaires, ""));
        assert_eq!(place(&CLES[idx("chiot.calme_apres_validation_min")]), (Page::Promenade, ""));
        assert_eq!(place(&CLES[idx("gestes.lancer_g")]), (Page::Board, "Gestes"));
        assert_eq!(place(&CLES[idx("clavier.lancer")]), (Page::Board, "Clavier"));
        assert_eq!(place(&CLES[idx("ecran.lum_jour")]), (Page::Board, "Écran"));
    }

    #[test]
    fn cle_inconnue_va_dans_la_page_par_defaut() {
        let t = Cle { section: "zzz", cle: "x", onglet: Onglet::Temps, ..CLES[0] };
        assert_eq!(place(&t), (Page::Promenade, "Autres"));
        let a = Cle { onglet: Onglet::Avance, ..t };
        assert_eq!(place(&a), (Page::Board, "Autres"));
        let pc = Cle { section: "pc", cle: "chien_px", onglet: Onglet::Avance, ..CLES[0] };
        assert_eq!(place(&pc), (Page::Board, "PC"));
        let patte = Cle { section: "chiot", cle: "patte_avant_aboie_min", ..CLES[0] };
        assert_eq!(place(&patte), (Page::Rappels, "Réglages fins"));
    }

    #[test]
    fn onglets_rappels_promenade_horaires_board() {
        assert_eq!(PAGES, [Page::Rappels, Page::Promenade, Page::Horaires, Page::Board]);
    }

    #[test]
    fn besoins_dans_promenade() {
        assert_eq!(place(&CLES[idx("besoins.gain")]), (Page::Promenade, "Besoins"));
        assert_eq!(place(&CLES[idx("besoins.baisse_par_jour")]), (Page::Promenade, "Besoins"));
    }

    #[test]
    fn heure_completee() {
        assert_eq!(heure_normalisee("8:00"), "08:00");
        assert_eq!(heure_normalisee(" 7:05 "), "07:05");
        assert_eq!(heure_normalisee("08:00"), "08:00");
        assert_eq!(heure_normalisee("21:45"), "21:45");
        // Ce qui n'est pas H:MM reste tel quel : la validation le signale.
        assert_eq!(heure_normalisee("8"), "8");
        assert_eq!(heure_normalisee("ab:cd"), "ab:cd");
        assert_eq!(heure_normalisee(""), "");
    }

    #[test]
    fn minutes_d_une_heure() {
        assert_eq!(minutes("08:00"), Some(480));
        assert_eq!(minutes("8:05"), Some(485));
        assert_eq!(minutes("25:00"), None);
        assert_eq!(minutes("x"), None);
    }

    #[test]
    fn deux_plages_calme_puis_nuit() {
        let p = plages();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].libelle, "Heures calmes");
        assert_eq!(p[0].note, "Début = fin : aucune heure calme");
        assert_eq!((p[0].debut, p[0].fin), (idx("heures.calme_debut"), idx("heures.calme_fin")));
        assert_eq!(p[1].libelle, "Nuit de l'écran");
        assert_eq!(p[1].note, "Début = fin : jamais la nuit");
        assert_eq!((p[1].debut, p[1].fin), (idx("heures.nuit_debut"), idx("heures.nuit_fin")));
    }

    #[test]
    fn bornes_courtes_par_genre() {
        assert_eq!(bornes_courtes(&CLES[idx("heures.calme_debut")]), "");
        assert_eq!(bornes_courtes(&CLES[idx("clavier.lancer")]), "1 caractère");
        assert_eq!(bornes_courtes(&CLES[idx("rappels.report_min")]), "1 à 120 min");
        assert_eq!(bornes_courtes(&CLES[idx("besoins.baisse_par_jour")]), "0 à 100 points par jour");
        assert_eq!(bornes_courtes(&CLES[idx("besoins.gain")]), "0 à 100 points par soin");
    }

    #[test]
    fn jours_aller_retour() {
        let c = cases("lun, MER dim");
        assert_eq!(c, [true, false, true, false, false, false, true]);
        assert_eq!(jours(&c), "lun mer dim");
        assert_eq!(cases(""), [false; 7]);
    }

    #[test]
    fn basculer_jour_garde_les_mots_inconnus() {
        assert_eq!(basculer_jour("lun mardi", 2, true), "lun mer mardi");
        assert_eq!(basculer_jour("lun, MER", 0, false), "mer");
        assert_eq!(basculer_jour("mardi", 1, false), "mardi");
        assert_eq!(basculer_jour("", 6, true), "dim");
    }

    #[test]
    fn texte_blocage_nomme_la_page() {
        let mut e = Erreurs { champs: vec![String::new(); CLES.len()], ..Default::default() };
        assert_eq!(texte_blocage(&e), "");
        e.champs[idx("gestes.lancer_g")] = "gestes.lancer_g: 99 hors de 0.5..16".into();
        assert_eq!(pages_en_erreur(&e), [false, false, false, true]);
        assert_eq!(texte_blocage(&e), "Corrigez le champ en rouge dans l'onglet Board.");
        e.rappels = vec!["rappel 1: jours vide".into()];
        assert_eq!(pages_en_erreur(&e), [true, false, false, true]);
        assert_eq!(texte_blocage(&e), "Corrigez les champs en rouge dans les onglets Rappels, Board.");
        assert_eq!(texte_blocage(&Erreurs::default()), "");
    }

    #[test]
    fn aucun_jour_coche_bloque() {
        let ed = Edition::depuis_texte(osampo_config::MODELE, Path::new("/tmp/x.toml")).unwrap();
        let mut m = Modele::new(ed);
        m.saisir_rappel(0, 2, &jours(&[false; 7]));
        assert!(!m.erreurs().rappels[0].is_empty());
        assert!(!m.enregistrable());
    }
}
