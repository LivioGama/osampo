//! Edition de `osampo.toml` en conservant le fichier a l'octet pres hors des valeurs
//! modifiees (SPEC-007). Les regles de validation sont celles du daemon : `osampo_config::lire`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use osampo_config::bornes::{self, Cle};
use osampo_config::{lire, MODELE};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Key, Table, Value};

pub const JOURS: [&str; 7] = ["lun", "mar", "mer", "jeu", "ven", "sam", "dim"];

/// Un `[[rappel]]` ; `jours` rempli des sept jours quand le fichier n'en donne pas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rappel {
    pub categorie: String,
    pub heure: String,
    pub jours: Vec<String>,
}

impl Rappel {
    pub fn nouveau(categorie: &str, heure: &str) -> Self {
        Rappel {
            categorie: categorie.into(),
            heure: heure.into(),
            jours: JOURS.iter().map(|j| j.to_string()).collect(),
        }
    }
}

/// Document ouvert pour edition. Rien n'est ecrit avant `enregistrer`.
pub struct Edition {
    doc: DocumentMut,
    chemin: PathBuf,
}

impl Edition {
    /// Fichier absent : part du modele. TOML illisible : erreur.
    pub fn charger(chemin: &Path) -> Result<Edition, String> {
        let texte = if chemin.exists() {
            fs::read_to_string(chemin)
                .map_err(|e| format!("lecture de {}: {e}", chemin.display()))?
        } else {
            MODELE.to_string()
        };
        Edition::depuis_texte(&texte, chemin)
    }

    pub fn depuis_texte(texte: &str, chemin: &Path) -> Result<Edition, String> {
        let doc: DocumentMut = texte
            .parse()
            .map_err(|e| format!("lecture de {}: {e}", chemin.display()))?;
        let mut ed = Edition { doc, chemin: chemin.to_path_buf() };
        ed.migrer_poste();
        Ok(ed)
    }

    pub fn chemin(&self) -> &Path {
        &self.chemin
    }

    pub fn texte(&self) -> String {
        self.doc.to_string()
    }

    /// `[poste]` -> `[promenade]` (laisse_apres_min -> vide_apres_min, pause_min -> courte_min),
    /// commentaires conserves. Si `[promenade]` existe aussi, rien n'est touche : `lire` refusera.
    fn migrer_poste(&mut self) {
        if self.doc.contains_key("promenade") {
            return;
        }
        let Some(Item::Table(ancien)) = self.doc.get("poste") else { return };
        let mut neuf = Table::new();
        *neuf.decor_mut() = ancien.decor().clone();
        neuf.set_implicit(ancien.is_implicit());
        neuf.set_position(ancien.position());
        for (k, v) in ancien.iter() {
            let nom = match k {
                "laisse_apres_min" => "vide_apres_min",
                "pause_min" => "courte_min",
                autre => autre,
            };
            let (cle, _) = ancien.get_key_value(k).expect("cle listee");
            let mut nk = Key::new(nom);
            *nk.leaf_decor_mut() = cle.leaf_decor().clone();
            *nk.dotted_decor_mut() = cle.dotted_decor().clone();
            neuf.insert_formatted(&nk, v.clone());
        }
        self.doc.remove("poste");
        self.doc.insert("promenade", Item::Table(neuf));
    }

    /// Valeur d'une cle en syntaxe TOML (sans decor), ou son defaut si elle est absente.
    pub fn valeur(&self, cle: &Cle) -> String {
        match self.doc.get(cle.section).and_then(|s| s.get(cle.cle)) {
            Some(Item::Value(v)) => sans_decor(v),
            _ => cle.defaut.to_string(),
        }
    }

    /// Modifie une cle `section.cle` (valeur en syntaxe TOML) ; l'ajoute dans sa section si
    /// elle est absente. Cle inconnue : `cle inconnue: section.cle`.
    pub fn modifier(&mut self, nom: &str, valeur: &str) -> Result<(), String> {
        let cle = bornes::trouver(nom).ok_or_else(|| format!("cle inconnue: {nom}"))?;
        let v = valeur
            .parse::<Value>()
            .map_err(|e| format!("{nom}: valeur TOML illisible: {}", e.message()))?;
        let table = self.section(cle.section)?;
        poser(table, cle.cle, v);
        Ok(())
    }

