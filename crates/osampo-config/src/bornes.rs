//! Table des cles de osampo.toml affichees par `osampo-reglages` (SPEC-007) : onglet,
//! cle, texte des bornes et defaut. Le test en bas verifie les bornes contre `lire` : la table
//! ne peut pas deriver des regles du daemon. La liste `[[rappel]]` n'est pas une cle scalaire,
//! elle n'est pas dans la table.

/// Onglet du programme de reglages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Onglet {
    Temps,
    Avance,
}

impl Onglet {
    /// Nom utilise par `--lister` : `temps` ou `avance`.
    pub fn nom(self) -> &'static str {
        match self {
            Onglet::Temps => "temps",
            Onglet::Avance => "avance",
        }
    }
}

/// Forme de la valeur et bornes acceptees (utilisees par le test de coherence avec `lire`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Genre {
    /// Entier dans `min..=max`.
    Entier { min: i64, max: i64 },
    /// 0 ou un entier dans `min..=max` (`ecran.batterie_faible_mv`).
    ZeroOuEntier { min: i64, max: i64 },
    /// Flottant en g dans `min..=max`.
    Flottant { min: f64, max: f64 },
    /// 1 a 3 entiers de `min` a `max`, strictement croissants.
    Liste { min: i64, max: i64 },
    /// `HH:MM`.
    Heure,
    /// Un caractere ASCII imprimable hors espace.
    Touche,
}

#[derive(Clone, Copy, Debug)]
pub struct Cle {
    pub onglet: Onglet,
    pub section: &'static str,
    pub cle: &'static str,
    pub genre: Genre,
    /// Texte des bornes affiche a cote du champ.
    pub bornes: &'static str,
    /// Defaut, en syntaxe de valeur TOML.
    pub defaut: &'static str,
    /// Libelle francais affiche a la place du nom `section.cle` (SPEC-010).
    pub libelle: &'static str,
    /// Phrase d'aide : role, unite, effet d'une valeur haute ou basse (SPEC-010).
    pub aide: &'static str,
    /// Cle voisine (`cle = valeur` TOML, meme section) a poser pour tester les bornes
    /// de cette cle sans heurter une regle croisee ; vide si inutile.
    pub voisine: &'static str,
}

impl Cle {
    /// Nom complet `section.cle`.
    pub fn nom(&self) -> String {
        format!("{}.{}", self.section, self.cle)
    }
}

const fn c(
    onglet: Onglet,
    section: &'static str,
    cle: &'static str,
    genre: Genre,
    bornes: &'static str,
    defaut: &'static str,
    libelle: &'static str,
    aide: &'static str,
) -> Cle {
    Cle { onglet, section, cle, genre, bornes, defaut, libelle, aide, voisine: "" }
}

use Genre::*;
use Onglet::{Avance, Temps};

const fn entier(min: i64, max: i64) -> Genre {
    Entier { min, max }
}

