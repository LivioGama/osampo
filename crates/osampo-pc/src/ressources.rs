//! Ressources lues a l'execution : icones de la zone de notification, sons, image des
//! notifications. Trois emplacements, dans l'ordre :
//! 1. `$OSAMPO_DATA` s'il est defini ;
//! 2. `<dossier du binaire>/../share/osampo` s'il existe (installation :
//!    `~/.local/bin/osampo-pc` -> `~/.local/share/osampo`) ;
//! 3. le depot, chemins fixes a la compilation (binaire lance depuis `target/`).
//!
//! Disposition d'un dossier installe : `icons/hicolor/...`, `sounds/<nom>.wav`, `aboie.png`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const DEPOT_ICONES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/icons");
const DEPOT_SONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../widgets/osampo/sounds");
const DEPOT_IMAGE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../widgets/osampo/ui/sprites/aboie.png");

struct Chemins {
    icones: PathBuf,
    sons: PathBuf,
    image: PathBuf,
}

/// Dossier installe a utiliser, s'il y en a un.
fn installe(env: Option<PathBuf>, exe: Option<&Path>) -> Option<PathBuf> {
    if let Some(d) = env.filter(|d| !d.as_os_str().is_empty()) {
        return Some(d);
    }
    let d = exe?.parent()?.join("../share/osampo");
    d.is_dir().then_some(d)
}

fn chemins() -> &'static Chemins {
    static C: OnceLock<Chemins> = OnceLock::new();
    C.get_or_init(|| {
        let exe = std::env::current_exe().ok();
        match installe(std::env::var_os("OSAMPO_DATA").map(PathBuf::from), exe.as_deref()) {
            Some(d) => Chemins { icones: d.join("icons"), sons: d.join("sounds"), image: d.join("aboie.png") },
            None => Chemins {
                icones: DEPOT_ICONES.into(),
                sons: DEPOT_SONS.into(),
                image: DEPOT_IMAGE.into(),
            },
        }
    })
}

/// Dossier des icones 22 et 24 px, `hicolor/<taille>/status/osampo-<icone>.png`.
pub fn icones() -> &'static Path {
    &chemins().icones
}

/// Dossier des sons `<nom>.wav`.
pub fn sons() -> &'static Path {
    &chemins().sons
}

/// Image des notifications de rappel.
pub fn image_notification() -> &'static Path {
    &chemins().image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_d_environnement_d_abord() {
        let d = installe(Some("/opt/x".into()), Some(Path::new("/usr/bin/osampo-pc")));
        assert_eq!(d, Some(PathBuf::from("/opt/x")));
        // vide : ignoree
        assert_eq!(installe(Some(PathBuf::new()), Some(Path::new("/nulle/part/bin/t"))), None);
    }

    #[test]
    fn dossier_share_a_cote_du_binaire() {
        let racine = std::env::temp_dir().join(format!("osampo-ress-{}", std::process::id()));
        std::fs::create_dir_all(racine.join("bin")).unwrap();
        let exe = racine.join("bin/osampo-pc");
        assert_eq!(installe(None, Some(&exe)), None, "sans share : chemins du depot");
        std::fs::create_dir_all(racine.join("share/osampo")).unwrap();
        let d = installe(None, Some(&exe)).unwrap();
        assert_eq!(d.canonicalize().unwrap(), racine.join("share/osampo").canonicalize().unwrap());
        std::fs::remove_dir_all(&racine).unwrap();
    }

    #[test]
    fn depot_par_defaut_existe() {
        assert!(Path::new(DEPOT_ICONES).join("hicolor").is_dir());
        assert!(Path::new(DEPOT_SONS).join("wouf.wav").is_file());
        assert!(Path::new(DEPOT_IMAGE).is_file());
    }
}
