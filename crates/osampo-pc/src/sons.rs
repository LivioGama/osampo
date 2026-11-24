//! Sons du PC (SPEC-008) : les WAV de `widgets/osampo/sounds/`, joués en entier par
//! `paplay` détaché (comme `widgets/osampo/desktop/sound_desktop.cpp`), muets en heures
//! calmes. Chaque son, joué ou non, laisse une ligne sur stderr.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// La ligne de trace de SPEC-008 : `son: wouf` ou `son: wouf muet (heures calmes)`.
fn trace(nom: &str, calme: bool) -> String {
    if calme {
        format!("son: {nom} muet (heures calmes)")
    } else {
        format!("son: {nom}")
    }
}

fn chemin(nom: &str) -> PathBuf {
    crate::ressources::sons().join(format!("{nom}.wav"))
}

/// Joue le son `nom` de `widgets/osampo/sounds/` en entier, muet si `calme`.
pub fn jouer(nom: &str, calme: bool) {
    eprintln!("{}", trace(nom, calme));
    if calme {
        return;
    }
    let fichier = chemin(nom);
    match Command::new("paplay")
        .arg(&fichier)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        // Un fil attend la fin de paplay : pas de zombie, jamais de blocage de la boucle.
        Ok(mut enfant) => {
            std::thread::spawn(move || {
                let _ = enfant.wait();
            });
        }
        Err(e) => eprintln!("paplay non lancé pour {} : {e}", fichier.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traces() {
        assert_eq!(trace("wouf", false), "son: wouf");
        assert_eq!(trace("jappe", true), "son: jappe muet (heures calmes)");
    }

    #[test]
    fn les_wav_existent() {
        for nom in ["wouf", "jappe"] {
            assert!(chemin(nom).exists(), "{}", chemin(nom).display());
        }
    }
}
