//! Position retenue du chien posé sur la barre (SPEC-011) : `droite_px`, la distance entre le
//! bord droit de la fenêtre et le bord droit de la zone de pose. Module pur, sans X11.
//! Les rectangles sont `[x, y, largeur, hauteur]`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `$XDG_STATE_HOME/osampo-pc/position.toml`, repli `~/.local/state/...`.
pub fn chemin_dans(xdg_state_home: Option<OsString>, home: OsString) -> PathBuf {
    let base = match xdg_state_home {
        Some(x) if !x.is_empty() => PathBuf::from(x),
        _ => PathBuf::from(home).join(".local").join("state"),
    };
    base.join("osampo-pc").join("position.toml")
}

pub fn chemin() -> PathBuf {
    chemin_dans(
        std::env::var_os("XDG_STATE_HOME"),
        std::env::var_os("HOME").unwrap_or_default(),
    )
}

/// Valeur de `droite_px = <entier>` (commentaires et lignes étrangères ignorés).
fn analyser(texte: &str) -> Option<i64> {
    texte.lines().find_map(|l| {
        let l = l.split('#').next()?.trim();
        let (cle, val) = l.split_once('=')?;
        (cle.trim() == "droite_px").then(|| val.trim().parse::<i64>().ok())?
    })
}

/// `droite_px` du fichier ; absent, illisible ou négatif → 0.
pub fn lire_depuis(chemin: &Path) -> i64 {
    std::fs::read_to_string(chemin)
        .ok()
        .and_then(|t| analyser(&t))
        .filter(|n| *n >= 0)
        .unwrap_or(0)
}

pub fn lire() -> i64 {
    lire_depuis(&chemin())
}

/// Écriture atomique (temporaire puis renommage), dossier créé au besoin.
pub fn ecrire_vers(chemin: &Path, droite_px: i64) -> std::io::Result<()> {
    if let Some(dossier) = chemin.parent() {
        std::fs::create_dir_all(dossier)?;
    }
    let mut tmp = chemin.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, format!("droite_px = {droite_px}\n"))?;
    std::fs::rename(&tmp, chemin)
}

pub fn ecrire(droite_px: i64) -> std::io::Result<()> {
    ecrire_vers(&chemin(), droite_px)
}

/// Ramène `droite_px` dans `0 ..= largeur_zone - cote` (0 si la zone est plus étroite).
pub fn borner(droite_px: i64, largeur_zone: i64, cote: i64) -> i64 {
    droite_px.clamp(0, (largeur_zone - cote).max(0))
}

/// `x` de la fenêtre pour une `droite_px` (bornée ici).
pub fn x_fenetre(zone: [i32; 4], droite_px: i64, cote: i64) -> i64 {
    let largeur = zone[2] as i64;
    zone[0] as i64 + largeur - cote - borner(droite_px, largeur, cote)
}

/// `droite_px` (bornée) pour une fenêtre dont le bord gauche est en `x`.
pub fn droite_depuis_x(zone: [i32; 4], x: i64, cote: i64) -> i64 {
    let largeur = zone[2] as i64;
    borner(zone[0] as i64 + largeur - cote - x, largeur, cote)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("osampo-position-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn chemin_xdg_puis_repli() {
        assert_eq!(
            chemin_dans(Some("/s".into()), "/h".into()),
            PathBuf::from("/s/osampo-pc/position.toml")
        );
        for vide in [None, Some(OsString::new())] {
            assert_eq!(
                chemin_dans(vide, "/h".into()),
                PathBuf::from("/h/.local/state/osampo-pc/position.toml")
            );
        }
    }

    #[test]
    fn ecrire_puis_lire_cree_le_dossier() {
        let d = dossier("aller-retour");
        let c = chemin_dans(Some(d.clone().into_os_string()), "/h".into());
        ecrire_vers(&c, 120).unwrap();
        assert_eq!(lire_depuis(&c), 120);
        assert_eq!(std::fs::read_to_string(&c).unwrap(), "droite_px = 120\n");
        assert!(!d.join("osampo-pc").join("position.toml.tmp").exists());
        ecrire_vers(&c, 7).unwrap();
        assert_eq!(lire_depuis(&c), 7);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lire_absent_illisible_ou_negatif_donne_zero() {
        let d = dossier("lire");
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(lire_depuis(&d.join("absent.toml")), 0);
        for (nom, texte) in [
            ("illisible", "droite_px = = ["),
            ("negatif", "droite_px = -5\n"),
            ("texte", "droite_px = \"12\"\n"),
            ("cle", "autre = 3\n"),
        ] {
            let f = d.join(nom);
            std::fs::write(&f, texte).unwrap();
            assert_eq!(lire_depuis(&f), 0, "{nom}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lire_tolere_commentaires_et_espaces() {
        let d = dossier("commentaires");
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("p.toml");
        std::fs::write(&f, "# position\n  droite_px=42 # px\n").unwrap();
        assert_eq!(lire_depuis(&f), 42);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn valeur_trop_grande_est_lue_telle_quelle() {
        // le fichier n'est pas réécrit : c'est l'appelant qui borne
        let d = dossier("grande");
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("p.toml");
        std::fs::write(&f, "droite_px = 99999\n").unwrap();
        assert_eq!(lire_depuis(&f), 99999);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn borner_dans_les_limites() {
        assert_eq!(borner(-3, 1000, 64), 0);
        assert_eq!(borner(0, 1000, 64), 0);
        assert_eq!(borner(500, 1000, 64), 500);
        assert_eq!(borner(936, 1000, 64), 936);
        assert_eq!(borner(937, 1000, 64), 936);
        assert_eq!(borner(5, 40, 64), 0); // zone plus étroite que le chien
    }

    #[test]
    fn x_et_droite_sont_reciproques() {
        let zone = [100, 0, 1000, 800];
        assert_eq!(x_fenetre(zone, 0, 64), 100 + 1000 - 64);
        assert_eq!(x_fenetre(zone, 936, 64), 100);
        assert_eq!(x_fenetre(zone, 5000, 64), 100); // bornée
        for d in [0, 1, 400, 936] {
            assert_eq!(droite_depuis_x(zone, x_fenetre(zone, d, 64), 64), d);
        }
        assert_eq!(droite_depuis_x(zone, 50, 64), 936); // trop à gauche
        assert_eq!(droite_depuis_x(zone, 2000, 64), 0); // trop à droite
    }
}
