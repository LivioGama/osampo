use crate::manifest::Manifest;
use crate::model::{Frame, Keys, Value};
use crate::source::Source;
use anyhow::Result;
use std::collections::BTreeMap;
use std::time::Instant;

struct Sample {
    value: Value,
    at: Instant,
}

/// Garde le dernier etat de chaque cle et fabrique une trame complete.
///
/// Une source lente ne bloque pas une source rapide : chacune ecrit dans cet
/// etat a son rythme, et l emission se fait a la cadence du manifeste.
pub struct Scheduler {
    manifest: Manifest,
    state: BTreeMap<String, Sample>,
}

impl Scheduler {
    pub fn new(manifest: Manifest) -> Scheduler {
        Scheduler { manifest, state: BTreeMap::new() }
    }

    /// Controle croise entre ce que les sources pretendent produire et ce que
    /// le manifeste declare. Fait au demarrage, pas au premier decodage.
    ///
    /// Rend la liste des avertissements ; une incoherence bloquante est une
    /// erreur. La cle age est fournie par le planificateur, pas par une source.
    pub fn check_sources(&self, sources: &[Box<dyn Source>]) -> Result<Vec<String>> {
        let mut avertissements = Vec::new();
        let mut produite_par: BTreeMap<&str, &str> = BTreeMap::new();

        for s in sources {
            for k in s.keys() {
                if let Some(autre) = produite_par.insert(k, s.id()) {
                    anyhow::bail!(
                        "la cle {k:?} est produite par deux sources, {autre:?} et {:?}",
                        s.id()
                    );
                }
            }
        }

        for nom in self.manifest.key_names() {
            if nom == "age" {
                continue;
            }
            if !produite_par.contains_key(nom.as_str()) {
                anyhow::bail!(
                    "le manifeste {:?} declare la cle {nom:?}, qu aucune source ne produit",
                    self.manifest.id
                );
            }
        }

        let declarees = self.manifest.key_names();
        for k in produite_par.keys() {
            if !declarees.iter().any(|d| d == k) {
                avertissements.push(format!(
                    "la cle {k:?} est produite mais absente du manifeste {:?}, elle ne sera pas emise",
                    self.manifest.id
                ));
            }
        }

        Ok(avertissements)
    }

    /// Integre ce qu une source vient de rendre. Une valeur non finie, d un autre
    /// type que celui declare, ou hors de la liste values du manifeste est
    /// refusee : la cle garde la precedente.
    pub fn merge(&mut self, keys: Keys, now: Instant) {
        for (nom, valeur) in keys {
            if let Value::Float(v) = &valeur {
                if !v.is_finite() {
                    eprintln!("valeur non finie refusee pour la cle {nom:?}");
                    continue;
                }
            }
            if let Some(spec) = self.manifest.keys.iter().find(|k| k.name == nom) {
                if spec.ty != valeur.type_name() {
                    eprintln!(
                        "valeur de type {} refusee pour la cle {nom:?}, declaree {}",
                        valeur.type_name(),
                        spec.ty
                    );
                    continue;
                }
                if !spec.values.is_empty() {
                    if let Value::Text(t) = &valeur {
                        if !spec.values.contains(t) {
                            eprintln!("valeur {t:?} hors de la liste declaree pour {nom:?}, ignoree");
                            continue;
                        }
                    }
                }
            }
            self.state.insert(nom, Sample { value: valeur, at: now });
        }
    }

