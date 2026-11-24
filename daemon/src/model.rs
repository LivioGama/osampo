use serde::Serialize;
use std::collections::BTreeMap;

/// Une valeur produite par une source. Les trois types du contrat de widget.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Float(f64),
    Int(i64),
    Text(String),
}

impl Value {
    /// Une valeur flottante non finie ne doit jamais entrer dans le modele :
    /// serde_json refuse de serialiser NaN et Inf, et une source SP4 qui
    /// divise par zero casserait la trame de tout le monde.
    pub fn float(v: f64) -> Option<Value> {
        if v.is_finite() {
            Some(Value::Float(v))
        } else {
            None
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Float(_) => "float",
            Value::Int(_) => "int",
            Value::Text(_) => "string",
        }
    }
}

/// Les cles rendues par une source en un appel a poll().
pub type Keys = BTreeMap<String, Value>;

/// Une trame prete a emettre, deja filtree pour une connexion donnee.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Frame(pub Keys);

impl Frame {
    /// Rend la ligne a ecrire sur le transport, saut de ligne compris.
    ///
    /// L invariant « aucune valeur non finie » est tenu par Value::float et
    /// verifie par le planificateur : la serialisation ne peut donc pas echouer.
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(&self.0)
            .expect("aucune valeur non finie ne peut entrer dans une Frame");
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les quatre cles du contrat, avec des valeurs typiques.
    fn trame_type() -> Keys {
        let mut k = Keys::new();
        k.insert("value".into(), Value::Float(62.5));
        k.insert("label".into(), Value::Text("trame 12".into()));
        k.insert("state".into(), Value::Text("ok".into()));
        k.insert("age".into(), Value::Int(0));
        k
    }

    #[test]
    fn aller_retour_sur_une_trame() {
        let ligne = Frame(trame_type()).to_line();
        assert!(ligne.ends_with('\n'), "la ligne doit finir par un saut de ligne");

        let relu: serde_json::Value = serde_json::from_str(ligne.trim_end()).unwrap();
        assert_eq!(relu["value"], 62.5);
        assert_eq!(relu["label"], "trame 12");
        assert_eq!(relu["state"], "ok");
        assert_eq!(relu["age"], 0);
    }

    #[test]
    fn la_ligne_tient_sous_la_limite_du_protocole() {
        let ligne = Frame(trame_type()).to_line();
        assert!(
            ligne.len() <= 4096,
            "la ligne serialisee fait {} octets, limite 4096",
            ligne.len()
        );
    }

    #[test]
    fn une_valeur_non_finie_est_refusee_a_la_construction() {
        assert!(Value::float(f64::NAN).is_none());
        assert!(Value::float(f64::INFINITY).is_none());
        assert_eq!(Value::float(1.5), Some(Value::Float(1.5)));
    }
}
