use crate::model::{Keys, Value};
use crate::source::Source;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Un fichier de session plus vieux que cela est ignore (spec D7) : une
/// session morte sans SessionEnd ne doit pas faire aboyer le chiot a jamais.
const PEREMPTION_S: u64 = 7200;

/// Duree minimale du 0 force quand une session entre en attente alors que
/// l agregat vaut deja 1 : plus long que le tick de 1 s de la board, qui
/// re-arme sa patte en voyant passer 0.
const FRONT: Duration = Duration::from_secs(2);

/// D7 : $XDG_RUNTIME_DIR/osampo/claude_wait, repli /tmp/osampo-daemon-$UID/claude_wait.
/// Le booleen dit si c est le repli /tmp (a verifier avant lecture).
pub fn repertoire_claude() -> (PathBuf, bool) {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(x) if !x.is_empty() => {
            (PathBuf::from(x).join("osampo").join("claude_wait"), false)
        }
        _ => (PathBuf::from(format!("/tmp/osampo-daemon-{}", uid_courant())).join("claude_wait"), true),
    }
}

fn uid_courant() -> u32 {
    // SAFETY: getuid n a aucune precondition et ne peut pas echouer.
    unsafe { libc::getuid() }
}

/// Repli /tmp : le repertoire et son parent doivent appartenir a uid et ne pas
/// etre des liens symboliques (un autre utilisateur pourrait les avoir crees
/// avant nous). Un repertoire absent est accepte : il ne donne que 0.
pub fn repertoire_sur(dir: &Path, uid: u32) -> std::result::Result<(), String> {
    for p in [dir.parent(), Some(dir)].into_iter().flatten() {
        let meta = match std::fs::symlink_metadata(p) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", p.display())),
        };
        if meta.file_type().is_symlink() {
            return Err(format!("{} est un lien symbolique", p.display()));
        }
        if !meta.is_dir() {
            return Err(format!("{} n est pas un repertoire", p.display()));
        }
        if meta.uid() != uid {
            return Err(format!("{} appartient a l uid {}, pas a {uid}", p.display(), meta.uid()));
        }
    }
    Ok(())
}

/// Sessions qui valent "1", non perimees, avec leur mtime. Les fichiers dont
/// le nom commence par "." (temporaires du hook) et les non-fichiers sont
/// ignores. Repertoire absent ou illisible : aucune.
pub fn sessions_en_attente(dir: &Path, maintenant: SystemTime) -> HashMap<OsString, SystemTime> {
    let mut vues = HashMap::new();
    let Ok(entrees) = std::fs::read_dir(dir) else {
        return vues;
    };
    for e in entrees.flatten() {
        let nom = e.file_name();
        if nom.as_encoded_bytes().first() == Some(&b'.') {
            continue;
        }
        // DirEntry::metadata ne suit pas les liens : un lien n est pas un fichier.
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let mtime = meta.modified().unwrap_or(maintenant);
        // Un mtime dans le futur compte comme age 0 (horloge reglee en arriere).
        let age = maintenant.duration_since(mtime).map_or(0, |d| d.as_secs());
        if age >= PEREMPTION_S {
            continue;
        }
        if std::fs::read_to_string(e.path()).is_ok_and(|c| c.trim() == "1") {
            vues.insert(nom, mtime);
        }
    }
    vues
}

/// 1 si au moins un fichier de session vaut "1" et date de moins de 7200 s.
#[cfg(test)]
pub fn etat_claude(dir: &Path, maintenant: SystemTime) -> i64 {
    i64::from(!sessions_en_attente(dir, maintenant).is_empty())
}

/// Logique pure du front : l agregat est le OU des sessions, mais une session
/// qui entre en attente (nouvelle a 1, ou mtime plus recent) alors que
/// l agregat valait deja 1 force 0 pendant FRONT, puis 1. La board re-arme
/// ainsi sa patte a chaque nouvelle attente (I2).
#[derive(Default)]
pub struct Fronts {
    precedentes: HashMap<OsString, SystemTime>,
    dernier: i64,
    zero_jusqu_a: Option<Instant>,
}

