//! Client du socket du daemon (SPEC-009) : réglage de la jauge `sage` et suivi des lignes
//! d'état. Seule exception à « le programme ne parle pas au daemon » (SPEC-007).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Nouvel essai de connexion quand le daemon est injoignable.
pub const DELAI_ESSAI: Duration = Duration::from_secs(5);
const DELAI_REPONSE: Duration = Duration::from_secs(5);
const HELLO: &str = "{\"hello\":\"osampo-pc\"}\n";

extern "C" {
    fn getuid() -> u32;
}

/// Le socket par défaut du daemon (même règle que `osampo-pc`).
pub fn socket_par_defaut() -> PathBuf {
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(d) if !d.is_empty() => PathBuf::from(d).join("osampo.sock"),
        _ => PathBuf::from(format!("/tmp/osampo-{}", unsafe { getuid() }))
            .join("osampo.sock"),
    }
}

#[derive(Debug, PartialEq)]
pub enum ErreurJauge {
    Injoignable,
    /// Texte de `erreur` de la réponse du daemon.
    Daemon(String),
}

impl ErreurJauge {
    pub fn texte(&self) -> String {
        match self {
            ErreurJauge::Injoignable => "daemon injoignable".into(),
            ErreurJauge::Daemon(m) => m.clone(),
        }
    }
}

/// Entier de 0 à 100 écrit en chiffres (sans signe, sans espace, sans décimale).
pub fn valeur_entiere(t: &str) -> Option<u8> {
    if t.is_empty() || t.len() > 3 || !t.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse::<u8>().ok().filter(|v| *v <= 100)
}

/// Argument de `--jauge` : `remplir`, `vider` ou un entier de 0 à 100.
pub fn parser_valeur(t: &str) -> Result<u8, String> {
    match t {
        "remplir" => Ok(100),
        "vider" => Ok(0),
        _ => valeur_entiere(t).ok_or_else(|| "valeur hors de 0..100".to_string()),
    }
}

pub fn ligne_jauge(valeur: u8) -> String {
    serde_json::json!({"cmd": "jauge", "valeur": valeur}).to_string()
}

/// `sage` d'une ligne d'état du daemon ; `None` pour toute autre ligne.
pub fn lire_sage(ligne: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(ligne).ok()?;
    v.get("pc")?.get("sage")?.as_i64()
}

/// `Some(réponse)` si la ligne est une `reponse` du daemon.
fn lire_reponse(ligne: &str) -> Option<Result<(), ErreurJauge>> {
    let v: serde_json::Value = serde_json::from_str(ligne).ok()?;
    v.get("reponse")?;
    if v.get("ok").and_then(|o| o.as_bool()) == Some(true) {
        return Some(Ok(()));
    }
    let m = v.get("erreur").and_then(|e| e.as_str()).unwrap_or("erreur inconnue");
    Some(Err(ErreurJauge::Daemon(m.to_string())))
}

/// Texte du cadre `Jauge sage` : valeur courante, ou `Daemon : injoignable`.
pub fn texte_etat(sage: Option<i64>) -> String {
    match sage {
        Some(s) => format!("Sage : {s} %"),
        None => "Daemon : injoignable".into(),
    }
}

/// Envoie la jauge et lit la réponse ; les lignes d'état reçues avant elle sont sautées.
pub fn envoyer_jauge(socket: &Path, valeur: u8) -> Result<(), ErreurJauge> {
    let mut flux = UnixStream::connect(socket).map_err(|_| ErreurJauge::Injoignable)?;
    let _ = flux.set_read_timeout(Some(DELAI_REPONSE));
    let _ = flux.set_write_timeout(Some(DELAI_REPONSE));
    let envoi = [HELLO, &ligne_jauge(valeur), "\n"].concat();
    flux.write_all(envoi.as_bytes()).map_err(|_| ErreurJauge::Injoignable)?;
    let mut lecture = BufReader::new(flux);
    let mut ligne = String::new();
    loop {
        ligne.clear();
        match lecture.read_line(&mut ligne) {
            Ok(0) | Err(_) => return Err(ErreurJauge::Injoignable),
            Ok(_) => {
                if let Some(r) = lire_reponse(&ligne) {
                    return r;
                }
            }
        }
    }
}