/// Toutes les cles scalaires que le daemon accepte, dans l'ordre d'affichage.
pub const CLES: &[Cle] = &[
    c(Temps, "rappels", "report_min", entier(1, 120), "1 à 120 min", "10", "Durée d'un report", "Un rappel reporté revient au bout de ce nombre de minutes ; plus haut, il revient plus tard, plus bas, il revient plus vite."),
    c(Temps, "rappels", "rattrapage_min", entier(1, 720), "1 à 720 min", "60", "Délai de rattrapage", "Un rappel qui n'a pas pu sonner à son heure sonne encore pendant ce nombre de minutes ; plus haut, il est rattrapé plus longtemps, plus bas, il est vite abandonné."),
    c(
        Temps,
        "rappels",
        "relances_s",
        Liste { min: 10, max: 3600 },
        "1 à 3 valeurs de 10 à 3600 s, strictement croissantes",
        "[60, 180, 420]",
        "Relances d'un rappel",
        "Tant qu'un rappel n'est pas pris en compte, le chiot insiste après chacun de ces délais, en secondes ; des valeurs hautes espacent les relances, des valeurs basses le rendent plus pressant.",
    ),
    c(Temps, "chiot", "calme_apres_validation_min", entier(1, 120), "1 à 120 min", "10", "Temps de calme après une validation", "Après un rappel validé, le chiot attend ce nombre de minutes avant de pouvoir réclamer la promenade ; plus haut, il reste tranquille plus longtemps, plus bas, il peut réclamer presque aussitôt."),
    c(Temps, "chiot", "patte_avant_aboie_min", entier(1, 60), "1 à 60 min", "2", "Patte levée avant d'aboyer", "Quand Claude attend une réponse, le chiot lève d'abord la patte, puis aboie s'il est ignoré pendant ce nombre de minutes ; plus haut, il patiente plus longtemps en silence, plus bas, il aboie presque aussitôt."),
    c(
        Temps,
        "heures",
        "calme_debut",
        Heure,
        "HH:MM, 00:00 à 23:59 ; début = fin : aucune heure calme",
        "\"22:00\"",
        "Début des heures calmes",
        "Heure (HH:MM) à partir de laquelle le chiot et le PC se taisent et aucun rappel ne sonne ; plus tôt, le calme commence plus tôt ; égale à la fin, il n'y a pas d'heures calmes.",
    ),
    c(
        Temps,
        "heures",
        "calme_fin",
        Heure,
        "HH:MM, 00:00 à 23:59 ; début = fin : aucune heure calme",
        "\"07:00\"",
        "Fin des heures calmes",
        "Heure (HH:MM) à laquelle les sons et les rappels reprennent ; plus tard, le calme dure plus longtemps ; égale au début, il n'y a pas d'heures calmes.",
    ),
    c(
        Temps,
        "heures",
        "nuit_debut",
        Heure,
        "HH:MM, 00:00 à 23:59 ; début = fin : jamais la nuit",
        "\"20:00\"",
        "Début de la nuit",
        "Heure (HH:MM) à partir de laquelle l'écran de la board passe à la luminosité de nuit ; plus tôt, la nuit commence plus tôt ; égale à la fin, il ne fait jamais nuit.",
    ),
    c(
        Temps,
        "heures",
        "nuit_fin",
        Heure,
        "HH:MM, 00:00 à 23:59 ; début = fin : jamais la nuit",
        "\"07:00\"",
        "Fin de la nuit",
        "Heure (HH:MM) à laquelle l'écran repasse à la luminosité de jour ; plus tard, la nuit dure plus longtemps ; égale au début, il ne fait jamais nuit.",
    ),
    c(Temps, "promenade", "vide_apres_min", entier(1, 600), "1 à 600 min", "90", "Temps pour vider la jauge", "Minutes passées au clavier et à la souris qui vident la jauge sage, pleine au départ ; vide, le chiot réclame la promenade ; plus haut, la jauge baisse plus lentement, plus bas, il réclame plus souvent."),
    Cle {
        voisine: "longue_min = 240",
        ..c(Temps, "promenade", "courte_min", entier(1, 120), "1 à 120 min", "5", "Remplissage de la jauge : promenade courte", "Absence du PC, en minutes, à partir de laquelle la jauge commence à se recharger ; plus haut, une courte pause ne compte plus comme promenade, plus bas, la moindre pause recharge un peu.")
    },
    Cle {
        voisine: "courte_min = 1",
        ..c(
            Temps,
            "promenade",
            "longue_min",
            entier(2, 240),
            "2 à 240 min, plus que courte_min",
            "30",
        "Remplissage de la jauge : promenade longue",
        "Absence du PC, en minutes, qui recharge la jauge entièrement ; plus haut, il faut une plus longue absence pour la remplir, plus bas, elle se remplit vite.",
        )
    },
    c(Temps, "promenade", "seuil_reclame", entier(0, 100), "0 à 100", "30", "Redemande si la jauge reste sous", "Niveau de la jauge, de 0 à 100, sous lequel le chiot redemande la promenade 10 minutes après la fin d'une promenade ; plus haut, il redemande plus souvent, plus bas, une petite promenade lui suffit."),
    c(Temps, "promenade", "retour_min", entier(1, 60), "1 à 60 min", "2", "Fin d'une promenade oubliée", "Minutes d'activité au PC qui terminent une promenade oubliée sur la board ; plus haut, la promenade reste ouverte plus longtemps, plus bas, elle se termine dès le retour au PC."),
    c(Avance, "ecran", "lum_jour", entier(5, 100), "5 à 100 %", "100", "Luminosité de jour", "Luminosité de l'écran de la board le jour, en pourcent ; plus haut, l'écran est plus lumineux, plus bas, il est plus sombre et use moins la batterie."),
    c(Avance, "ecran", "lum_nuit", entier(5, 100), "5 à 100 %", "20", "Luminosité de nuit", "Luminosité de l'écran de la board la nuit, en pourcent ; plus haut, l'écran éclaire davantage la pièce, plus bas, il gêne moins dans le noir."),
    c(
        Avance,
        "ecran",
        "batterie_faible_mv",
        ZeroOuEntier { min: 3000, max: 4200 },
        "0 (jamais) ou 3000 à 4200 mV",
        "0",
        "Seuil de batterie faible",
        "Tension de la batterie, en millivolts, sous laquelle l'icône de batterie faible s'affiche (0 : jamais) ; plus haut, l'alerte vient plus tôt, plus bas, elle vient plus près de la panne.",
    ),
    c(Avance, "besoins", "baisse_par_jour", entier(0, 100), "0 à 100 points par jour", "15", "Baisse des besoins", "Points perdus chaque jour par les besoins du chiot (bouger, jouer, dormir), de 0 à 100 ; plus haut, il faut s'en occuper plus souvent, plus bas, ses besoins restent longtemps remplis."),
    c(Avance, "besoins", "gain", entier(0, 100), "0 à 100 points par soin", "20", "Gain d'un soin", "Points rendus à un besoin par chaque soin donné, de 0 à 100 ; plus haut, quelques soins suffisent, plus bas, il en faut beaucoup."),
    c(Avance, "gestes", "lancer_g", Flottant { min: 0.5, max: 16.0 }, "0.5 à 16 g", "2.0", "Force d'un lancer", "Accélération, en g, qui déclenche un lancer de la board ; plus haut, il faut lancer plus fort, plus bas, un petit mouvement suffit et les fausses détections augmentent."),
    c(Avance, "gestes", "secousse_g", Flottant { min: 0.5, max: 16.0 }, "0.5 à 16 g", "2.0", "Force d'une secousse", "Accélération, en g, qui compte comme un coup de secousse ; plus haut, il faut secouer plus fort, plus bas, une légère secousse suffit."),
    c(Avance, "gestes", "lancer_dps", entier(50, 4000), "50 à 4000 °/s", "300", "Rotation d'un lancer", "Vitesse de rotation, en degrés par seconde, qui déclenche un lancer ; plus haut, il faut faire tourner la board plus vite, plus bas, un lancer est détecté plus facilement."),
    c(Avance, "gestes", "lancer_ms", entier(10, 1000), "10 à 1000 ms", "20", "Durée d'un lancer", "Durée minimale du mouvement, en millisecondes, pour compter un lancer ; plus haut, les chocs brefs sont ignorés, plus bas, un lancer très court est compté."),
    c(Avance, "gestes", "secousse_coups", entier(2, 20), "2 à 20", "4", "Coups d'une secousse", "Nombre de coups à réunir pour une secousse ; plus haut, il faut secouer plus longtemps, plus bas, deux ou trois coups suffisent."),
    c(Avance, "gestes", "secousse_fenetre_ms", entier(200, 5000), "200 à 5000 ms", "1000", "Fenêtre d'une secousse", "Temps, en millisecondes, pour réunir ces coups ; plus haut, on peut secouer plus lentement, plus bas, il faut secouer vite."),
    c(Avance, "gestes", "retourne_ms", entier(200, 10000), "200 à 10000 ms", "1500", "Durée d'un retournement", "Temps, en millisecondes, pendant lequel la board doit rester retournée pour compter le geste ; plus haut, il faut la laisser plus longtemps, plus bas, un retournement bref suffit."),
    c(Avance, "gestes", "pause_gestes_ms", entier(0, 10000), "0 à 10000 ms", "1500", "Pause entre deux gestes", "Temps, en millisecondes, pendant lequel un nouveau geste est ignoré après le précédent ; plus haut, les gestes s'enchaînent moins, plus bas, deux gestes rapprochés comptent tous les deux."),
    c(Avance, "gestes", "journal", entier(0, 2), "0, 1 ou 2", "0", "Journal des gestes", "Détail du journal des gestes de la board : 0 muet, 1 une ligne par salve, 2 salve et échantillons bruts ; plus haut, le journal est plus bavard."),
    c(
        Avance,
        "clavier",
        "lancer",
        Touche,
        "un caractère ASCII imprimable hors espace, les trois distincts",
        "\"t\"",
        "Touche du lancer",
        "Touche, un caractère, qui simule un lancer dans le chiot du desktop ; elle doit différer des deux autres touches.",
    ),
    c(
        Avance,
        "clavier",
        "secousse",
        Touche,
        "un caractère ASCII imprimable hors espace, les trois distincts",
        "\"s\"",
        "Touche de la secousse",
        "Touche, un caractère, qui simule une secousse dans le chiot du desktop ; elle doit différer des deux autres touches.",
    ),
    c(
        Avance,
        "clavier",
        "retourner",
        Touche,
        "un caractère ASCII imprimable hors espace, les trois distincts",
        "\"f\"",
        "Touche du retournement",
        "Touche, un caractère, qui simule un retournement dans le chiot du desktop ; elle doit différer des deux autres touches.",
    ),
    c(
        Avance,
        "pc",
        "chien_px",
        entier(32, 256),
        "32 à 256 px",
        "64",
        "Taille du chien sur le bureau",
        "Hauteur et largeur, en pixels, du chien posé au-dessus de la barre des tâches du PC ; plus haut, il est plus grand et cache plus de bureau, plus bas, il est plus discret.",
    ),
];