impl Fronts {
    pub fn pas(&mut self, sessions: HashMap<OsString, SystemTime>, now: Instant) -> i64 {
        let nouvelle = sessions
            .iter()
            .any(|(nom, m)| self.precedentes.get(nom).is_none_or(|p| m > p));
        if nouvelle && self.dernier == 1 {
            self.zero_jusqu_a = Some(now + FRONT);
        }
        self.precedentes = sessions;
        let agregat = i64::from(!self.precedentes.is_empty());
        let valeur = match self.zero_jusqu_a {
            Some(t) if now < t && agregat == 1 => 0,
            _ => {
                self.zero_jusqu_a = None;
                agregat
            }
        };
        self.dernier = valeur;
        valeur
    }
}

pub struct SourceClaude {
    dir: PathBuf,
    repli_tmp: bool,
    fronts: Fronts,
    repli_signale: bool,
}

impl SourceClaude {
    pub fn new() -> Self {
        let (dir, repli_tmp) = repertoire_claude();
        SourceClaude::avec(dir, repli_tmp)
    }

    fn avec(dir: PathBuf, repli_tmp: bool) -> Self {
        SourceClaude { dir, repli_tmp, fronts: Fronts::default(), repli_signale: false }
    }
}

impl Default for SourceClaude {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for SourceClaude {
    fn id(&self) -> &str {
        "claude"
    }

    fn keys(&self) -> &'static [&'static str] {
        &["claude_wait"]
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(2)
    }