    fn section(&mut self, nom: &str) -> Result<&mut Table, String> {
        if !self.doc.contains_key(nom) {
            let mut t = Table::new();
            if !self.doc.to_string().is_empty() {
                t.decor_mut().set_prefix("\n");
            }
            self.doc.insert(nom, Item::Table(t));
        }
        self.doc
            .get_mut(nom)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| format!("{nom}: une section est attendue"))
    }

    /// Liste des rappels du fichier, ou celle du modele si le fichier n'en a pas.
    pub fn rappels(&self) -> Vec<Rappel> {
        match self.doc.get("rappel").and_then(Item::as_array_of_tables) {
            Some(l) if !l.is_empty() => l.iter().map(lire_rappel).collect(),
            _ => modele_rappels(),
        }
    }

    /// Les rappels du modele deviennent ceux du fichier (premiere modification de la liste).
    fn materialiser_rappels(&mut self) {
        let vide = self
            .doc
            .get("rappel")
            .and_then(Item::as_array_of_tables)
            .map_or(true, |l| l.is_empty());
        if vide {
            let mut l = ArrayOfTables::new();
            for r in modele_rappels() {
                l.push(table_rappel(&r));
            }
            self.doc.insert("rappel", Item::ArrayOfTables(l));
        }
    }

    pub fn ajouter_rappel(&mut self, r: &Rappel) {
        self.materialiser_rappels();
        self.liste().push(table_rappel(r));
    }

    /// `n` compte a partir de 1. Le dernier rappel ne se supprime pas (le daemon en veut 1 a 16).
    pub fn supprimer_rappel(&mut self, n: usize) -> Result<(), String> {
        self.materialiser_rappels();
        let l = self.liste();
        if n == 0 || n > l.len() {
            return Err(format!("rappel {n}: inexistant ({} rappels)", l.len()));
        }
        if l.len() == 1 {
            return Err("rappel: 1 a 16 rappels, 0 donnes".into());
        }
        l.remove(n - 1);
        Ok(())
    }

    /// Remplace les champs du rappel `n` (a partir de 1) en gardant ses commentaires.
    pub fn remplacer_rappel(&mut self, n: usize, r: &Rappel) -> Result<(), String> {
        self.materialiser_rappels();
        let l = self.liste();
        if n == 0 || n > l.len() {
            return Err(format!("rappel {n}: inexistant ({} rappels)", l.len()));
        }
        let t = l.get_mut(n - 1).expect("indice verifie");
        poser(t, "categorie", Value::from(r.categorie.as_str()));
        poser(t, "heure", Value::from(r.heure.as_str()));
        if tous_les_jours(&r.jours) {
            t.remove("jours");
        } else {
            poser(t, "jours", jours_valeur(&r.jours));
        }
        Ok(())
    }

    fn liste(&mut self) -> &mut ArrayOfTables {
        self.doc
            .get_mut("rappel")
            .and_then(Item::as_array_of_tables_mut)
            .expect("liste materialisee")
    }

    /// Valide le document entier avec les regles du daemon.
    pub fn valider(&self) -> Result<(), String> {
        lire(&self.texte()).map(|_| ())
    }

    /// Valide puis ecrit (temporaire dans le meme dossier, puis renommage).
    pub fn enregistrer(&self) -> Result<(), String> {
        self.valider()?;
        ecrire_atomique(&self.chemin, &self.texte())
    }
}

fn sans_decor(v: &Value) -> String {
    let mut v = v.clone();
    v.decor_mut().clear();
    v.to_string()
}

/// Remplace la valeur de `cle` en gardant le decor (espaces, commentaire) de l'ancienne ;
/// l'ajoute en fin de table si elle est absente.
fn poser(t: &mut Table, cle: &str, mut v: Value) {
    if let Some(item) = t.get_mut(cle) {
        if let Item::Value(ancien) = item {
            *v.decor_mut() = ancien.decor().clone();
        }
        *item = Item::Value(v);
    } else {
        t.insert(cle, Item::Value(v));
    }
}

fn jours_valeur(jours: &[String]) -> Value {
    Value::Array(jours.iter().map(|j| j.as_str()).collect::<Array>())
}

fn tous_les_jours(jours: &[String]) -> bool {
    JOURS.iter().all(|j| jours.iter().any(|x| x == j))
}

fn table_rappel(r: &Rappel) -> Table {
    let mut t = Table::new();
    t.decor_mut().set_prefix("\n");
    t.insert("categorie", Item::Value(Value::from(r.categorie.as_str())));
    t.insert("heure", Item::Value(Value::from(r.heure.as_str())));
    if !tous_les_jours(&r.jours) {
        t.insert("jours", Item::Value(jours_valeur(&r.jours)));
    }
    t
}

fn lire_rappel(t: &Table) -> Rappel {
    let texte = |k: &str| t.get(k).and_then(Item::as_str).unwrap_or("").to_string();
    let jours = match t.get("jours").and_then(Item::as_array) {
        Some(a) => a.iter().filter_map(|j| j.as_str()).map(String::from).collect(),
        None => JOURS.iter().map(|j| j.to_string()).collect(),
    };
    Rappel { categorie: texte("categorie"), heure: texte("heure"), jours }
}

