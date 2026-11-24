//! État publié par le daemon (SPEC-002) et lignes de statut de l'infobulle et du menu
//! (SPEC-004). Tout est pur : testé sans D-Bus ni socket.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Rappel {
    pub id: String,
    pub categorie: String,
    pub heure: String,
    /// `reclame` ou `reporte`
    pub etat: String,
}

impl Rappel {
    pub fn reclame(&self) -> bool {
        self.etat == "reclame"
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geste {
    pub geste: String,
    pub source: String,
    pub heure: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Etat {
    pub etat: String,
    pub icone: String,
    pub sage: i64,
    pub prochain_rappel: String,
    /// `HH:MM` ou `-`
    pub promenade_debut: String,
    pub connectee: bool,
    pub pile: i64,
    pub besoins: Option<(i64, i64, i64)>,
    pub rappel_en_cours: Option<Rappel>,
    pub calme: bool,
    pub dernier_geste: Option<Geste>,
}

fn texte(v: &Value, cle: &str, defaut: &str) -> String {
    v.get(cle).and_then(Value::as_str).unwrap_or(defaut).to_string()
}

fn rappel(v: &Value) -> Option<Rappel> {
    if !v.is_object() {
        return None;
    }
    Some(Rappel {
        id: texte(v, "id", ""),
        categorie: texte(v, "categorie", ""),
        heure: texte(v, "heure", ""),
        etat: texte(v, "etat", "reclame"),
    })
}

impl Etat {
    /// Lit une ligne du daemon ; `None` si ce n'est pas une ligne d'état (réponse à une
    /// commande, ligne illisible).
    pub fn lire(ligne: &str) -> Option<Etat> {
        let v: Value = serde_json::from_str(ligne).ok()?;
        let pc = v.get("pc")?;
        let besoins = pc.get("besoins").filter(|b| b.is_object()).map(|b| {
            let n = |k: &str| b.get(k).and_then(Value::as_i64).unwrap_or(0);
            (n("bouger"), n("jouer"), n("dormir"))
        });
        let geste = pc.get("dernier_geste").filter(|g| g.is_object()).map(|g| Geste {
            geste: texte(g, "geste", ""),
            source: texte(g, "source", ""),
            heure: texte(g, "heure", ""),
        });
        Some(Etat {
            etat: texte(pc, "etat", "-"),
            icone: texte(pc, "icone", "assis"),
            sage: pc.get("sage").and_then(Value::as_i64).unwrap_or(0),
            prochain_rappel: texte(pc, "prochain_rappel", "aucun"),
            promenade_debut: texte(pc, "promenade_debut", "-"),
            connectee: texte(pc, "board", "hors_ligne") == "connectee",
            pile: pc.get("pile").and_then(Value::as_i64).unwrap_or(-1),
            besoins,
            rappel_en_cours: pc.get("rappel_en_cours").and_then(rappel),
            calme: pc.get("calme").and_then(Value::as_bool).unwrap_or(false),
            dernier_geste: geste,
        })
    }
}

/// Ce que l'icône montre : le dernier état du daemon, ou `None` s'il est injoignable.
pub type Vue = Option<Etat>;

/// Nom de l'icône : `osampo-` + `icone`, `osampo-assis` daemon injoignable.
pub fn nom_icone(vue: &Vue) -> String {
    format!("osampo-{}", vue.as_ref().map_or("assis", |e| e.icone.as_str()))
}

pub fn attention(vue: &Vue) -> bool {
    vue.as_ref().is_some_and(|e| matches!(e.icone.as_str(), "aboie" | "patte" | "reclame_laisse"))
}

pub fn titre(vue: &Vue) -> &'static str {
    match vue {
        Some(e) if !e.connectee => "Osampo : board hors ligne",
        _ => "Osampo",
    }
}

/// Les lignes de statut, dans l'ordre de SPEC-004 (infobulle et début du menu).
pub fn lignes(vue: &Vue) -> Vec<String> {
    let Some(e) = vue else {
        return vec!["Daemon : injoignable".to_string()];
    };
    let rappel = match &e.rappel_en_cours {
        Some(r) => {
            let etat = if r.reclame() { "en attente" } else { "reporté" };
            if r.categorie == "promenade" {
                format!("Rappel : promenade (sage à 0), {etat}")
            } else {
                format!("Rappel : {} de {}, {etat}", r.categorie, r.heure)
            }
        }
        None => format!("Prochain rappel : {}", e.prochain_rappel),
    };
    let promenade = if e.promenade_debut == "-" {
        "Promenade : non".to_string()
    } else {
        format!("Promenade : oui, depuis {}", e.promenade_debut)
    };
    let pile = if e.connectee && e.pile >= 0 {
        format!("Pile : {} %", e.pile)
    } else {
        "Pile : inconnue".to_string()
    };
    let besoins = match e.besoins {
        Some((b, j, d)) => format!("Besoins : bouger {b} %, jouer {j} %, dormir {d} %"),
        None => "Besoins : inconnus".to_string(),
    };
    let geste = match &e.dernier_geste {
        Some(g) => {
            let source = if g.source == "pc" { "PC" } else { g.source.as_str() };
            format!("Dernier geste : {} ({source}) à {}", g.geste, g.heure)
        }
        None => "Dernier geste : aucun".to_string(),
    };
    vec![
        format!("Sage : {} %", e.sage),
        rappel,
        promenade,
        format!("Board : {}", if e.connectee { "connectée" } else { "hors ligne" }),
        pile,
        besoins,
        geste,
    ]
}

/// `Chatouiller` et `Donner un os` possibles ? Grisés quand le chiot dort, réclame ou est en
/// promenade (board connectée), pendant une réclamation de sortie, daemon injoignable.
pub fn gestes_possibles(vue: &Vue) -> bool {
    let Some(e) = vue else { return false };
    if e.rappel_en_cours.as_ref().is_some_and(Rappel::reclame) {
        return false;
    }
    !(e.connectee && matches!(e.etat.as_str(), "dort" | "reclame" | "promenade"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIGNE: &str = r#"{"pc":{"etat":"repos","icone":"assis","sage":72,
        "prochain_rappel":"sortie à 12:30","promenade_debut":"-","board":"connectee","pile":83,
        "besoins":{"bouger":70,"jouer":70,"dormir":70},"rappel_en_cours":null,"calme":false,
        "dernier_geste":{"geste":"os","source":"board","heure":"14:12"}}}"#;

    fn etat() -> Etat {
        Etat::lire(LIGNE).unwrap()
    }

    #[test]
    fn lit_la_ligne_de_spec_002() {
        let e = etat();
        assert_eq!(e.sage, 72);
        assert!(e.connectee && !e.calme);
        assert_eq!(e.besoins, Some((70, 70, 70)));
        assert_eq!(e.rappel_en_cours, None);
        assert!(Etat::lire(r#"{"reponse":"os","ok":true}"#).is_none());
        assert!(Etat::lire("pas du json").is_none());
    }

    #[test]
    fn lignes_board_connectee() {
        assert_eq!(
            lignes(&Some(etat())),
            [
                "Sage : 72 %",
                "Prochain rappel : sortie à 12:30",
                "Promenade : non",
                "Board : connectée",
                "Pile : 83 %",
                "Besoins : bouger 70 %, jouer 70 %, dormir 70 %",
                "Dernier geste : os (board) à 14:12",
            ]
        );
    }

    #[test]
    fn lignes_hors_ligne_et_inconnus() {
        let mut e = etat();
        e.connectee = false;
        e.besoins = None;
        e.dernier_geste = Some(Geste { geste: "chatouille".into(), source: "pc".into(), heure: "14:15".into() });
        e.promenade_debut = "14:05".into();
        let l = lignes(&Some(e.clone()));
        assert_eq!(l[2], "Promenade : oui, depuis 14:05");
        assert_eq!(l[3], "Board : hors ligne");
        assert_eq!(l[4], "Pile : inconnue");
        assert_eq!(l[5], "Besoins : inconnus");
        assert_eq!(l[6], "Dernier geste : chatouille (PC) à 14:15");
        assert_eq!(titre(&Some(e)), "Osampo : board hors ligne");
        e_pile_moins_un();
    }

    fn e_pile_moins_un() {
        let mut e = etat();
        e.pile = -1;
        assert_eq!(lignes(&Some(e))[4], "Pile : inconnue");
    }

    #[test]
    fn lignes_du_rappel_en_cours() {
        let mut e = etat();
        let r = |id: &str, cat: &str, etat: &str| Rappel {
            id: id.into(), categorie: cat.into(), heure: "12:30".into(), etat: etat.into(),
        };
        e.rappel_en_cours = Some(r("sortie,1230", "sortie", "reclame"));
        assert_eq!(lignes(&Some(e.clone()))[1], "Rappel : sortie de 12:30, en attente");
        e.rappel_en_cours = Some(r("dodo,1230", "dodo", "reporte"));
        assert_eq!(lignes(&Some(e.clone()))[1], "Rappel : dodo de 12:30, reporté");
        e.rappel_en_cours = Some(r("promenade", "promenade", "reclame"));
        assert_eq!(lignes(&Some(e.clone()))[1], "Rappel : promenade (sage à 0), en attente");
        e.rappel_en_cours = Some(r("promenade", "promenade", "reporte"));
        assert_eq!(lignes(&Some(e))[1], "Rappel : promenade (sage à 0), reporté");
    }

    #[test]
    fn daemon_injoignable() {
        assert_eq!(lignes(&None), ["Daemon : injoignable"]);
        assert_eq!(nom_icone(&None), "osampo-assis");
        assert!(!attention(&None));
        assert!(!gestes_possibles(&None));
    }

    #[test]
    fn icone_et_attention() {
        let mut e = etat();
        e.icone = "aboie".into();
        assert_eq!(nom_icone(&Some(e.clone())), "osampo-aboie");
        assert!(attention(&Some(e)));
        assert!(!attention(&Some(etat())));
    }

    #[test]
    fn icone_patte_demande_l_attention() {
        let mut e = etat();
        e.icone = "patte".into();
        assert_eq!(nom_icone(&Some(e.clone())), "osampo-patte");
        assert!(attention(&Some(e)));
    }

    #[test]
    fn icone_reclame_laisse_demande_l_attention() {
        let mut e = etat();
        e.icone = "reclame_laisse".into();
        assert_eq!(nom_icone(&Some(e.clone())), "osampo-reclame_laisse");
        assert!(attention(&Some(e)));
    }

    #[test]
    fn grisage_des_gestes() {
        let mut e = etat();
        assert!(gestes_possibles(&Some(e.clone())));
        for s in ["dort", "reclame", "promenade"] {
            e.etat = s.into();
            assert!(!gestes_possibles(&Some(e.clone())), "{s}");
            e.connectee = false; // lien coupé : la réponse « board hors ligne » suffit
            assert!(gestes_possibles(&Some(e.clone())), "{s} hors ligne");
            e.connectee = true;
        }
        e.etat = "repos".into();
        e.rappel_en_cours = Some(Rappel { id: "p".into(), categorie: "promenade".into(), heure: "1".into(), etat: "reclame".into() });
        assert!(!gestes_possibles(&Some(e.clone())));
        e.rappel_en_cours.as_mut().unwrap().etat = "reporte".into();
        assert!(gestes_possibles(&Some(e)));
    }
}
