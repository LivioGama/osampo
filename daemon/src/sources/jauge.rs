//! Jauge « sage » du chiot (SP5h) : se vide pendant l'activite au PC, se
//! recharge apres une absence d'au moins `courte_min` (pleine a `longue_min`).
//! Logique pure : l'horloge est passee en secondes (epoch, pour survivre a un
//! redemarrage du daemon et compter une veille du PC comme absence).

/// Absence comptee des qu'aucune entree X depuis ce nombre de secondes.
pub const ABSENT_DES_S: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReglagesJauge {
    pub vide_apres_min: u64,
    pub courte_min: u64,
    pub longue_min: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Etat {
    pub sage: i64,
    pub actif: bool,
    pub absence_min: i64,
    /// Derniere absence terminee : duree et minutes depuis sa fin (-1 : aucune).
    /// La board juge ainsi une promenade faite hors de portee, PC en veille.
    pub absence_finie_min: i64,
    pub absence_finie_age_min: i64,
}

#[derive(Debug)]
pub struct Jauge {
    /// Niveau 0..100 ; pendant une absence, niveau a son debut.
    niveau: f64,
    debut_absence: Option<u64>,
    dernier: Option<u64>,
    /// (debut, fin) de la derniere absence terminee ; non persistee.
    finie: Option<(u64, u64)>,
}

fn borne(x: f64) -> f64 {
    x.clamp(0.0, 100.0)
}

fn recharge(duree_s: u64, r: &ReglagesJauge) -> f64 {
    if duree_s < r.courte_min * 60 {
        0.0
    } else {
        100.0 * duree_s as f64 / (r.longue_min.max(1) * 60) as f64
    }
}

impl Jauge {
    pub fn new(niveau: f64) -> Self {
        Self::restaurer(niveau, None)
    }

    pub fn restaurer(niveau: f64, debut_absence: Option<u64>) -> Self {
        Jauge { niveau: borne(niveau), debut_absence, dernier: None, finie: None }
    }

    pub fn niveau(&self) -> f64 {
        self.niveau
    }

    pub fn debut_absence(&self) -> Option<u64> {
        self.debut_absence
    }

    /// Echantillon manque (X illisible) : le temps avance sans creer de trou de
    /// veille, pour que la panne ne compte pas comme absence au retour de X.
    pub fn sans_lecture(&mut self, now_s: u64) {
        if self.debut_absence.is_none() {
            self.dernier = Some(now_s);
        }
    }

    /// Reglage depuis le PC (SPEC-009) : la jauge vaut `valeur` a `now_s`. La baisse depuis
    /// l echantillon precedent ne s applique pas a la valeur reglee ; `absent` : une absence
    /// est en cours, elle compte comme commencee a `now_s`. La derniere absence terminee
    /// (`finie`) n est pas touchee.
    pub fn regler(&mut self, valeur: f64, now_s: u64, absent: bool) {
        self.niveau = borne(valeur);
        self.dernier = Some(now_s);
        self.debut_absence = absent.then_some(now_s);
    }

    fn finie(&self, now_s: u64) -> (i64, i64) {
        match self.finie {
            Some((d, f)) => (((f - d) / 60) as i64, (now_s.saturating_sub(f) / 60) as i64),
            None => (0, -1),
        }
    }

    /// idle_s : inactivite X courante ; now_s : epoch en secondes.
    pub fn echantillon(&mut self, idle_s: u64, now_s: u64, r: &ReglagesJauge) -> Etat {
        let precedent = self.dernier.replace(now_s);
        // Trou plus long qu'une promenade courte (PC en veille, daemon arrete) :
        // l'absence a commence au plus tard au dernier echantillon.
        let trou = precedent.filter(|p| now_s.saturating_sub(*p) > r.courte_min * 60);
        if let Some(p) = trou {
            let d = self.debut_absence.get_or_insert(p);
            *d = (*d).min(p);
        }
        if idle_s >= ABSENT_DES_S {
            let debut = *self.debut_absence.get_or_insert(now_s.saturating_sub(idle_s));
            let duree = now_s.saturating_sub(debut);
            let (absence_finie_min, absence_finie_age_min) = self.finie(now_s);
            return Etat {
                sage: borne(self.niveau + recharge(duree, r)).round() as i64,
                actif: false,
                absence_min: (duree / 60) as i64,
                absence_finie_min,
                absence_finie_age_min,
            };
        }
        // Actif : une absence en cours se termine a now - idle.
        let fin = now_s.saturating_sub(idle_s);
        let depuis = if let Some(d) = self.debut_absence.take() {
            self.niveau = borne(self.niveau + recharge(fin.saturating_sub(d), r));
            self.finie = Some((d.min(fin), fin));
            fin
        } else {
            precedent.unwrap_or(now_s)
        };
        let vidage = 100.0 / (r.vide_apres_min.max(1) * 60) as f64;
        self.niveau = borne(self.niveau - vidage * now_s.saturating_sub(depuis) as f64);
        let (absence_finie_min, absence_finie_age_min) = self.finie(now_s);
        Etat { sage: self.niveau.round() as i64, actif: true, absence_min: 0, absence_finie_min, absence_finie_age_min }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: ReglagesJauge = ReglagesJauge { vide_apres_min: 90, courte_min: 5, longue_min: 30 };
    const T0: u64 = 1_800_000_000;

    /// Echantillons toutes les 30 s pendant `min` minutes ; `absent` : idle croit.
    fn derouler(j: &mut Jauge, t: &mut u64, min: u64, absent: bool) -> Etat {
        let debut = *t;
        let mut e = j.echantillon(0, *t, &R);
        for _ in 0..(min * 2) {
            *t += 30;
            e = j.echantillon(if absent { *t - debut } else { 0 }, *t, &R);
        }
        e
    }

    #[test]
    fn vide_en_90_min() {
        let mut j = Jauge::new(100.0);
        let mut t = T0;
        assert_eq!(derouler(&mut j, &mut t, 45, false).sage, 50);
        assert_eq!(derouler(&mut j, &mut t, 45, false).sage, 0);
        assert_eq!(derouler(&mut j, &mut t, 30, false).sage, 0);
    }

    #[test]
    fn absence_courte_ne_recharge_pas() {
        let mut j = Jauge::new(0.0);
        let mut t = T0;
        derouler(&mut j, &mut t, 4, true);
        t += 30;
        let e = j.echantillon(0, t, &R);
        assert_eq!(e.sage, 0);
        assert!(e.actif);
    }

    #[test]
    fn quinze_minutes_rechargent_la_moitie() {
        let mut j = Jauge::new(0.0);
        let mut t = T0;
        assert_eq!(derouler(&mut j, &mut t, 15, true).sage, 50);
        t += 1;
        assert_eq!(j.echantillon(0, t, &R).sage, 50);
    }

    #[test]
    fn trente_minutes_rechargent_tout() {
        let mut j = Jauge::new(60.0);
        let mut t = T0;
        assert_eq!(derouler(&mut j, &mut t, 30, true).sage, 100);
        t += 1;
        assert_eq!(j.echantillon(0, t, &R).sage, 100);
    }

    #[test]
    fn absence_min_compte_en_direct() {
        let mut j = Jauge::new(50.0);
        let mut t = T0;
        let e = derouler(&mut j, &mut t, 7, true);
        assert_eq!(e.absence_min, 7);
        assert!(!e.actif);
        t += 1;
        let e = j.echantillon(0, t, &R);
        assert_eq!(e.absence_min, 0);
        assert!(e.actif);
    }

    #[test]
    fn trou_de_veille_vaut_absence() {
        let mut j = Jauge::new(0.0);
        j.echantillon(0, T0, &R);
        assert_eq!(j.echantillon(0, T0 + 20 * 60, &R).sage, 67);
    }

    #[test]
    fn restauree_reprend_l_absence() {
        let mut j = Jauge::new(0.0);
        let mut t = T0;
        derouler(&mut j, &mut t, 10, true);
        let mut r = Jauge::restaurer(j.niveau(), j.debut_absence());
        t += 5 * 60;
        let e = r.echantillon(t - j.debut_absence().unwrap(), t, &R);
        assert_eq!(e.absence_min, 15);
        assert_eq!(e.sage, 50);
    }

    /// C1 : l'absence qui vient de se terminer reste lisible (duree, age), pour
    /// juger une promenade faite hors de portee, PC en veille.
    #[test]
    fn absence_finie_reste_lisible() {
        let mut j = Jauge::new(50.0);
        let mut t = T0;
        let e = j.echantillon(0, t, &R);
        assert_eq!((e.absence_finie_min, e.absence_finie_age_min), (0, -1));
        t += 30 * 60; // veille du PC pendant la promenade
        let e = j.echantillon(0, t, &R);
        assert_eq!((e.absence_finie_min, e.absence_finie_age_min), (30, 0));
        t += 3 * 60;
        let e = j.echantillon(0, t, &R);
        assert_eq!((e.absence_finie_min, e.absence_finie_age_min), (30, 3));
    }

    /// I3 : un echantillon manque (X illisible) n'est pas un trou de veille.
    #[test]
    fn sans_lecture_ne_fait_pas_de_trou() {
        let mut j = Jauge::new(0.0);
        let mut t = T0;
        j.echantillon(0, t, &R);
        for _ in 0..80 {
            t += 30;
            j.sans_lecture(t);
        }
        t += 30;
        let e = j.echantillon(0, t, &R);
        assert_eq!(e.sage, 0);
        assert_eq!(e.absence_finie_age_min, -1);
    }

    /// SPEC-009 : apres un reglage, la baisse repart de la valeur reglee, sans rattraper le
    /// temps ecoule depuis l'echantillon precedent.
    #[test]
    fn regler_puis_activite_baisse_depuis_la_valeur() {
        let mut j = Jauge::new(100.0);
        let mut t = T0;
        derouler(&mut j, &mut t, 10, false);
        t += 20 * 60; // longue pause entre deux echantillons
        j.regler(80.0, t, false);
        let e = j.echantillon(0, t, &R);
        assert_eq!(e.sage, 80);
        assert!(e.actif);
        // 9 min a 100/90 par min = 10 points
        let e = derouler(&mut j, &mut t, 9, false);
        assert_eq!(e.sage, 70);
    }

    #[test]
    fn regler_en_absence_recommence_l_absence_au_reglage() {
        let mut j = Jauge::new(100.0);
        let mut t = T0;
        derouler(&mut j, &mut t, 10, true); // 10 min d'absence deja ecoulees
        j.regler(0.0, t, true);
        assert_eq!(j.debut_absence(), Some(t));
        // l'inactivite X, elle, remonte a avant le reglage : seule la duree depuis le reglage compte
        t += 15 * 60;
        let e = j.echantillon(25 * 60, t, &R);
        assert_eq!((e.sage, e.absence_min), (50, 15));
    }

    #[test]
    fn regler_garde_la_derniere_absence_finie() {
        let mut j = Jauge::new(50.0);
        j.echantillon(0, T0, &R);
        let t = T0 + 30 * 60;
        j.echantillon(0, t, &R); // absence finie de 30 min
        j.regler(10.0, t + 60, false);
        let e = j.echantillon(0, t + 60, &R);
        assert_eq!((e.absence_finie_min, e.absence_finie_age_min), (30, 1));
        assert_eq!(e.sage, 10);
    }

    #[test]
    fn regler_borne_la_valeur_et_sans_absence_efface_l_absence() {
        let mut j = Jauge::restaurer(20.0, Some(T0));
        j.regler(150.0, T0 + 5, false);
        assert_eq!((j.niveau(), j.debut_absence()), (100.0, None));
        j.regler(-3.0, T0 + 6, false);
        assert_eq!(j.niveau(), 0.0);
    }
}
