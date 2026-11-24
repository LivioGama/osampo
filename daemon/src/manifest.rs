use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

fn refresh_ms_par_defaut() -> u64 {
    500
}

fn stale_s_par_defaut() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeySpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub values: Vec<String>,
}

/// Le manifeste d un widget, lu par le daemon et par personne d autre.
/// Le firmware n analyse aucun manifeste : c est le §7 du design de cadrage.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    // contrat du manifeste (design de cadrage §7), lu par aucun code du daemon a ce jour
    #[allow(dead_code)]
    pub formats: Vec<String>,
    #[serde(default = "refresh_ms_par_defaut")]
    pub refresh_ms: u64,
    #[serde(default = "stale_s_par_defaut")]
    pub stale_s: u64,
    #[serde(default)]
    pub keys: Vec<KeySpec>,
}

impl Manifest {
    /// Charge widgets/<id>/widget.toml. L identifiant vient du reseau : il est
    /// valide avant d etre transforme en chemin, sinon un « ../.. » ferait
    /// sortir du repertoire des widgets.
    pub fn load(dir: &Path, id: &str) -> Result<Manifest> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            anyhow::bail!("identifiant de widget invalide: {id:?}");
        }
        let chemin = dir.join(id).join("widget.toml");
        let texte = std::fs::read_to_string(&chemin)
            .with_context(|| format!("manifeste introuvable: {}", chemin.display()))?;
        let m: Manifest = toml::from_str(&texte)
            .with_context(|| format!("manifeste illisible: {}", chemin.display()))?;
        if m.id != id {
            anyhow::bail!("le manifeste {} declare id = {:?}, attendu {:?}", chemin.display(), m.id, id);
        }
        Ok(m)
    }

    /// Les noms de cles declares, age compris : le daemon l ajoute toujours.
    pub fn key_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.keys.iter().map(|k| k.name.clone()).collect();
        if !v.iter().any(|n| n == "age") {
            v.push("age".to_string());
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn ecrire_manifeste(racine: &Path, id: &str, contenu: &str) {
        let d = racine.join(id);
        std::fs::create_dir_all(&d).unwrap();
        let mut f = std::fs::File::create(d.join("widget.toml")).unwrap();
        f.write_all(contenu.as_bytes()).unwrap();
    }

    fn racine_temporaire(nom: &str) -> osampo_config::essai::DossierTemp {
        osampo_config::essai::DossierTemp::new(&format!("manifest_{nom}"))
    }

    #[test]
    fn lit_un_manifeste_complet() {
        let racine = racine_temporaire("complet");
        ecrire_manifeste(
            &racine,
            "demo3",
            r#"
id = "demo3"
formats = ["bar", "square"]
refresh_ms = 500

[[keys]]
name = "value"
type = "float"

[[keys]]
name = "state"
type = "string"
values = ["ok", "warn", "alert", "offline"]
"#,
        );
        let m = Manifest::load(&racine, "demo3").unwrap();
        assert_eq!(m.id, "demo3");
        assert_eq!(m.refresh_ms, 500);
        assert_eq!(m.stale_s, 30, "stale_s absent doit prendre son defaut");
        assert!(m.key_names().contains(&"age".to_string()), "age est toujours declare");
    }

    #[test]
    fn refuse_un_manifeste_absent() {
        let racine = racine_temporaire("absent");
        let e = Manifest::load(&racine, "inconnu").unwrap_err();
        assert!(format!("{e}").contains("manifeste introuvable"), "message obtenu: {e}");
    }

    #[test]
    fn refuse_un_identifiant_qui_sort_du_repertoire() {
        let racine = racine_temporaire("evasion");
        assert!(Manifest::load(&racine, "../../etc").is_err());
        assert!(Manifest::load(&racine, "Majuscule").is_err());
        assert!(Manifest::load(&racine, "").is_err());
    }

    #[test]
    fn refresh_ms_absent_prend_son_defaut() {
        let racine = racine_temporaire("defaut_refresh");
        ecrire_manifeste(&racine, "demo3", "id = \"demo3\"\nformats = [\"bar\"]\n");
        let m = Manifest::load(&racine, "demo3").unwrap();
        assert_eq!(m.refresh_ms, 500, "refresh_ms absent doit prendre son defaut");
    }

    #[test]
    fn refuse_un_toml_invalide() {
        let racine = racine_temporaire("toml_invalide");
        ecrire_manifeste(&racine, "demo3", "id = [pas du toml");
        let e = Manifest::load(&racine, "demo3").unwrap_err();
        assert!(format!("{e}").contains("manifeste illisible"), "message obtenu: {e}");
    }

    #[test]
    fn refuse_un_id_different_de_celui_demande() {
        let racine = racine_temporaire("id_different");
        ecrire_manifeste(&racine, "demo3", "id = \"autre\"\nformats = [\"bar\"]\n");
        let e = Manifest::load(&racine, "demo3").unwrap_err();
        let msg = format!("{e}");
        assert!(msg.contains("declare id") && msg.contains("autre") && msg.contains("demo3"), "message obtenu: {msg}");
    }
}
