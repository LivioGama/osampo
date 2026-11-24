//! Mode ligne de commande de `osampo-reglages` : `--lister` et `--set` (SPEC-007), `--jauge`
//! (SPEC-009).

use std::io::Write;
use std::path::PathBuf;

use crate::daemon::{self, ErreurJauge};
use crate::edition::{lignes_lister, Edition};

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub config: Option<PathBuf>,
    pub lister: bool,
    pub aide: bool,
    pub sets: Vec<(String, String)>,
    pub socket: Option<PathBuf>,
    /// Argument de `--jauge`, pas encore valide.
    pub jauge: Option<String>,
}

impl Args {
    /// Vrai si l'un des modes ligne de commande est demande (sinon : interface graphique).
    pub fn en_ligne(&self) -> bool {
        self.lister || !self.sets.is_empty() || self.jauge.is_some()
    }
}

pub fn parser<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.into_iter();
    while let Some(x) = it.next() {
        match x.as_str() {
            "--config" => {
                a.config = Some(it.next().ok_or("--config attend un chemin")?.into());
            }
            "--socket" => {
                a.socket = Some(it.next().ok_or("--socket attend un chemin")?.into());
            }
            "--jauge" => {
                a.jauge = Some(it.next().ok_or("--jauge attend remplir, vider ou un entier")?);
            }
            "--lister" => a.lister = true,
            "--aide" => a.aide = true,
            "--set" => {
                let s = it.next().ok_or("--set attend section.cle=valeur")?;
                let (nom, valeur) = s
                    .split_once('=')
                    .ok_or_else(|| format!("--set {s}: section.cle=valeur attendu"))?;
                a.sets.push((nom.trim().to_string(), valeur.trim().to_string()));
            }
            autre => return Err(format!("argument inconnu: {autre}")),
        }
    }
    if a.lister && !a.sets.is_empty() {
        return Err("--lister et --set s'excluent".into());
    }
    if a.jauge.is_some() && (a.lister || !a.sets.is_empty()) {
        return Err("--jauge exclut --lister et --set".into());
    }
    if a.aide && !a.lister {
        return Err("--aide s'utilise avec --lister".into());
    }
    Ok(a)
}

/// `--jauge` : 0 si `ok`, 2 si valeur refusee ou erreur du daemon, 1 si daemon injoignable.
fn lancer_jauge(a: &Args, valeur: &str, err: &mut dyn Write) -> i32 {
    let v = match daemon::parser_valeur(valeur) {
        Ok(v) => v,
        Err(m) => {
            let _ = writeln!(err, "jauge: {m}");
            return 2;
        }
    };
    let socket = a.socket.clone().unwrap_or_else(daemon::socket_par_defaut);
    match daemon::envoyer_jauge(&socket, v) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "jauge: {}", e.texte());
            if e == ErreurJauge::Injoignable {
                1
            } else {
                2
            }
        }
    }
}

/// Execute `--lister`, `--set` ou `--jauge` ; rend le code de sortie (0, 1 ou 2).
pub fn lancer(a: &Args, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if let Some(v) = &a.jauge {
        return lancer_jauge(a, v, err);
    }
    match executer(a, out) {
        Ok(()) => 0,
        Err(m) => {
            let _ = writeln!(err, "{m}");
            2
        }
    }
}

fn executer(a: &Args, out: &mut dyn Write) -> Result<(), String> {
    let chemin = a.config.clone().unwrap_or_else(osampo_config::chemin_par_defaut);
    let mut ed = Edition::charger(&chemin)?;
    if a.lister {
        for l in lignes_lister(&ed, a.aide) {
            writeln!(out, "{l}").map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    for (nom, valeur) in &a.sets {
        ed.modifier(nom, valeur)?;
    }
    ed.enregistrer()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parse_les_options() {
        let a = parser(s(&["--config", "f", "--set", "a.b=1", "--set", "c.d=\"x=y\""])).unwrap();
        assert_eq!(a.config, Some("f".into()));
        assert_eq!(a.sets, [("a.b".into(), "1".into()), ("c.d".into(), "\"x=y\"".into())]);
        assert!(a.en_ligne());
        assert!(!parser(s(&[])).unwrap().en_ligne());
        assert!(parser(s(&["--set", "sans_egal"])).is_err());
        assert!(parser(s(&["--lister", "--set", "a.b=1"])).is_err());
        assert!(parser(s(&["--nimporte"])).is_err());
        let a = parser(s(&["--lister", "--aide"])).unwrap();
        assert!(a.lister && a.aide);
        assert!(parser(s(&["--aide"])).unwrap_err().contains("--lister"));
        assert!(parser(s(&["--aide", "--set", "a.b=1"])).is_err());
    }

    #[test]
    fn parse_jauge() {
        let a = parser(s(&["--socket", "x.sock", "--jauge", "remplir"])).unwrap();
        assert_eq!(a.socket, Some("x.sock".into()));
        assert_eq!(a.jauge.as_deref(), Some("remplir"));
        assert!(a.en_ligne());
        assert_eq!(parser(s(&["--jauge", "-1"])).unwrap().jauge.as_deref(), Some("-1"));
        assert!(parser(s(&["--jauge"])).is_err());
        assert!(parser(s(&["--jauge", "vider", "--lister"])).is_err());
        assert!(parser(s(&["--set", "a.b=1", "--jauge", "vider"])).is_err());
    }

    #[test]
    fn jauge_codes_de_sortie() {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let a = Args {
            jauge: Some("101".into()),
            socket: Some("/tmp/absent-t.sock".into()),
            ..Args::default()
        };
        assert_eq!(lancer(&a, &mut o, &mut e), 2);
        assert_eq!(String::from_utf8(std::mem::take(&mut e)).unwrap(), "jauge: valeur hors de 0..100\n");
        let a = Args { jauge: Some("vider".into()), ..a };
        assert_eq!(lancer(&a, &mut o, &mut e), 1);
        assert_eq!(String::from_utf8(e).unwrap(), "jauge: daemon injoignable\n");
        assert!(o.is_empty());
    }

    #[test]
    fn set_invalide_code_2_fichier_inchange() {
        let d = std::env::temp_dir().join(format!("osampo-cli-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("t.toml");
        std::fs::write(&p, "[rappels]\nreport_min = 10\n").unwrap();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let a = Args {
            config: Some(p.clone()),
            sets: vec![("rappels.report_min".into(), "200".into())],
            ..Args::default()
        };
        assert_eq!(lancer(&a, &mut o, &mut e), 2);
        assert!(String::from_utf8(e).unwrap().contains("rappels.report_min: 200 hors de 1..120"));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "[rappels]\nreport_min = 10\n");
        let a = Args {
            config: Some(p.clone()),
            sets: vec![("rappels.report_min".into(), "15".into())],
            ..Args::default()
        };
        assert_eq!(lancer(&a, &mut o, &mut Vec::new()), 0);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "[rappels]\nreport_min = 15\n");
        let _ = std::fs::remove_dir_all(&d);
    }
}
