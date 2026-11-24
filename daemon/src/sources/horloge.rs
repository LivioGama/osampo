use crate::model::{Keys, Value};
use crate::source::Source;
use anyhow::Result;
use async_trait::async_trait;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

extern "C" {
    // Absent du crate libc pour Linux ; declare par time.h (POSIX).
    fn tzset();
}

/// Decalage local courant (secondes est de UTC) a l instant epoch_s,
/// via localtime_r / tm_gmtoff : suit l heure d ete sans redemarrage.
/// POSIX ne garantit pas que localtime_r relise TZ et /etc/localtime :
/// tzset() d abord, pour suivre un changement de fuseau en cours de route.
fn decalage_local_s(epoch_s: i64) -> i64 {
    let t: libc::time_t = epoch_s as libc::time_t;
    // SAFETY: tzset n a pas de precondition. tm est une structure C de plain
    // data, remplie par localtime_r (version reentrante) a partir de t ; on ne
    // lit que si elle rend non nul.
    unsafe {
        tzset();
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        tm.tm_gmtoff as i64
    }
}

/// epoch et tz_s, relus a chaque poll (toutes les secondes) : la valeur emise
/// a au plus ~1 s de retard sur l horloge systeme. Apres une veille, l epoch
/// d avant la veille (vieux de plusieurs heures) ne reste donc emis qu environ
/// une seconde, le temps du premier poll de la reprise (M4 : une board
/// relancee a la reprise se cale presque aussitot sur l heure courante).
pub struct SourceHorloge;

#[async_trait]
impl Source for SourceHorloge {
    fn id(&self) -> &str {
        "horloge"
    }

    fn keys(&self) -> &'static [&'static str] {
        &["epoch", "tz_s"]
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(1)
    }

    async fn poll(&mut self) -> Result<Keys> {
        let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let mut k = Keys::new();
        k.insert("epoch".into(), Value::Int(epoch));
        k.insert("tz_s".into(), Value::Int(decalage_local_s(epoch)));
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decalage de `date +%z` (+hhmm / -hhmm) en secondes.
    fn decalage_date() -> i64 {
        let sortie = std::process::Command::new("date").arg("+%z").output().unwrap();
        let s = String::from_utf8(sortie.stdout).unwrap();
        let s = s.trim();
        let signe = if s.starts_with('-') { -1 } else { 1 };
        let h: i64 = s[1..3].parse().unwrap();
        let m: i64 = s[3..5].parse().unwrap();
        signe * (h * 3600 + m * 60)
    }

    #[tokio::test]
    async fn tz_s_egal_au_decalage_de_date() {
        let mut s = SourceHorloge;
        let k = s.poll().await.unwrap();
        assert_eq!(k.get("tz_s"), Some(&Value::Int(decalage_date())));
        match k.get("epoch") {
            Some(Value::Int(e)) => assert!(*e > 1_700_000_000, "epoch implausible: {e}"),
            autre => panic!("epoch doit etre un entier, obtenu {autre:?}"),
        }
        assert_eq!(s.keys(), &["epoch", "tz_s"]);
        assert_eq!(s.id(), "horloge");
        assert_eq!(s.interval(), Duration::from_secs(1));
    }
}