    /// Rend la trame a emettre, ou None tant que toutes les cles declarees
    /// n ont pas recu une valeur au moins une fois.
    ///
    /// Sans ce None, le daemon emettrait une trame sans value au demarrage et
    /// le widget afficherait une jauge a zero indistinguable d une vraie.
    pub fn snapshot(&self, now: Instant) -> Option<Frame> {
        let mut sortie = Keys::new();
        let mut plus_vieux: Option<Instant> = None;

        for nom in self.manifest.key_names() {
            if nom == "age" {
                continue;
            }
            let sample = self.state.get(&nom)?;
            plus_vieux = Some(match plus_vieux {
                Some(p) if p <= sample.at => p,
                _ => sample.at,
            });
            sortie.insert(nom, sample.value.clone());
        }

        let age = plus_vieux
            .map(|p| now.saturating_duration_since(p).as_secs() as i64)
            .unwrap_or(0);
        sortie.insert("age".into(), Value::Int(age));

        Some(Frame(sortie))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{KeySpec, Manifest};
    use crate::source::FakeSource;
    use std::time::Duration;

    fn manifeste() -> Manifest {
        Manifest {
            id: "demo3".into(),
            formats: vec!["bar".into()],
            refresh_ms: 500,
            stale_s: 30,
            keys: vec![
                KeySpec { name: "value".into(), ty: "float".into(), values: vec![] },
                KeySpec { name: "label".into(), ty: "string".into(), values: vec![] },
                KeySpec {
                    name: "state".into(),
                    ty: "string".into(),
                    values: vec!["ok".into(), "warn".into(), "alert".into(), "offline".into()],
                },
            ],
        }
    }

    fn cles(value: f64, label: &str, state: &str) -> Keys {
        let mut k = Keys::new();
        k.insert("value".into(), Value::Float(value));
        k.insert("label".into(), Value::Text(label.into()));
        k.insert("state".into(), Value::Text(state.into()));
        k
    }

    #[test]
    fn rien_n_est_emis_avant_que_toutes_les_cles_existent() {
        let mut s = Scheduler::new(manifeste());
        let t0 = Instant::now();
        assert!(s.snapshot(t0).is_none(), "demarrage a froid : aucune trame");

        let mut partiel = Keys::new();
        partiel.insert("value".into(), Value::Float(10.0));
        s.merge(partiel, t0);
        assert!(s.snapshot(t0).is_none(), "trame incomplete : toujours rien");

        s.merge(cles(10.0, "trame 0", "ok"), t0);
        assert!(s.snapshot(t0).is_some(), "toutes les cles presentes : une trame sort");
    }

    #[test]
    fn deux_sources_a_rythmes_differents_fusionnent() {
        let mut s = Scheduler::new(manifeste());
        let t0 = Instant::now();
        s.merge(cles(10.0, "trame 0", "ok"), t0);

        // une source rapide met a jour value seule, 10 secondes plus tard
        let t1 = t0 + Duration::from_secs(10);
        let mut rapide = Keys::new();
        rapide.insert("value".into(), Value::Float(80.0));
        s.merge(rapide, t1);

        let f = s.snapshot(t1).unwrap();
        assert_eq!(f.0.get("value"), Some(&Value::Float(80.0)), "la cle fraiche est a jour");
        assert_eq!(
            f.0.get("label"),
            Some(&Value::Text("trame 0".into())),
            "la cle non mise a jour garde sa valeur"
        );
        assert_eq!(
            f.0.get("age"),
            Some(&Value::Int(10)),
            "age est celui du plus vieil echantillon, pas du plus jeune"
        );
    }

    #[test]
    fn age_croit_sans_nouvel_echantillon_jusqu_a_la_peremption() {
        // Moitie daemon de la preuve de peremption : une source figee ne
        // merge plus rien, et age doit depasser stale_s de lui-meme.
        let m = manifeste();
        let stale_s = m.stale_s;
        let mut s = Scheduler::new(m);
        let t0 = Instant::now();
        s.merge(cles(10.0, "trame 0", "ok"), t0);

        let f = s.snapshot(t0 + Duration::from_secs(stale_s + 2)).unwrap();
        match f.0.get("age") {
            Some(Value::Int(age)) => assert!(
                *age > stale_s as i64,
                "age {age} doit depasser stale_s {stale_s} sans nouvel echantillon"
            ),
            autre => panic!("age absent ou mal type: {autre:?}"),
        }
    }

    #[test]
    fn une_valeur_non_finie_ne_remplace_pas_la_precedente() {
        let mut s = Scheduler::new(manifeste());
        let t0 = Instant::now();
        s.merge(cles(42.0, "trame 0", "ok"), t0);

        let mut pourri = Keys::new();
        pourri.insert("value".into(), Value::Float(f64::NAN));
        s.merge(pourri, t0);

        let f = s.snapshot(t0).unwrap();
        assert_eq!(f.0.get("value"), Some(&Value::Float(42.0)));
        // la trame reste serialisable : c est tout l enjeu
        assert!(f.to_line().contains("42"));
    }

    #[test]
    fn une_valeur_hors_liste_est_ignoree() {
        let mut s = Scheduler::new(manifeste());
        let t0 = Instant::now();
        s.merge(cles(42.0, "trame 0", "ok"), t0);

        let mut pourri = Keys::new();
        pourri.insert("state".into(), Value::Text("fantaisie".into()));
        s.merge(pourri, t0);

        let f = s.snapshot(t0).unwrap();
        assert_eq!(f.0.get("state"), Some(&Value::Text("ok".into())));
    }

    #[test]
    fn une_valeur_du_mauvais_type_ne_remplace_pas_la_precedente() {
        let mut s = Scheduler::new(manifeste());
        let t0 = Instant::now();
        s.merge(cles(42.0, "trame 0", "ok"), t0);

        // value est declaree float, label string : les types croises sont refuses
        let mut pourri = Keys::new();
        pourri.insert("value".into(), Value::Text("quarante".into()));
        pourri.insert("label".into(), Value::Int(7));
        s.merge(pourri, t0);

        let f = s.snapshot(t0).unwrap();
        assert_eq!(f.0.get("value"), Some(&Value::Float(42.0)));
        assert_eq!(f.0.get("label"), Some(&Value::Text("trame 0".into())));
    }

    #[test]
    fn le_controle_croise_refuse_une_cle_que_personne_ne_produit() {
        struct Muette;
        #[async_trait::async_trait]
        impl Source for Muette {
            fn id(&self) -> &str { "muette" }
            fn keys(&self) -> &'static [&'static str] { &["value"] }
            fn interval(&self) -> Duration { Duration::from_millis(500) }
            async fn poll(&mut self) -> Result<Keys> { Ok(Keys::new()) }
        }
        let s = Scheduler::new(manifeste());
        let sources: Vec<Box<dyn Source>> = vec![Box::new(Muette)];
        let e = s.check_sources(&sources).unwrap_err();
        assert!(format!("{e}").contains("qu aucune source ne produit"), "message: {e}");
    }

