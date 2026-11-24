//! Outil des tests du depot (daemon, osampo-reglages) : un dossier temporaire efface a la
//! fin du test, y compris quand le test echoue. Hors tests, ce module ne sert a rien.

use std::ops::Deref;
use std::path::{Path, PathBuf};

/// `<tmp>/osampo-test-<pid>-<nom>`, cree vide et efface au Drop. Le nom doit etre unique
/// dans le binaire de test (les tests d un binaire tournent en parallele).
pub struct DossierTemp(PathBuf);

impl DossierTemp {
    pub fn new(nom: &str) -> Self {
        let d = std::env::temp_dir().join(format!("osampo-test-{}-{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dossier temporaire de test");
        DossierTemp(d)
    }
}

impl Deref for DossierTemp {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl AsRef<Path> for DossierTemp {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for DossierTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cree_vide_puis_efface() {
        let chemin;
        {
            let d = DossierTemp::new("essai");
            chemin = d.to_path_buf();
            assert!(chemin.is_dir());
            std::fs::write(d.join("f"), "x").unwrap();
        }
        assert!(!chemin.exists(), "efface au Drop");
    }

    #[test]
    fn efface_meme_si_le_test_panique() {
        let chemin = std::env::temp_dir().join(format!("osampo-test-{}-panique", std::process::id()));
        let r = std::panic::catch_unwind(|| {
            let _d = DossierTemp::new("panique");
            panic!("echec simule");
        });
        assert!(r.is_err());
        assert!(!chemin.exists());
    }
}
