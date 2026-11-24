use crate::model::{Keys, Value};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// Une source produit des cles a son propre rythme.
///
/// poll() est asynchrone a dessein : en SP4, une source fera un appel reseau
/// de plusieurs secondes. Une signature synchrone bloquerait l executeur et
/// figerait l emission pour tous les widgets.
#[async_trait]
pub trait Source: Send {
    fn id(&self) -> &str;

    /// Les cles que cette source pretend produire. Sert au controle croise
    /// avec le manifeste au demarrage, avant la premiere trame.
    fn keys(&self) -> &'static [&'static str];

    fn interval(&self) -> Duration;

    async fn poll(&mut self) -> Result<Keys>;

    /// Reveil hors intervalle : un `notify_one` dessus fait poller la source tout de suite
    /// (SPEC-009 : un reglage de la jauge doit arriver en moins de 2 s). Aucun par defaut.
    fn reveil(&self) -> Option<Arc<Notify>> {
        None
    }
}

/// Le jumeau Rust du FakeSource C++ de SP2 : meme loi, memes seuils.
/// En SP4, les sources reelles prennent sa place sans changer le planificateur.
pub struct FakeSource {
    value: f32,
    step: f32,
    frame: u64,
}

impl FakeSource {
    pub fn new() -> FakeSource {
        FakeSource { value: 0.0, step: 4.0, frame: 0 }
    }
}

impl Default for FakeSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for FakeSource {
    fn id(&self) -> &str {
        "fake"
    }

    fn keys(&self) -> &'static [&'static str] {
        &["value", "label", "state"]
    }

    fn interval(&self) -> Duration {
        Duration::from_millis(500)
    }

    async fn poll(&mut self) -> Result<Keys> {
        self.value += self.step;
        if self.value >= 100.0 {
            self.value = 100.0;
            self.step = -self.step;
        } else if self.value <= 0.0 {
            self.value = 0.0;
            self.step = -self.step;
        }

        let etat = if self.value < 60.0 {
            "ok"
        } else if self.value < 85.0 {
            "warn"
        } else {
            "alert"
        };

        let mut k = Keys::new();
        k.insert(
            "value".into(),
            Value::float(self.value as f64).expect("la loi du generateur reste finie"),
        );
        k.insert("label".into(), Value::Text(format!("trame {}", self.frame)));
        k.insert("state".into(), Value::Text(etat.into()));
        self.frame += 1;
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn la_valeur_monte_puis_redescend() {
        let mut s = FakeSource::new();
        let mut vues = Vec::new();
        for _ in 0..60 {
            let k = s.poll().await.unwrap();
            match k.get("value").unwrap() {
                Value::Float(v) => vues.push(*v),
                autre => panic!("value doit etre un flottant, obtenu {autre:?}"),
            }
        }
        assert!(vues.iter().all(|v| (0.0..=100.0).contains(v)), "hors bornes: {vues:?}");
        assert!(vues.iter().any(|v| *v >= 100.0), "le va-et-vient doit toucher 100");
        assert!(vues.windows(2).any(|w| w[1] < w[0]), "le va-et-vient doit redescendre");
    }

    #[tokio::test]
    async fn l_etat_suit_les_seuils() {
        // La loi monte par pas de 4 : ces valeurs sont atteintes exactement.
        // Attendus ecrits en dur, aux bornes 60 et 85 (85 n est pas atteint,
        // 84 et 88 l encadrent).
        let attendus: [(f64, &str); 8] = [
            (4.0, "ok"),
            (56.0, "ok"),
            (60.0, "warn"),
            (64.0, "warn"),
            (84.0, "warn"),
            (88.0, "alert"),
            (96.0, "alert"),
            (100.0, "alert"),
        ];
        let mut s = FakeSource::new();
        let mut vus = Vec::new();
        for _ in 0..25 {
            let k = s.poll().await.unwrap();
            let v = match k.get("value").unwrap() {
                Value::Float(v) => *v,
                _ => unreachable!(),
            };
            let etat = match k.get("state").unwrap() {
                Value::Text(t) => t.clone(),
                _ => unreachable!(),
            };
            vus.push((v, etat));
        }
        for (v, attendu) in attendus {
            let trouve = vus.iter().find(|(x, _)| *x == v);
            let (_, etat) = trouve.unwrap_or_else(|| panic!("valeur {v} jamais produite: {vus:?}"));
            assert_eq!(etat, attendu, "a la valeur {v}");
        }
    }

    #[tokio::test]
    async fn keys_dit_la_verite_sur_ce_qui_est_produit() {
        let mut s = FakeSource::new();
        let declarees: Vec<&str> = s.keys().to_vec();
        let produites = s.poll().await.unwrap();
        for d in &declarees {
            assert!(produites.contains_key(*d), "cle declaree non produite: {d}");
        }
        assert_eq!(produites.len(), declarees.len(), "cle produite non declaree");
    }
}