/// Ce que le fil de suivi rapporte à la fenêtre.
#[derive(Debug, PartialEq)]
pub enum Evt {
    Sage(i64),
    Injoignable,
}

/// Suit les lignes d'état dans un fil à part ; `rapporter` rend `false` quand la fenêtre est
/// partie (le fil s'arrête). Daemon injoignable ou connexion perdue : nouvel essai toutes les 5 s.
pub fn suivre<F>(socket: PathBuf, rapporter: F)
where
    F: Fn(Evt) -> bool + Send + 'static,
{
    std::thread::spawn(move || loop {
        if let Ok(mut flux) = UnixStream::connect(&socket) {
            if flux.write_all(HELLO.as_bytes()).is_ok() {
                for l in BufReader::new(flux).lines().map_while(Result::ok) {
                    if let Some(s) = lire_sage(&l) {
                        if !rapporter(Evt::Sage(s)) {
                            return;
                        }
                    }
                }
            }
        }
        if !rapporter(Evt::Injoignable) {
            return;
        }
        std::thread::sleep(DELAI_ESSAI);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn valeurs_acceptees_et_refusees() {
        assert_eq!(parser_valeur("remplir"), Ok(100));
        assert_eq!(parser_valeur("vider"), Ok(0));
        assert_eq!(parser_valeur("0"), Ok(0));
        assert_eq!(parser_valeur("42"), Ok(42));
        assert_eq!(parser_valeur("100"), Ok(100));
        for t in ["-1", "101", "abc", "42.5", "", "REMPLIR", "+5", " 5", "1000", "99999999999"] {
            assert_eq!(parser_valeur(t), Err("valeur hors de 0..100".into()), "{t:?}");
        }
    }

    #[test]
    fn lignes_du_daemon() {
        assert_eq!(ligne_jauge(42), r#"{"cmd":"jauge","valeur":42}"#);
        assert_eq!(lire_sage(r#"{"pc":{"etat":"repos","sage":72}}"#), Some(72));
        assert_eq!(lire_sage(r#"{"reponse":"jauge","ok":true}"#), None);
        assert_eq!(texte_etat(Some(72)), "Sage : 72 %");
        assert_eq!(texte_etat(None), "Daemon : injoignable");
    }

    fn faux_daemon(nom: &str, reponse: &'static str) -> (PathBuf, std::thread::JoinHandle<String>) {
        let d = std::env::temp_dir().join(format!("osampo-daemon-{}-{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("s.sock");
        let l = UnixListener::bind(&p).unwrap();
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            let (mut hello, mut cmd) = (String::new(), String::new());
            r.read_line(&mut hello).unwrap();
            r.read_line(&mut cmd).unwrap();
            // Une ligne d'état avant la réponse : elle doit être sautée.
            s.write_all(b"{\"pc\":{\"sage\":10}}\n").unwrap();
            s.write_all([reponse, "\n"].concat().as_bytes()).unwrap();
            [hello, cmd].concat()
        });
        (p, h)
    }

    #[test]
    fn envoi_ok_saute_les_lignes_d_etat() {
        let (p, h) = faux_daemon("ok", r#"{"reponse":"jauge","ok":true}"#);
        assert_eq!(envoyer_jauge(&p, 42), Ok(()));
        let recu = h.join().unwrap();
        assert_eq!(recu, "{\"hello\":\"osampo-pc\"}\n{\"cmd\":\"jauge\",\"valeur\":42}\n");
    }

    #[test]
    fn envoi_erreur_du_daemon_et_injoignable() {
        let (p, h) = faux_daemon("err", r#"{"reponse":"jauge","ok":false,"erreur":"jauge indisponible"}"#);
        assert_eq!(envoyer_jauge(&p, 1), Err(ErreurJauge::Daemon("jauge indisponible".into())));
        h.join().unwrap();
        let absent = Path::new("/tmp/absent-osampo.sock");
        assert_eq!(envoyer_jauge(absent, 1), Err(ErreurJauge::Injoignable));
    }
}
