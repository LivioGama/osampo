//! Lecture et validation de osampo.toml, et ligne "cfg" envoyee a la board.
//! Tout ou rien : un fichier invalide est refuse en entier, l erreur nomme la cle.

pub mod bornes;
pub mod essai;

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Modele commente ecrit au premier lancement (toutes les cles a leur defaut).
pub const MODELE: &str = include_str!("../../../widgets/osampo/config.toml");

/// Liste compilee de la board (default_reminders en C++).
const RAPPELS_DEFAUT: &str = "repas,1200,127;sortie,1230,127;sport,1800,21;repas,1930,127;dodo,2145,127";
const MAX_RAPPELS: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Reglages {
    pub report_min: i64,
    pub rattrapage_min: i64,
    /// 1 a 3 valeurs strictement croissantes.
    pub relances_s: Vec<i64>,
    pub calme_validation_min: i64,
    pub patte_avant_aboie_min: i64,
    pub calme_debut: i64,
    pub calme_fin: i64,
    pub nuit_debut: i64,
    pub nuit_fin: i64,
    pub lum_jour: i64,
    pub lum_nuit: i64,
    pub baisse_jour: i64,
    pub gain: i64,
    pub lancer_mg: i64,
    pub lancer_dps: i64,
    pub lancer_ms: i64,
    pub secousse_mg: i64,
    pub secousse_coups: i64,
    pub secousse_fenetre_ms: i64,
    pub retourne_ms: i64,
    pub pause_gestes_ms: i64,
    pub journal: i64,
    pub k_lancer: char,
    pub k_secousse: char,
    pub k_retourner: char,
    /// Promenade cote board (SP5h) et repere batterie (0 : aucun).
    pub seuil_reclame: i64,
    pub retour_min: i64,
    pub batterie_faible_mv: i64,
    /// Texte du fil "categorie,HHMM,masque;..." (memes octets que reminders_to_text).
    pub rappels: String,
    /// Reglages du daemon, hors fil : jauge « sage » (SP5h).
    pub vide_apres_min: u64,
    pub courte_min: u64,
    pub longue_min: u64,
    /// Cote en pixels de la fenetre du chien de osampo-pc (SPEC-011) ; le daemon l'ignore
    /// et ne l'envoie pas a la board.
    pub chien_px: i64,
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages {
            report_min: 10,
            rattrapage_min: 60,
            relances_s: vec![60, 180, 420],
            calme_validation_min: 10,
            patte_avant_aboie_min: 2,
            calme_debut: 1320,
            calme_fin: 420,
            nuit_debut: 1200,
            nuit_fin: 420,
            lum_jour: 100,
            lum_nuit: 20,
            baisse_jour: 15,
            gain: 20,
            lancer_mg: 2000,
            lancer_dps: 300,
            lancer_ms: 20,
            secousse_mg: 2000,
            secousse_coups: 4,
            secousse_fenetre_ms: 1000,
            retourne_ms: 1500,
            pause_gestes_ms: 1500,
            journal: 0,
            k_lancer: 't',
            k_secousse: 's',
            k_retourner: 'f',
            seuil_reclame: 30,
            retour_min: 2,
            batterie_faible_mv: 0,
            rappels: RAPPELS_DEFAUT.into(),
            vide_apres_min: 90,
            courte_min: 5,
            longue_min: 30,
            chien_px: 64,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Fichier {
    rappels: Rappels,
    chiot: Chiot,
    heures: Heures,
    ecran: Ecran,
    besoins: Besoins,
    gestes: Gestes,
    clavier: Clavier,
    poste: Poste,
    promenade: Promenade,
    pc: Pc,
    rappel: Option<Vec<Rappel>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rappel {
    categorie: String,
    heure: String,
    jours: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Rappels {
    report_min: Option<i64>,
    rattrapage_min: Option<i64>,
    relances_s: Option<Vec<i64>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Chiot {
    calme_apres_validation_min: Option<i64>,
    patte_avant_aboie_min: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Heures {
    calme_debut: Option<String>,
    calme_fin: Option<String>,
    nuit_debut: Option<String>,
    nuit_fin: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Ecran {
    lum_jour: Option<i64>,
    lum_nuit: Option<i64>,
    batterie_faible_mv: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Besoins {
    baisse_par_jour: Option<i64>,
    gain: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Gestes {
    lancer_g: Option<f64>,
    lancer_dps: Option<i64>,
    lancer_ms: Option<i64>,
    secousse_g: Option<f64>,
    secousse_coups: Option<i64>,
    secousse_fenetre_ms: Option<i64>,
    retourne_ms: Option<i64>,
    pause_gestes_ms: Option<i64>,
    journal: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Clavier {
    lancer: Option<String>,
    secousse: Option<String>,
    retourner: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Poste {
    laisse_apres_min: Option<i64>,
    pause_min: Option<i64>,
}

/// SP5h ; [poste] (laisse_apres_min, pause_min) reste lu comme ancien nom.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Promenade {
    vide_apres_min: Option<i64>,
    courte_min: Option<i64>,
    longue_min: Option<i64>,
    seuil_reclame: Option<i64>,
    retour_min: Option<i64>,
}

/// SPEC-011 : lue par osampo-pc, acceptee ici pour que le fichier ne soit pas refuse.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Pc {
    chien_px: Option<i64>,
}

/// Verifie une borne ; `cle` est le nom complet "section.cle".
fn borne(cle: &str, v: i64, min: i64, max: i64) -> Result<i64, String> {
    if v < min || v > max {
        Err(format!("{cle}: {v} hors de {min}..{max}"))
    } else {
        Ok(v)
    }
}

fn entier(cle: &str, v: Option<i64>, defaut: i64, min: i64, max: i64) -> Result<i64, String> {
    match v {
        Some(v) => borne(cle, v, min, max),
        None => Ok(defaut),
    }
}

/// Flottant en g -> milliemes arrondis, puis verifie en millg.
fn milli(cle: &str, v: Option<f64>, defaut: i64) -> Result<i64, String> {
    match v {
        None => Ok(defaut),
        Some(g) => {
            if !g.is_finite() || g.abs() > 1.0e6 {
                return Err(format!("{cle}: {g} hors de 0.5..16"));
            }
            let mg = (g * 1000.0).round() as i64;
            if !(500..=16000).contains(&mg) {
                return Err(format!("{cle}: {g} hors de 0.5..16"));
            }
            Ok(mg)
        }
    }
}

/// "HH:MM" strict -> minutes depuis minuit.
fn heure(cle: &str, v: &Option<String>, defaut: i64) -> Result<i64, String> {
    let Some(t) = v else { return Ok(defaut) };
    let o = t.as_bytes();
    let ok = o.len() == 5
        && o[2] == b':'
        && [0, 1, 3, 4].iter().all(|&i| o[i].is_ascii_digit());
    if !ok {
        return Err(format!("{cle}: \"{t}\" n est pas une heure HH:MM"));
    }
    let h = ((o[0] - b'0') * 10 + (o[1] - b'0')) as i64;
    let m = ((o[3] - b'0') * 10 + (o[4] - b'0')) as i64;
    if h > 23 || m > 59 {
        return Err(format!("{cle}: \"{t}\" hors de 00:00..23:59"));
    }
    Ok(h * 60 + m)
}

fn touche(cle: &str, v: &Option<String>, defaut: char) -> Result<char, String> {
    let Some(t) = v else { return Ok(defaut) };
    let mut it = t.chars();
    match (it.next(), it.next()) {
        (Some(c), None) if ('\u{21}'..='\u{7e}').contains(&c) => Ok(c),
        _ => Err(format!(
            "{cle}: \"{t}\" doit etre un seul caractere ASCII imprimable (hors espace)"
        )),
    }
}

/// Lit et valide le contenu d un osampo.toml. Tout ou rien.
pub fn lire(texte: &str) -> Result<Reglages, String> {
    let f: Fichier = toml::from_str(texte).map_err(|e| e.to_string())?;
    let d = Reglages::default();

    let relances_s = match f.rappels.relances_s {
        None => d.relances_s.clone(),
        Some(v) => {
            if v.is_empty() || v.len() > 3 {
                return Err(format!(
                    "rappels.relances_s: {} valeurs, 1 a 3 attendues",
                    v.len()
                ));
            }
            for (i, &x) in v.iter().enumerate() {
                borne("rappels.relances_s", x, 10, 3600)?;
                if i > 0 && x <= v[i - 1] {
                    return Err(format!(
                        "rappels.relances_s: {x} doit etre superieur a {}",
                        v[i - 1]
                    ));
                }
            }
            v
        }
    };

    let k_lancer = touche("clavier.lancer", &f.clavier.lancer, d.k_lancer)?;
    let k_secousse = touche("clavier.secousse", &f.clavier.secousse, d.k_secousse)?;
    let k_retourner = touche("clavier.retourner", &f.clavier.retourner, d.k_retourner)?;
    if k_lancer == k_secousse || k_lancer == k_retourner || k_secousse == k_retourner {
        return Err(format!(
            "clavier: les touches lancer, secousse et retourner doivent etre distinctes ('{k_lancer}', '{k_secousse}', '{k_retourner}')"
        ));
    }

    let g = &f.gestes;
    let rappels = match &f.rappel {
        None => RAPPELS_DEFAUT.to_string(),
        Some(l) => texte_rappels(l)?,
    };
    let batterie_faible_mv = match f.ecran.batterie_faible_mv {
        None => d.batterie_faible_mv,
        Some(v) if v == 0 || (3000..=4200).contains(&v) => v,
        Some(v) => return Err(format!("ecran.batterie_faible_mv: {v} : 0 ou 3000..4200")),
    };
    let p = &f.promenade;
    let ancien = f.poste.laisse_apres_min.is_some() || f.poste.pause_min.is_some();
    if ancien && (p.vide_apres_min.is_some() || p.courte_min.is_some() || p.longue_min.is_some()) {
        return Err("[poste] et [promenade] a la fois : garder seulement [promenade]".into());
    }
    let sec = if ancien { "poste" } else { "promenade" };
    let (vide, courte) = if ancien {
        (f.poste.laisse_apres_min, f.poste.pause_min)
    } else {
        (p.vide_apres_min, p.courte_min)
    };
    let courte_min = entier(
        &format!("{sec}.{}", if ancien { "pause_min" } else { "courte_min" }),
        courte,
        d.courte_min as i64,
        1,
        120,
    )? as u64;
    let longue_min = entier("promenade.longue_min", p.longue_min, d.longue_min as i64, 2, 240)? as u64;
    if longue_min <= courte_min {
        return Err(format!(
            "promenade.longue_min: {longue_min} doit etre superieur a courte_min ({courte_min})"
        ));
    }
    Ok(Reglages {
        report_min: entier("rappels.report_min", f.rappels.report_min, d.report_min, 1, 120)?,
        rattrapage_min: entier(
            "rappels.rattrapage_min",
            f.rappels.rattrapage_min,
            d.rattrapage_min,
            1,
            720,
        )?,
        relances_s,
        calme_validation_min: entier(
            "chiot.calme_apres_validation_min",
            f.chiot.calme_apres_validation_min,
            d.calme_validation_min,
            1,
            120,
        )?,
        patte_avant_aboie_min: entier(
            "chiot.patte_avant_aboie_min",
            f.chiot.patte_avant_aboie_min,
            d.patte_avant_aboie_min,
            1,
            60,
        )?,
        calme_debut: heure("heures.calme_debut", &f.heures.calme_debut, d.calme_debut)?,
        calme_fin: heure("heures.calme_fin", &f.heures.calme_fin, d.calme_fin)?,
        nuit_debut: heure("heures.nuit_debut", &f.heures.nuit_debut, d.nuit_debut)?,
        nuit_fin: heure("heures.nuit_fin", &f.heures.nuit_fin, d.nuit_fin)?,
        lum_jour: entier("ecran.lum_jour", f.ecran.lum_jour, d.lum_jour, 5, 100)?,
        lum_nuit: entier("ecran.lum_nuit", f.ecran.lum_nuit, d.lum_nuit, 5, 100)?,
        baisse_jour: entier(
            "besoins.baisse_par_jour",
            f.besoins.baisse_par_jour,
            d.baisse_jour,
            0,
            100,
        )?,
        gain: entier("besoins.gain", f.besoins.gain, d.gain, 0, 100)?,
        lancer_mg: milli("gestes.lancer_g", g.lancer_g, d.lancer_mg)?,
        lancer_dps: entier("gestes.lancer_dps", g.lancer_dps, d.lancer_dps, 50, 4000)?,
        lancer_ms: entier("gestes.lancer_ms", g.lancer_ms, d.lancer_ms, 10, 1000)?,
        secousse_mg: milli("gestes.secousse_g", g.secousse_g, d.secousse_mg)?,
        secousse_coups: entier("gestes.secousse_coups", g.secousse_coups, d.secousse_coups, 2, 20)?,
        secousse_fenetre_ms: entier(
            "gestes.secousse_fenetre_ms",
            g.secousse_fenetre_ms,
            d.secousse_fenetre_ms,
            200,
            5000,
        )?,
        retourne_ms: entier("gestes.retourne_ms", g.retourne_ms, d.retourne_ms, 200, 10000)?,
        pause_gestes_ms: entier(
            "gestes.pause_gestes_ms",
            g.pause_gestes_ms,
            d.pause_gestes_ms,
            0,
            10000,
        )?,
        journal: entier("gestes.journal", g.journal, d.journal, 0, 2)?,
        k_lancer,
        k_secousse,
        k_retourner,
        vide_apres_min: entier(
            &format!("{sec}.{}", if ancien { "laisse_apres_min" } else { "vide_apres_min" }),
            vide,
            d.vide_apres_min as i64,
            1,
            600,
        )? as u64,
        courte_min,
        longue_min,
        seuil_reclame: entier("promenade.seuil_reclame", p.seuil_reclame, d.seuil_reclame, 0, 100)?,
        retour_min: entier("promenade.retour_min", p.retour_min, d.retour_min, 1, 60)?,
        batterie_faible_mv,
        rappels,
        chien_px: entier("pc.chien_px", f.pc.chien_px, d.chien_px, 32, 256)?,
    })
}

/// [[rappel]] -> texte du fil ; l erreur nomme le rappel (1 = premier).
fn texte_rappels(l: &[Rappel]) -> Result<String, String> {
    const JOURS: [&str; 7] = ["lun", "mar", "mer", "jeu", "ven", "sam", "dim"];
    if l.is_empty() || l.len() > MAX_RAPPELS {
        return Err(format!("rappel: 1 a {MAX_RAPPELS} rappels, {} donnes", l.len()));
    }
    let mut out = Vec::new();
    for (i, r) in l.iter().enumerate() {
        let n = i + 1;
        if !["repas", "sortie", "sport", "dodo"].contains(&r.categorie.as_str()) {
            return Err(format!("rappel {n}: categorie \"{}\" (repas, sortie, sport, dodo)", r.categorie));
        }
        let m = heure(&format!("rappel {n}: heure"), &Some(r.heure.clone()), 0)?;
        let masque = match &r.jours {
            None => 127,
            Some(j) if j.is_empty() => return Err(format!("rappel {n}: jours vide")),
            Some(j) => {
                let mut m = 0;
                for x in j {
                    let Some(k) = JOURS.iter().position(|y| y == x) else {
                        return Err(format!("rappel {n}: jour \"{x}\" (lun..dim)"));
                    };
                    m |= 1 << k;
                }
                m
            }
        };
        out.push(format!("{},{:02}{:02},{masque}", r.categorie, m / 60, m % 60));
    }
    Ok(out.join(";"))
}

/// $XDG_CONFIG_HOME/osampo/osampo.toml, repli ~/.config.
pub fn chemin_par_defaut() -> PathBuf {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(x) if !x.is_empty() => PathBuf::from(x),
        _ => {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        }
    };
    base.join("osampo").join("osampo.toml")
}

/// Fichier absent : ecrit MODELE (repertoires crees) et rend les defauts.
/// Fichier present : jamais reecrit.
pub fn charger_ou_creer(chemin: &Path) -> Result<Reglages, String> {
    if chemin.exists() {
        let texte = std::fs::read_to_string(chemin)
            .map_err(|e| format!("lecture de {}: {e}", chemin.display()))?;
        return lire(&texte);
    }
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creation de {}: {e}", parent.display()))?;
    }
    std::fs::write(chemin, MODELE)
        .map_err(|e| format!("ecriture de {}: {e}", chemin.display()))?;
    Ok(Reglages::default())
}

fn echappe(c: char) -> String {
    if c == '"' || c == '\\' {
        format!("\\{c}")
    } else {
        c.to_string()
    }
}

impl Reglages {
    /// Ligne JSON plate du fil, memes octets que Settings::to_line en C++.
    pub fn ligne_cfg(&self) -> String {
        let r = |i: usize| self.relances_s.get(i).copied().unwrap_or(0);
        format!(
            "{{\"cfg\":1,\"report_min\":{},\"rattrapage_min\":{},\"relance1_s\":{},\"relance2_s\":{},\"relance3_s\":{},\"calme_validation_min\":{},\"patte_avant_aboie_min\":{},\"calme_debut\":{},\"calme_fin\":{},\"nuit_debut\":{},\"nuit_fin\":{},\"lum_jour\":{},\"lum_nuit\":{},\"baisse_jour\":{},\"gain\":{},\"lancer_mg\":{},\"lancer_dps\":{},\"lancer_ms\":{},\"secousse_mg\":{},\"secousse_coups\":{},\"secousse_fenetre_ms\":{},\"retourne_ms\":{},\"pause_gestes_ms\":{},\"journal\":{},\"courte_min\":{},\"seuil_reclame\":{},\"retour_min\":{},\"batterie_faible_mv\":{},\"k_lancer\":\"{}\",\"k_secousse\":\"{}\",\"k_retourner\":\"{}\",\"rappels\":\"{}\"}}",
            self.report_min,
            self.rattrapage_min,
            r(0),
            r(1),
            r(2),
            self.calme_validation_min,
            self.patte_avant_aboie_min,
            self.calme_debut,
            self.calme_fin,
            self.nuit_debut,
            self.nuit_fin,
            self.lum_jour,
            self.lum_nuit,
            self.baisse_jour,
            self.gain,
            self.lancer_mg,
            self.lancer_dps,
            self.lancer_ms,
            self.secousse_mg,
            self.secousse_coups,
            self.secousse_fenetre_ms,
            self.retourne_ms,
            self.pause_gestes_ms,
            self.journal,
            self.courte_min,
            self.seuil_reclame,
            self.retour_min,
            self.batterie_faible_mv,
            echappe(self.k_lancer),
            echappe(self.k_secousse),
            echappe(self.k_retourner),
            self.rappels,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier_temp(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sp5c_config_{}_{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn modele_donne_les_defauts() {
        assert_eq!(lire(MODELE).unwrap(), Reglages::default());
    }

    #[test]
    fn ligne_cfg_egale_au_fichier_commun() {
        let ref_ = include_str!("../../../widgets/osampo/src/test/cfg_defaut.json");
        assert_eq!(
            Reglages::default().ligne_cfg(),
            ref_.strip_suffix('\n').unwrap_or(ref_)
        );
    }

    #[test]
    fn promenade_lue() {
        let r = lire("[promenade]\nvide_apres_min = 60\ncourte_min = 4\nlongue_min = 20\n").unwrap();
        assert_eq!((r.vide_apres_min, r.courte_min, r.longue_min), (60, 4, 20));
        let e = lire("[promenade]\ncourte_min = 30\nlongue_min = 30\n").unwrap_err();
        assert!(e.starts_with("promenade.longue_min:"), "{e}");
        assert!(lire("[promenade]\nvide_apres_min = 601\n").is_err());
        assert!(lire("[promenade]\ncourte_min = 0\n").is_err());
    }

    #[test]
    fn rappels_lus() {
        let r = lire(
            "[[rappel]]\ncategorie = \"sortie\"\nheure = \"17:30\"\njours = [\"lun\", \"mar\", \"ven\"]\n\n[[rappel]]\ncategorie = \"dodo\"\nheure = \"21:45\"\n",
        )
        .unwrap();
        assert_eq!(r.rappels, "sortie,1730,19;dodo,2145,127");
        assert!(r.ligne_cfg().ends_with(",\"rappels\":\"sortie,1730,19;dodo,2145,127\"}"));
        for faux in [
            "[[rappel]]\ncategorie = \"chat\"\nheure = \"12:00\"\n",
            "[[rappel]]\ncategorie = \"repas\"\nheure = \"25:00\"\n",
            "[[rappel]]\ncategorie = \"repas\"\nheure = \"12:00\"\njours = [\"lundi\"]\n",
            "[[rappel]]\ncategorie = \"repas\"\nheure = \"12:00\"\njours = []\n",
        ] {
            assert!(lire(faux).unwrap_err().starts_with("rappel"), "{faux}");
        }
        let dix_sept = "[[rappel]]\ncategorie = \"repas\"\nheure = \"12:00\"\n".repeat(17);
        assert!(lire(&dix_sept).is_err());
    }

    #[test]
    fn promenade_et_batterie_sur_le_fil() {
        let r = lire("[promenade]\nseuil_reclame = 40\nretour_min = 3\ncourte_min = 6\n[ecran]\nbatterie_faible_mv = 3500\n").unwrap();
        let l = r.ligne_cfg();
        assert!(l.contains("\"courte_min\":6,\"seuil_reclame\":40,\"retour_min\":3,\"batterie_faible_mv\":3500,"), "{l}");
        assert!(lire("[ecran]\nbatterie_faible_mv = 2000\n").is_err());
        assert!(lire("[promenade]\nseuil_reclame = 101\n").is_err());
    }

    #[test]
    fn anciennes_cles_poste_acceptees() {
        let r = lire("[poste]\nlaisse_apres_min = 80\npause_min = 6\n").unwrap();
        assert_eq!((r.vide_apres_min, r.courte_min), (80, 6));
        let e = lire("[poste]\npause_min = 6\n[promenade]\ncourte_min = 4\n").unwrap_err();
        assert!(e.contains("[poste]") && e.contains("[promenade]"), "{e}");
    }

    #[test]
    fn fichier_vide_donne_les_defauts() {
        assert_eq!(lire("").unwrap(), Reglages::default());
    }

    #[test]
    fn cle_inconnue_refusee() {
        let e = lire("[rappels]\nreport_minn = 5\n").unwrap_err();
        assert!(e.contains("report_minn"), "{e}");
        let e = lire("[inconnue]\nx = 1\n").unwrap_err();
        assert!(e.contains("inconnue"), "{e}");
    }

    #[test]
    fn valeur_hors_bornes_refusee() {
        let e = lire("[rappels]\nreport_min = 0\n").unwrap_err();
        assert!(e.contains("report_min"), "{e}");
        // 0 couperait tous les rappels (fenetre vide) : refuse.
        let e = lire("[rappels]\nrattrapage_min = 0\n").unwrap_err();
        assert!(e.contains("rattrapage_min"), "{e}");
        let e = lire("[gestes]\nlancer_g = 0.1\n").unwrap_err();
        assert!(e.contains("lancer_g"), "{e}");
        let e = lire("[rappels]\nrelances_s = [60, 60]\n").unwrap_err();
        assert!(e.contains("relances_s"), "{e}");
        let e = lire("[rappels]\nrelances_s = []\n").unwrap_err();
        assert!(e.contains("relances_s"), "{e}");
    }

    #[test]
    fn patte_avant_aboie_min_lue_et_bornee() {
        assert_eq!(lire("").unwrap().patte_avant_aboie_min, 2);
        let r = lire("[chiot]\npatte_avant_aboie_min = 5\n").unwrap();
        assert_eq!(r.patte_avant_aboie_min, 5);
        assert!(r.ligne_cfg().contains("\"calme_validation_min\":10,\"patte_avant_aboie_min\":5,"));
        let e = lire("[chiot]\npatte_avant_aboie_min = 0\n").unwrap_err();
        assert!(e.contains("chiot.patte_avant_aboie_min: 0 hors de 1..60"), "{e}");
        assert!(lire("[chiot]\npatte_avant_aboie_min = 61\n").is_err());
    }

    #[test]
    fn heure_invalide_refusee() {
        for h in ["25:00", "7:00", "07:60", "0700", "07:0a"] {
            let e = lire(&format!("[heures]\ncalme_debut = \"{h}\"\n")).unwrap_err();
            assert!(e.contains("calme_debut"), "{h}: {e}");
        }
        let r = lire("[heures]\ncalme_debut = \"23:59\"\ncalme_fin = \"00:00\"\n").unwrap();
        assert_eq!((r.calme_debut, r.calme_fin), (1439, 0));
    }

    #[test]
    fn touches_identiques_refusees() {
        let e = lire("[clavier]\nlancer = \"x\"\nsecousse = \"x\"\n").unwrap_err();
        assert!(e.contains("clavier"), "{e}");
        let e = lire("[clavier]\nlancer = \"ab\"\n").unwrap_err();
        assert!(e.contains("clavier.lancer"), "{e}");
        let e = lire("[clavier]\nlancer = \" \"\n").unwrap_err();
        assert!(e.contains("clavier.lancer"), "{e}");
    }

    #[test]
    fn relances_partielles_et_g_arrondis() {
        let r = lire("[rappels]\nrelances_s = [30]\n[gestes]\nlancer_g = 2.0005\nsecousse_g = 3\n")
            .unwrap();
        assert_eq!(r.relances_s, vec![30]);
        assert!(r.ligne_cfg().contains("\"relance1_s\":30,\"relance2_s\":0,\"relance3_s\":0"));
        assert_eq!(r.lancer_mg, 2001);
        assert_eq!(r.secousse_mg, 3000);
    }

    #[test]
    fn touches_echappees_sur_le_fil() {
        let r = lire("[clavier]\nlancer = \"\\\"\"\nsecousse = \"\\\\\"\n").unwrap();
        assert!(r.ligne_cfg().contains("\"k_lancer\":\"\\\"\",\"k_secousse\":\"\\\\\""));
        let v: serde_json::Value = serde_json::from_str(&r.ligne_cfg()).unwrap();
        assert_eq!(v["k_lancer"], "\"");
        assert_eq!(v["k_secousse"], "\\");
    }

    #[test]
    fn fichier_absent_cree() {
        let d = dossier_temp("absent");
        let chemin = d.join("sous").join("osampo.toml");
        let r = charger_ou_creer(&chemin).unwrap();
        assert_eq!(r, Reglages::default());
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), MODELE);
        assert_eq!(charger_ou_creer(&chemin).unwrap(), Reglages::default());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn fichier_present_jamais_reecrit() {
        let d = dossier_temp("present");
        std::fs::create_dir_all(&d).unwrap();
        let chemin = d.join("osampo.toml");
        let texte = "[rappels]\nreport_min = 25\n";
        std::fs::write(&chemin, texte).unwrap();
        assert_eq!(charger_ou_creer(&chemin).unwrap().report_min, 25);
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), texte);
        // Un fichier invalide est refuse et laisse intact.
        std::fs::write(&chemin, "[rappels]\nreport_min = 0\n").unwrap();
        assert!(charger_ou_creer(&chemin).is_err());
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "[rappels]\nreport_min = 0\n");
        let _ = std::fs::remove_dir_all(&d);
    }
}