fn modele_rappels() -> Vec<Rappel> {
    let doc: DocumentMut = MODELE.parse().expect("le modele est du TOML");
    doc.get("rappel")
        .and_then(Item::as_array_of_tables)
        .map(|l| l.iter().map(lire_rappel).collect())
        .unwrap_or_default()
}

fn ecrire_atomique(chemin: &Path, texte: &str) -> Result<(), String> {
    let dossier = match chemin.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    fs::create_dir_all(&dossier)
        .map_err(|e| format!("creation de {}: {e}", dossier.display()))?;
    let nom = chemin.file_name().and_then(|n| n.to_str()).unwrap_or("osampo.toml");
    let tmp = dossier.join(format!(".{nom}.{}.tmp", std::process::id()));
    let ecrit = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(texte.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, chemin)
    })();
    ecrit.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("ecriture de {}: {e}", chemin.display())
    })
}

/// Lignes `--lister` : `onglet<TAB>section.cle<TAB>valeur<TAB>bornes`, suivies avec `aide` de
/// `<TAB>libelle<TAB>aide` (SPEC-010).
pub fn lignes_lister(ed: &Edition, aide: bool) -> Vec<String> {
    let suite = |libelle: &str, texte: &str| {
        if aide { format!("\t{libelle}\t{texte}") } else { String::new() }
    };
    let mut out: Vec<String> = bornes::CLES
        .iter()
        .map(|c| {
            format!(
                "{}\t{}\t{}\t{}{}",
                c.onglet.nom(),
                c.nom(),
                ed.valeur(c),
                c.bornes,
                suite(c.libelle, c.aide)
            )
        })
        .collect();
    for (i, r) in ed.rappels().iter().enumerate() {
        let n = i + 1;
        let jours = sans_decor(&jours_valeur(&r.jours));
        let (c, h, j) = (&bornes::RAPPEL_CATEGORIE, &bornes::RAPPEL_HEURE, &bornes::RAPPEL_JOURS);
        out.push(format!(
            "temps\trappel.{n}.categorie\t\"{}\"\trepas, sortie, sport ou dodo{}",
            r.categorie,
            suite(c.libelle, c.aide)
        ));
        out.push(format!(
            "temps\trappel.{n}.heure\t\"{}\"\tHH:MM, 00:00 à 23:59{}",
            r.heure,
            suite(h.libelle, h.aide)
        ));
        out.push(format!(
            "temps\trappel.{n}.jours\t{jours}\tau moins un jour parmi lun..dim{}",
            suite(j.libelle, j.aide)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(t: &str) -> Edition {
        Edition::depuis_texte(t, Path::new("/tmp/x.toml")).unwrap()
    }

    #[test]
    fn modele_valide_et_cinq_rappels() {
        let e = ed(MODELE);
        e.valider().unwrap();
        assert_eq!(e.rappels().len(), 5);
        assert_eq!(e.rappels()[2].jours, ["lun", "mer", "ven"]);
        assert_eq!(e.rappels()[0].jours.len(), 7);
    }

    #[test]
    fn toml_illisible_refuse() {
        assert!(Edition::depuis_texte("pas [du toml", Path::new("x")).is_err());
    }

    #[test]
    fn modification_conserve_la_mise_en_forme() {
        let t = "# mon fichier\n\n[rappels]   # section\nreport_min   =   12   # douze\n\n[heures]\ncalme_debut = \"22:00\"\n\n# fin\n";
        let mut e = ed(t);
        e.modifier("heures.calme_debut", "\"23:00\"").unwrap();
        assert_eq!(e.texte(), t.replace("\"22:00\"", "\"23:00\""));
        e.modifier("rappels.report_min", "15").unwrap();
        assert!(e.texte().contains("report_min   =   15   # douze\n"), "{}", e.texte());
    }

    #[test]
    fn cle_absente_ajoutee_et_section_creee() {
        let mut e = ed("[rappels]\nreport_min = 10\n");
        e.modifier("rappels.rattrapage_min", "30").unwrap();
        e.modifier("heures.calme_debut", "\"23:00\"").unwrap();
        assert_eq!(
            e.texte(),
            "[rappels]\nreport_min = 10\nrattrapage_min = 30\n\n[heures]\ncalme_debut = \"23:00\"\n"
        );
        assert_eq!(e.valeur(bornes::trouver("heures.calme_fin").unwrap()), "\"07:00\"");
    }

    #[test]
    fn cle_inconnue_et_valeur_illisible() {
        let mut e = ed("");
        assert_eq!(e.modifier("rappels.nimporte", "3").unwrap_err(), "cle inconnue: rappels.nimporte");
        assert!(e.modifier("rappels.report_min", "[pas").is_err());
    }

    #[test]
    fn validation_du_daemon() {
        let mut e = ed(MODELE);
        e.modifier("rappels.report_min", "200").unwrap();
        assert!(e.valider().unwrap_err().contains("rappels.report_min: 200 hors de 1..120"));
        let mut e = ed(MODELE);
        e.modifier("promenade.longue_min", "4").unwrap();
        assert!(e.valider().is_err());
    }

    #[test]
    fn poste_devient_promenade() {
        let t = "[rappels]\nreport_min = 10\n\n[poste]\n# duree avant la laisse\nlaisse_apres_min = 80\npause_min = 4   # courte\n";
        let mut e = ed(t);
        e.modifier("rappels.report_min", "11").unwrap();
        assert_eq!(
            e.texte(),
            "[rappels]\nreport_min = 11\n\n[promenade]\n# duree avant la laisse\nvide_apres_min = 80\ncourte_min = 4   # courte\n"
        );
        e.valider().unwrap();
        assert_eq!(e.valeur(bornes::trouver("promenade.vide_apres_min").unwrap()), "80");
    }

    #[test]
    fn poste_et_promenade_refuses() {
        let e = ed("[promenade]\nvide_apres_min = 90\n\n[poste]\nlaisse_apres_min = 80\n");
        assert!(e.valider().unwrap_err().contains("[poste] et [promenade]"));
    }

    #[test]
    fn rappels_ajout_suppression_modification() {
        let mut e = ed("[rappels]\nreport_min = 10\n");
        assert_eq!(e.rappels().len(), 5);
        e.ajouter_rappel(&Rappel::nouveau("sport", "08:00"));
        assert_eq!(e.rappels().len(), 6);
        e.valider().unwrap();
        e.supprimer_rappel(1).unwrap();
        assert_eq!(e.rappels()[0].categorie, "sortie");
        let mut r = e.rappels()[0].clone();
        r.heure = "13:00".into();
        r.jours = vec!["sam".into(), "dim".into()];
        e.remplacer_rappel(1, &r).unwrap();
        e.valider().unwrap();
        assert_eq!(e.rappels()[0], r);
        assert!(e.supprimer_rappel(9).is_err());
    }

    #[test]
    fn dernier_rappel_et_17e_refuses() {
        let mut e = ed("[[rappel]]\ncategorie = \"repas\"\nheure = \"12:00\"\n");
        assert!(e.supprimer_rappel(1).is_err());
        for _ in 0..16 {
            e.ajouter_rappel(&Rappel::nouveau("repas", "12:00"));
        }
        assert!(e.valider().unwrap_err().contains("rappel"));
    }

    #[test]
    fn lister_cles_et_rappels() {
        let l = lignes_lister(&ed(MODELE), false);
        assert!(l.contains(&"temps\trappels.report_min\t10\t1 à 120 min".to_string()));
        assert!(l.contains(&"temps\trappel.1.categorie\t\"repas\"\trepas, sortie, sport ou dodo".to_string()));
        assert!(l.iter().any(|x| x.starts_with("temps\trappel.3.jours\t[\"lun\", \"mer\", \"ven\"]\t")));
        assert!(l.iter().all(|x| x.split('\t').count() == 4));
    }

    #[test]
    fn lister_avec_aide_ajoute_libelle_et_aide() {
        let l = lignes_lister(&ed(MODELE), true);
        assert!(l.iter().all(|x| x.split('\t').count() == 6), "{l:?}");
        let k = bornes::trouver("rappels.report_min").unwrap();
        assert!(l.contains(&format!("temps\trappels.report_min\t10\t1 à 120 min\t{}\t{}", k.libelle, k.aide)));
        let r = l.iter().find(|x| x.starts_with("temps\trappel.2.jours\t")).unwrap();
        assert!(r.ends_with(&format!("\t{}\t{}", bornes::RAPPEL_JOURS.libelle, bornes::RAPPEL_JOURS.aide)));
    }

    #[test]
    fn ecriture_atomique() {
        let d = std::env::temp_dir().join(format!("osampo-edition-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let p = d.join("sous").join("t.toml");
        let mut e = Edition::charger(&p).unwrap();
        e.modifier("rappels.report_min", "15").unwrap();
        e.enregistrer().unwrap();
        assert!(fs::read_to_string(&p).unwrap().contains("report_min = 15"));
        assert_eq!(fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&d);
    }
}