    #[test]
    fn le_controle_croise_refuse_deux_sources_sur_la_meme_cle() {
        let s = Scheduler::new(manifeste());
        let sources: Vec<Box<dyn Source>> =
            vec![Box::new(FakeSource::new()), Box::new(FakeSource::new())];
        let e = s.check_sources(&sources).unwrap_err();
        assert!(format!("{e}").contains("produite par deux sources"), "message: {e}");
    }

    #[test]
    fn le_controle_croise_accepte_le_generateur_factice() {
        let s = Scheduler::new(manifeste());
        let sources: Vec<Box<dyn Source>> = vec![Box::new(FakeSource::new())];
        assert!(s.check_sources(&sources).unwrap().is_empty());
    }

    #[test]
    fn une_cle_produite_non_declaree_donne_un_avertissement() {
        struct Bavarde;
        #[async_trait::async_trait]
        impl Source for Bavarde {
            fn id(&self) -> &str { "bavarde" }
            fn keys(&self) -> &'static [&'static str] { &["value", "label", "state", "extra"] }
            fn interval(&self) -> Duration { Duration::from_millis(500) }
            async fn poll(&mut self) -> Result<Keys> { Ok(Keys::new()) }
        }
        let s = Scheduler::new(manifeste());
        let sources: Vec<Box<dyn Source>> = vec![Box::new(Bavarde)];
        let w = s.check_sources(&sources).unwrap();
        assert!(!w.is_empty(), "un avertissement est attendu");
        assert!(w.iter().any(|m| m.contains("extra")), "il doit nommer la cle: {w:?}");
    }
}
