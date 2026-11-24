//! Logique du chiot cote daemon pour les programmes du PC (SP6) : rappels tenus par le
//! daemon (SPEC-003) et etat publie (SPEC-002). Logique pure, sans E/S reseau : l heure est
//! injectee (`Heure`). Cable au socket et au transport par la tache « pc » de main.rs.
pub mod etat;
pub mod rappels;

/// Un instant, en heure locale : jour entier (compte de jours locaux depuis 1970-01-01),
/// minute du jour 0..1439, et l epoch pour les durees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heure {
    pub epoch_s: i64,
    pub jour: i64,
    pub minute: i64,
}

extern "C" {
    // Absent du crate libc pour Linux ; declare par time.h (POSIX).
    fn tzset();
}

/// Decalage local (secondes est de UTC) a l instant `epoch_s`.
fn decalage_local_s(epoch_s: i64) -> i64 {
    let t = epoch_s as libc::time_t;
    // SAFETY: tzset n a pas de precondition ; tm est rempli par localtime_r a partir de t et
    // n est lu que si l appel rend non nul.
    unsafe {
        tzset();
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        tm.tm_gmtoff as i64
    }
}

impl Heure {
    /// Instant a partir de l epoch et d un decalage donne (tests, horloge injectee).
    pub fn avec_decalage(epoch_s: i64, decalage_s: i64) -> Heure {
        let local = epoch_s + decalage_s;
        Heure {
            epoch_s,
            jour: local.div_euclid(86400),
            minute: local.rem_euclid(86400) / 60,
        }
    }

    /// Instant en heure locale du systeme.
    pub fn locale(epoch_s: i64) -> Heure {
        Heure::avec_decalage(epoch_s, decalage_local_s(epoch_s))
    }

    pub fn hhmm(&self) -> String {
        format!("{:02}:{:02}", self.minute / 60, self.minute % 60)
    }
}

/// Jour de la semaine d un numero de jour, 0 = lundi (1970-01-01 etait un jeudi).
pub fn semaine_de(jour: i64) -> usize {
    (jour + 3).rem_euclid(7) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heure_et_semaine() {
        // 2026-11-22 est un dimanche ; 14:12 a UTC+1.
        let h = Heure::avec_decalage(1_795_353_120, 3600);
        assert_eq!(h.hhmm(), "14:12");
        assert_eq!(semaine_de(h.jour), 6);
        assert_eq!(semaine_de(0), 3);
        assert_eq!(Heure::avec_decalage(-60, 0).minute, 23 * 60 + 59);
    }
}