    async fn poll(&mut self) -> Result<Keys> {
        let sur = if self.repli_tmp {
            match repertoire_sur(&self.dir, uid_courant()) {
                Ok(()) => {
                    self.repli_signale = false;
                    true
                }
                Err(raison) => {
                    if !self.repli_signale {
                        eprintln!("claude: repertoire refuse ({raison}), claude_wait = 0");
                        self.repli_signale = true;
                    }
                    false
                }
            }
        } else {
            true
        };
        let sessions =
            if sur { sessions_en_attente(&self.dir, SystemTime::now()) } else { HashMap::new() };
        let mut k = Keys::new();
        k.insert("claude_wait".into(), Value::Int(self.fronts.pas(sessions, Instant::now())));
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Repertoire temporaire unique par test, nettoye a la fin.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new(nom: &str) -> Tmp {
            let p = std::env::temp_dir().join(format!("claude_test_{}_{nom}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn aucun_repertoire_rend_0() {
        let d = std::env::temp_dir().join(format!("claude_absent_{}", std::process::id()));
        assert_eq!(etat_claude(&d, SystemTime::now()), 0);
    }

    #[test]
    fn une_session_qui_attend_rend_1() {
        let t = Tmp::new("une");
        fs::write(t.0.join("abc"), "1").unwrap();
        assert_eq!(etat_claude(&t.0, SystemTime::now()), 1);
    }

    #[test]
    fn deux_sessions_une_attend_rend_1() {
        let t = Tmp::new("deux");
        fs::write(t.0.join("abc"), "0").unwrap();
        fs::write(t.0.join("def"), "1").unwrap();
        assert_eq!(etat_claude(&t.0, SystemTime::now()), 1);
    }

    #[test]
    fn fichier_perime_ignore() {
        let t = Tmp::new("perime");
        fs::write(t.0.join("abc"), "1").unwrap();
        let futur = SystemTime::now() + Duration::from_secs(7201);
        assert_eq!(etat_claude(&t.0, futur), 0);
        let juste_avant = SystemTime::now() + Duration::from_secs(7100);
        assert_eq!(etat_claude(&t.0, juste_avant), 1);
    }

    #[test]
    fn fichier_0_rend_0() {
        let t = Tmp::new("zero");
        fs::write(t.0.join("abc"), "0").unwrap();
        assert_eq!(etat_claude(&t.0, SystemTime::now()), 0);
    }

    #[tokio::test]
    async fn la_source_produit_claude_wait() {
        let t = Tmp::new("source");
        fs::write(t.0.join("abc"), "1").unwrap();
        let mut s = SourceClaude::avec(t.0.clone(), false);
        let k = s.poll().await.unwrap();
        assert_eq!(k.get("claude_wait"), Some(&Value::Int(1)));
        assert_eq!(s.keys(), &["claude_wait"]);
        assert_eq!(s.id(), "claude");
        assert_eq!(s.interval(), Duration::from_secs(2));
    }

    #[test]
    fn fichier_cache_du_hook_ignore() {
        let t = Tmp::new("cache");
        fs::write(t.0.join(".abc.123"), "1").unwrap();
        assert_eq!(etat_claude(&t.0, SystemTime::now()), 0);
    }

    fn sessions(t: &Tmp) -> HashMap<OsString, SystemTime> {
        sessions_en_attente(&t.0, SystemTime::now())
    }

    #[test]
    fn deuxieme_session_en_attente_donne_un_0_avant_le_1() {
        let t = Tmp::new("front");
        let mut f = Fronts::default();
        let t0 = Instant::now();
        fs::write(t.0.join("abc"), "1").unwrap();
        let mut suite = vec![f.pas(sessions(&t), t0), f.pas(sessions(&t), t0 + FRONT)];
        fs::write(t.0.join("def"), "1").unwrap();
        for i in 2..=5 {
            suite.push(f.pas(sessions(&t), t0 + FRONT * i));
        }
        assert_eq!(suite, vec![1, 1, 0, 1, 1, 1]);
    }

    #[test]
    fn mtime_plus_recent_rearme_aussi() {
        let t = Tmp::new("mtime");
        let mut f = Fronts::default();
        let t0 = Instant::now();
        let p = t.0.join("abc");
        fs::write(&p, "1").unwrap();
        let avant = SystemTime::now() - Duration::from_secs(60);
        fs::File::options().write(true).open(&p).unwrap().set_modified(avant).unwrap();
        assert_eq!(f.pas(sessions(&t), t0), 1);
        assert_eq!(f.pas(sessions(&t), t0 + FRONT), 1);
        // Meme session : 0 puis 1 entre deux polls, le fichier est reecrit.
        fs::File::options().write(true).open(&p).unwrap().set_modified(SystemTime::now()).unwrap();
        assert_eq!(f.pas(sessions(&t), t0 + FRONT * 2), 0);
        assert_eq!(f.pas(sessions(&t), t0 + FRONT * 3), 1);
    }

    #[test]
    fn premiere_attente_sans_agregat_ne_retarde_pas() {
        let t = Tmp::new("premiere");
        let mut f = Fronts::default();
        let t0 = Instant::now();
        assert_eq!(f.pas(sessions(&t), t0), 0);
        fs::write(t.0.join("abc"), "1").unwrap();
        assert_eq!(f.pas(sessions(&t), t0 + FRONT), 1);
    }

    #[test]
    fn repli_tmp_refuse_lien_et_autre_proprietaire() {
        let t = Tmp::new("repli");
        let vrai = t.0.join("esp32").join("claude_wait");
        fs::create_dir_all(&vrai).unwrap();
        let uid = uid_courant();
        assert_eq!(repertoire_sur(&vrai, uid), Ok(()));
        assert!(repertoire_sur(&vrai, uid + 1).is_err(), "autre proprietaire");
        let lien = t.0.join("lien");
        std::os::unix::fs::symlink(t.0.join("esp32"), &lien).unwrap();
        assert!(repertoire_sur(&lien.join("claude_wait"), uid).is_err(), "parent lien");
        assert_eq!(repertoire_sur(&t.0.join("absent").join("claude_wait"), uid), Ok(()));
    }

    #[tokio::test]
    async fn repli_tmp_refuse_rend_0() {
        let t = Tmp::new("repli_source");
        let lien = t.0.join("lien");
        fs::create_dir_all(t.0.join("vrai").join("claude_wait")).unwrap();
        fs::write(t.0.join("vrai").join("claude_wait").join("abc"), "1").unwrap();
        std::os::unix::fs::symlink(t.0.join("vrai"), &lien).unwrap();
        let mut s = SourceClaude::avec(lien.join("claude_wait"), true);
        assert_eq!(s.poll().await.unwrap().get("claude_wait"), Some(&Value::Int(0)));
        let mut s = SourceClaude::avec(t.0.join("vrai").join("claude_wait"), true);
        assert_eq!(s.poll().await.unwrap().get("claude_wait"), Some(&Value::Int(1)));
    }
}