/// Libelle et aide d'un champ des rappels `[[rappel]]` (SPEC-010).
#[derive(Clone, Copy, Debug)]
pub struct TexteRappel {
    pub libelle: &'static str,
    pub aide: &'static str,
}

/// Textes de `rappel.categorie`.
pub const RAPPEL_CATEGORIE: TexteRappel = TexteRappel {
    libelle: "Sorte de rappel",
    aide: "Ce que le chiot réclame à l'heure dite : repas se valide d'un toucher, sortie, sport et dodo réclament la promenade (dodo : celle du soir, puis il dort).",
};

/// Textes de `rappel.heure`.
pub const RAPPEL_HEURE: TexteRappel = TexteRappel {
    libelle: "Heure du rappel",
    aide: "Heure (HH:MM) à laquelle le rappel sonne ; plus tôt ou plus tard, il sonne plus tôt ou plus tard dans la journée.",
};

/// Textes de `rappel.jours`.
pub const RAPPEL_JOURS: TexteRappel = TexteRappel {
    libelle: "Jours du rappel",
    aide: "Jours de la semaine où le rappel sonne, au moins un parmi lun..dim ; moins de jours, il sonne moins souvent.",
};

/// Cherche une cle par son nom complet `section.cle`.
pub fn trouver(nom: &str) -> Option<&'static Cle> {
    CLES.iter().find(|k| k.nom() == nom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lire, Reglages};

    /// Fichier TOML minimal qui pose `cle = valeur` (et la voisine de la table).
    fn fichier(k: &Cle, valeur: &str) -> String {
        format!("[{}]\n{} = {}\n{}\n", k.section, k.cle, valeur, k.voisine)
    }

    fn accepte(k: &Cle, v: &str) -> bool {
        lire(&fichier(k, v)).is_ok()
    }

    #[test]
    fn noms_uniques() {
        for (i, a) in CLES.iter().enumerate() {
            assert!(CLES[i + 1..].iter().all(|b| b.nom() != a.nom()), "{}", a.nom());
        }
        assert_eq!(CLES.len(), 32);
    }

    #[test]
    fn defauts_acceptes_et_egaux_aux_defauts_du_daemon() {
        for k in CLES {
            let r = lire(&fichier(k, k.defaut)).unwrap_or_else(|e| panic!("{}: {e}", k.nom()));
            // La voisine (min/max d'une regle croisee) n'est pas un defaut : on la saute.
            if k.voisine.is_empty() {
                assert_eq!(r, Reglages::default(), "{}", k.nom());
            }
        }
    }

    #[test]
    fn bornes_de_la_table_egales_a_celles_de_lire() {
        for k in CLES {
            let n = k.nom();
            match k.genre {
                Entier { min, max } => {
                    assert!(accepte(k, &min.to_string()), "{n} min");
                    assert!(accepte(k, &max.to_string()), "{n} max");
                    assert!(!accepte(k, &(min - 1).to_string()), "{n} min-1");
                    assert!(!accepte(k, &(max + 1).to_string()), "{n} max+1");
                }
                ZeroOuEntier { min, max } => {
                    assert!(accepte(k, "0"), "{n} 0");
                    assert!(accepte(k, &min.to_string()), "{n} min");
                    assert!(accepte(k, &max.to_string()), "{n} max");
                    assert!(!accepte(k, &(min - 1).to_string()), "{n} min-1");
                    assert!(!accepte(k, &(max + 1).to_string()), "{n} max+1");
                    assert!(!accepte(k, "1"), "{n} 1");
                }
                Flottant { min, max } => {
                    assert!(accepte(k, &format!("{min:.1}")), "{n} min");
                    assert!(accepte(k, &format!("{max:.1}")), "{n} max");
                    assert!(!accepte(k, &format!("{:.1}", min - 0.1)), "{n} min-0.1");
                    assert!(!accepte(k, &format!("{:.1}", max + 0.1)), "{n} max+0.1");
                }
                Liste { min, max } => {
                    assert!(accepte(k, &format!("[{min}]")), "{n} min");
                    assert!(accepte(k, &format!("[{max}]")), "{n} max");
                    assert!(accepte(k, &format!("[{min}, {}, {max}]", min + 1)), "{n} 3 valeurs");
                    assert!(!accepte(k, &format!("[{}]", min - 1)), "{n} min-1");
                    assert!(!accepte(k, &format!("[{}]", max + 1)), "{n} max+1");
                    assert!(!accepte(k, "[]"), "{n} vide");
                    assert!(!accepte(k, &format!("[{min}, {}, {}, {max}]", min + 1, min + 2)), "{n} 4");
                    assert!(!accepte(k, &format!("[{min}, {min}]")), "{n} egales");
                    assert!(!accepte(k, &format!("[{}, {min}]", min + 1)), "{n} decroissantes");
                }
                Heure => {
                    for ok in ["\"00:00\"", "\"23:59\""] {
                        assert!(accepte(k, ok), "{n} {ok}");
                    }
                    for ko in ["\"24:00\"", "\"12:60\"", "\"7:00\"", "\"0700\""] {
                        assert!(!accepte(k, ko), "{n} {ko}");
                    }
                }
                Touche => {
                    // Les deux autres touches gardent leur defaut : on choisit des caracteres
                    // differents des trois defauts t, s, f.
                    for ok in ["\"!\"", "\"~\"", "\"a\""] {
                        assert!(accepte(k, ok), "{n} {ok}");
                    }
                    for ko in ["\" \"", "\"ab\"", "\"\"", "\"é\""] {
                        assert!(!accepte(k, ko), "{n} {ko}");
                    }
                }
            }
        }
    }

    #[test]
    fn libelles_et_aides_presents_sans_tabulation_ni_saut_de_ligne() {
        let propre = |t: &str| !t.trim().is_empty() && !t.contains(['\t', '\n', '\r']);
        for k in CLES {
            assert!(propre(k.libelle), "{} libelle", k.nom());
            assert!(propre(k.aide), "{} aide", k.nom());
        }
        for (nom, t) in [
            ("rappel.categorie", RAPPEL_CATEGORIE),
            ("rappel.heure", RAPPEL_HEURE),
            ("rappel.jours", RAPPEL_JOURS),
        ] {
            assert!(propre(t.libelle), "{nom} libelle");
            assert!(propre(t.aide), "{nom} aide");
        }
    }

    #[test]
    fn textes_de_spec_010() {
        let k = trouver("gestes.lancer_g").unwrap();
        assert_eq!(k.libelle, "Force d'un lancer");
        assert!(k.aide.starts_with("Accélération, en g, qui déclenche un lancer"));
        assert_eq!(RAPPEL_JOURS.libelle, "Jours du rappel");
    }

    #[test]
    fn chien_px_spec_011() {
        let k = trouver("pc.chien_px").unwrap();
        assert_eq!((k.onglet, k.bornes, k.defaut), (Avance, "32 à 256 px", "64"));
        assert_eq!(k.libelle, "Taille du chien sur le bureau");
        assert_eq!(lire("").unwrap().chien_px, 64);
        assert_eq!(lire("[pc]\nchien_px = 32\n").unwrap().chien_px, 32);
        assert_eq!(lire("[pc]\nchien_px = 256\n").unwrap().chien_px, 256);
        assert_eq!(lire("[pc]\nchien_px = 31\n").unwrap_err(), "pc.chien_px: 31 hors de 32..256");
        assert_eq!(lire("[pc]\nchien_px = 257\n").unwrap_err(), "pc.chien_px: 257 hors de 32..256");
        assert_eq!(lire("[pc]\nchien_px = 300\n").unwrap_err(), "pc.chien_px: 300 hors de 32..256");
    }

    #[test]
    fn trouver_par_nom() {
        assert_eq!(trouver("rappels.report_min").unwrap().defaut, "10");
        assert!(trouver("rappels.inconnue").is_none());
    }
}
