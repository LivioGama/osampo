// Test d integration de l emetteur socket. Le binaire est lance en
// sous-processus : c est le seul moyen d exercer le chemin reel, poignee de
// main comprise.
//
// Aucune lecture ne peut pendre : chaque flux porte un delai de lecture, et
// chaque attente a une echeance.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// Delai de lecture applique a tout flux de test.
const DELAI_LECTURE: Duration = Duration::from_secs(3);

struct Daemon {
    child: Child,
    socket: PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn ecrire_manifeste(racine: &Path, id: &str, cles: &str) {
    let d = racine.join(id);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("widget.toml"),
        // cadence volontairement rapide : le test du client lent doit voir
        // passer plus de 80 trames sans durer dix secondes.
        format!("id = \"{id}\"\nformats = [\"bar\"]\nrefresh_ms = 20\n{cles}"),
    )
    .unwrap();
}

const CLES_DEMO3: &str = "[[keys]]\nname = \"value\"\ntype = \"float\"\n\n[[keys]]\nname = \"label\"\ntype = \"string\"\n\n[[keys]]\nname = \"state\"\ntype = \"string\"\n";

/// Repertoire de test vide, rendu avec le chemin du socket et la racine des widgets.
fn preparer(nom: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("sp3_socket_{nom}"));
    let _ = std::fs::remove_dir_all(&base);
    let racine = base.join("widgets");
    std::fs::create_dir_all(&racine).unwrap();
    (racine, base.join("d.sock"))
}

/// Lance le daemon et attend qu il accepte une connexion. Attendre la seule
/// existence du fichier ne suffit pas : un socket residuel existe deja.
fn lancer(racine: &Path, socket: &Path) -> Daemon {
    lancer_avec(racine, socket, &[])
}

fn lancer_avec(racine: &Path, socket: &Path, extra: &[&Path]) -> Daemon {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_osampo-daemon"));
    cmd.arg("--widgets-dir").arg(racine).arg("--socket").arg(socket);
    if let [faux] = extra {
        cmd.arg("--faux-ble").arg(faux);
    }
    let child = cmd
        .spawn()
        .expect("le binaire daemon doit se lancer");
    let mut d = Daemon { child, socket: socket.to_path_buf() };

    let limite = Instant::now() + Duration::from_secs(10);
    while UnixStream::connect(socket).is_err() {
        if let Ok(Some(statut)) = d.child.try_wait() {
            panic!("le daemon s est arrete avant d ecouter: {statut}");
        }
        assert!(Instant::now() < limite, "le daemon n a jamais accepte de connexion");
        std::thread::sleep(Duration::from_millis(50));
    }
    d
}

fn demarrer(nom: &str) -> Daemon {
    let (racine, socket) = preparer(nom);
    ecrire_manifeste(&racine, "demo3", CLES_DEMO3);
    // un second widget qui ne declare qu une cle : sert au test de filtrage
    ecrire_manifeste(&racine, "maigre", "[[keys]]\nname = \"value\"\ntype = \"float\"\n");
    lancer(&racine, &socket)
}

fn ouvrir(d: &Daemon, hello: &[u8]) -> UnixStream {
    let flux = UnixStream::connect(&d.socket).expect("connexion au daemon");
    flux.set_read_timeout(Some(DELAI_LECTURE)).unwrap();
    flux.set_write_timeout(Some(DELAI_LECTURE)).unwrap();
    (&flux).write_all(hello).unwrap();
    flux
}

fn connecter(d: &Daemon, hello: &str) -> BufReader<UnixStream> {
    BufReader::new(ouvrir(d, format!("{hello}\n").as_bytes()))
}

fn ligne(r: &mut BufReader<UnixStream>) -> serde_json::Value {
    let mut s = String::new();
    r.read_line(&mut s).expect("lecture sous delai");
    assert!(!s.is_empty(), "flux ferme alors qu une ligne etait attendue");
    serde_json::from_str(s.trim_end()).unwrap_or_else(|e| panic!("ligne illisible {s:?}: {e}"))
}

/// Lit jusqu a la fin de flux. Rend vrai si le pair a ferme avant l echeance.
fn ferme_avant(flux: &mut impl Read, echeance: Duration) -> bool {
    let limite = Instant::now() + echeance;
    let mut tampon = [0u8; 4096];
    while Instant::now() < limite {
        match flux.read(&mut tampon) {
            Ok(0) => return true,
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut => continue,
            // ECONNRESET : le daemon a ferme avec des donnees non lues.
            Err(_) => return true,
        }
    }
    false
}

/// Attend la fin du processus, sous echeance.
fn attendre_la_fin(d: &mut Daemon, echeance: Duration) -> Option<std::process::ExitStatus> {
    let limite = Instant::now() + echeance;
    while Instant::now() < limite {
        if let Some(s) = d.child.try_wait().unwrap() {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

// Evite une dependance a la crate libc pour un seul appel.
extern "C" {
    fn setsockopt(fd: i32, level: i32, name: i32, val: *const std::ffi::c_void, len: u32) -> i32;
}
const SOL_SOCKET: i32 = 1;
const SO_RCVBUF: i32 = 8;

#[test]
fn hello_valide_donne_un_ack_puis_des_trames() {
    let d = demarrer("ack");
    let mut r = connecter(&d, "{\"hello\":\"demo3\"}");

    let ack = ligne(&mut r);
    assert_eq!(ack["ack"], "demo3");
    assert_eq!(ack["cadence_ms"], 20);
    assert_eq!(ack["stale_s"], 30, "le seuil par defaut doit etre annonce");

    for _ in 0..3 {
        let t = ligne(&mut r);
        assert!(t["value"].is_number(), "trame sans value: {t}");
        assert!(t["label"].is_string(), "trame sans label: {t}");
        assert!(t["age"].is_number(), "trame sans age: {t}");
    }
}

#[test]
fn hello_inconnu_est_refuse_avec_un_message() {
    let d = demarrer("refus");
    let mut r = connecter(&d, "{\"hello\":\"inexistant\"}");
    let rep = ligne(&mut r);
    assert!(rep["error"].is_string(), "un refus doit porter un message: {rep}");
    assert!(
        rep["error"].as_str().unwrap().contains("manifeste introuvable"),
        "message obtenu: {rep}"
    );
    assert!(ferme_avant(r.get_mut(), Duration::from_secs(5)), "un refus est suivi de la fermeture");
}

#[test]
fn le_filtrage_est_par_connexion() {
    let d = demarrer("filtrage");
    let mut riche = connecter(&d, "{\"hello\":\"demo3\"}");
    let mut maigre = connecter(&d, "{\"hello\":\"maigre\"}");
    let _ = ligne(&mut riche);
    let _ = ligne(&mut maigre);

    let a = ligne(&mut riche);
    let b = ligne(&mut maigre);
    assert!(a["label"].is_string(), "le widget riche recoit label: {a}");
    assert!(b["label"].is_null(), "le widget maigre ne doit pas recevoir label: {b}");
    assert!(b["value"].is_number(), "le widget maigre recoit bien value: {b}");
}

#[test]
fn un_client_lent_est_deconnecte_sans_geler_les_autres() {
    let d = demarrer("lent");

    // Le lecteur assidu lit en continu dans son propre fil, et note l heure
    // de chaque trame : il doit recevoir a la cadence quoi qu il arrive.
    let mut assidu = connecter(&d, "{\"hello\":\"demo3\"}");
    let _ = ligne(&mut assidu);
    let fil_assidu = std::thread::spawn(move || {
        let mut instants = Vec::new();
        let fin = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fin {
            let t = ligne(&mut assidu);
            assert!(t["value"].is_number(), "l assidu doit recevoir des trames: {t}");
            instants.push(Instant::now());
        }
        instants
    });

    // Le paresseux : tampon de reception reduit, hello, puis plus rien.
    let flux = UnixStream::connect(&d.socket).unwrap();
    let petit: i32 = 1024;
    let rc = unsafe {
        setsockopt(
            flux.as_raw_fd(),
            SOL_SOCKET,
            SO_RCVBUF,
            &petit as *const i32 as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        )
    };
    assert_eq!(rc, 0, "setsockopt SO_RCVBUF");
    flux.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
    (&flux).write_all(b"{\"hello\":\"demo3\"}\n").unwrap();
    let mut paresseux = flux;

    // A 20 ms par trame, file de 16 puis 65 trames jetees d affilee : la
    // fermeture tombe vers 2 s. Le paresseux ne lit rien pendant ce temps.
    std::thread::sleep(Duration::from_secs(3));

    // Il lit maintenant ce qui restait en tampon. Si le daemon n avait pas
    // ferme, ce lecteur devenu rapide ne verrait jamais la fin de flux.
    assert!(
        ferme_avant(&mut paresseux, Duration::from_secs(5)),
        "le daemon doit fermer la connexion d un client qui ne lit plus"
    );

    let instants = fil_assidu.join().expect("le fil de l assidu ne doit pas paniquer");
    assert!(instants.len() >= 100, "l assidu doit recevoir a la cadence, {} trames en 5 s", instants.len());
    let pire = instants.windows(2).map(|w| w[1] - w[0]).max().unwrap();
    assert!(pire < Duration::from_millis(500), "trou de {pire:?} chez l assidu");
}

#[test]
fn demarre_malgre_un_socket_residuel() {
    // Un daemon qui a plante laisse son fichier socket en place. bind
    // echouerait en EADDRINUSE : le daemon doit constater que plus personne
    // n ecoute, supprimer le fichier, et repartir.
    let (racine, socket) = preparer("residuel");
    ecrire_manifeste(&racine, "demo3", CLES_DEMO3);
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    assert!(socket.exists(), "le socket residuel doit exister avant le test");
    assert!(UnixStream::connect(&socket).is_err(), "personne ne doit ecouter le residuel");

    let d = lancer(&racine, &socket);
    let mut r = connecter(&d, "{\"hello\":\"demo3\"}");
    assert_eq!(ligne(&mut r)["ack"], "demo3");
}

#[test]
fn une_ligne_qui_n_est_pas_un_hello_est_refusee() {
    let d = demarrer("illisible");
    let mut r = connecter(&d, "ceci n est pas du json");
    let rep = ligne(&mut r);
    assert!(rep["error"].is_string(), "reponse obtenue: {rep}");
}

#[test]
fn un_hello_trop_long_est_refuse_sans_attendre_son_saut_de_ligne() {
    let d = demarrer("trop_long");
    // 5000 octets sans saut de ligne : le daemon ne doit pas attendre la fin
    // de la ligne, il refuse des que la limite de 4096 est franchie.
    let mut r = BufReader::new(ouvrir(&d, &[b'a'; 5000]));
    let rep = ligne(&mut r);
    assert!(rep["error"].is_string(), "reponse obtenue: {rep}");
    assert!(ferme_avant(r.get_mut(), Duration::from_secs(5)), "le refus est suivi de la fermeture");
}

#[test]
fn le_socket_disparait_sur_sigterm() {
    let mut d = demarrer("sigterm");
    let statut = Command::new("kill")
        .arg("-TERM")
        .arg(d.child.id().to_string())
        .status()
        .unwrap();
    assert!(statut.success());
    let fin = attendre_la_fin(&mut d, Duration::from_secs(5)).expect("le daemon doit s arreter sur SIGTERM");
    assert!(fin.success(), "arret propre attendu, obtenu {fin}");
    assert!(!d.socket.exists(), "le fichier socket doit etre supprime a l arret");
}

#[test]
fn le_controle_croise_porte_sur_tous_les_manifestes() {
    // Le widget fautif est le dernier par ordre alphabetique : un controle
    // limite au widget de cadence ne le verrait pas.
    let (racine, socket) = preparer("croise");
    ecrire_manifeste(&racine, "demo3", CLES_DEMO3);
    ecrire_manifeste(&racine, "zz_orphelin", "[[keys]]\nname = \"absente\"\ntype = \"float\"\n");
    let child = Command::new(env!("CARGO_BIN_EXE_osampo-daemon"))
        .arg("--widgets-dir")
        .arg(&racine)
        .arg("--socket")
        .arg(&socket)
        .spawn()
        .unwrap();
    let mut d = Daemon { child, socket };
    let fin = attendre_la_fin(&mut d, Duration::from_secs(10))
        .expect("le daemon doit refuser de demarrer");
    assert!(!fin.success(), "une cle que personne ne produit doit bloquer le demarrage");
}

#[test]
fn une_cle_absente_du_manifeste_de_cadence_bloque_le_demarrage() {
    // Le widget de cadence (premier par ordre alphabetique) ne declare que
    // value ; le second declare aussi label, que la source produit bien. Le
    // planificateur ne connait que les cles du premier : label ne serait
    // jamais emise. Le daemon doit le dire et refuser de demarrer.
    let (racine, socket) = preparer("hors_cadence");
    ecrire_manifeste(&racine, "aa_cadence", "[[keys]]\nname = \"value\"\ntype = \"float\"\n");
    ecrire_manifeste(
        &racine,
        "zz_second",
        "[[keys]]\nname = \"value\"\ntype = \"float\"\n\n[[keys]]\nname = \"label\"\ntype = \"string\"\n",
    );
    let child = Command::new(env!("CARGO_BIN_EXE_osampo-daemon"))
        .arg("--widgets-dir")
        .arg(&racine)
        .arg("--socket")
        .arg(&socket)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut d = Daemon { child, socket };
    let fin = attendre_la_fin(&mut d, Duration::from_secs(10))
        .expect("le daemon doit refuser de demarrer");
    assert!(!fin.success(), "une cle absente du manifeste de cadence doit bloquer le demarrage");
    let mut erreur = String::new();
    d.child.stderr.take().unwrap().read_to_string(&mut erreur).unwrap();
    assert!(
        erreur.contains("zz_second") && erreur.contains("label") && erreur.contains("aa_cadence"),
        "le message doit nommer le widget, la cle et le manifeste de cadence: {erreur}"
    );
}

#[test]
fn le_mode_d_un_repertoire_existant_n_est_pas_touche() {
    use std::os::unix::fs::PermissionsExt;
    // --socket dans un repertoire qui n est pas celui du repli : le daemon ne
    // doit pas en changer le mode (sinon $HOME passerait en 0700).
    let (racine, socket) = preparer("mode");
    ecrire_manifeste(&racine, "demo3", CLES_DEMO3);
    let base = socket.parent().unwrap().to_path_buf();
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o755)).unwrap();

    let d = lancer(&racine, &socket);
    let mode = std::fs::metadata(&base).unwrap().permissions().mode() & 0o777;
    drop(d);
    assert_eq!(mode, 0o755, "mode du repertoire change: {mode:o}");
}

#[test]
fn un_fichier_ordinaire_sur_le_chemin_n_est_pas_efface() {
    let (racine, socket) = preparer("ordinaire");
    ecrire_manifeste(&racine, "demo3", CLES_DEMO3);
    std::fs::write(&socket, b"precieux").unwrap();

    let child = Command::new(env!("CARGO_BIN_EXE_osampo-daemon"))
        .arg("--widgets-dir")
        .arg(&racine)
        .arg("--socket")
        .arg(&socket)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut d = Daemon { child, socket: socket.clone() };
    let fin = attendre_la_fin(&mut d, Duration::from_secs(10))
        .expect("le daemon doit refuser de demarrer");
    let mut journal = String::new();
    d.child.stderr.take().unwrap().read_to_string(&mut journal).unwrap();

    assert!(!fin.success(), "demarrage attendu en echec, journal: {journal}");
    assert_eq!(std::fs::read(&socket).unwrap(), b"precieux", "le fichier a ete touche");
    assert!(journal.contains("n est pas un socket"), "message explicite attendu: {journal}");
}

// ---------------------------------------------------------------- programmes du PC (ACT-007)

/// Daemon avec une board simulee (`--faux-ble`) ; rend aussi le chemin de celle-ci.
fn demarrer_faux_ble(nom: &str) -> (Daemon, PathBuf) {
    let (racine, socket) = preparer(nom);
    ecrire_manifeste(&racine, "demo3", CLES_DEMO3);
    let faux = socket.with_file_name("b.sock");
    let d = lancer_avec(&racine, &socket, &[faux.as_path()]);
    // Ne PAS sonder par une connexion : la board simulee n accepte qu un client a la fois et
    // prendrait la sonde pour la board. Pendant qu elle detecte la fermeture de la sonde, elle
    // refuse (ferme) la vraie connexion du test. bind cree le fichier et met a l ecoute d un
    // seul geste, et `preparer` a vide le repertoire : son existence suffit.
    let limite = Instant::now() + Duration::from_secs(10);
    while !faux.exists() {
        assert!(Instant::now() < limite, "la board simulee n ecoute jamais");
        std::thread::sleep(Duration::from_millis(20));
    }
    (d, faux)
}

/// La board simulee : une ligne par envoi, lecture sous delai.
fn board(faux: &Path) -> BufReader<UnixStream> {
    // Entre bind (le fichier apparait) et listen, une connexion est refusee : on reessaie.
    let limite = Instant::now() + Duration::from_secs(5);
    let flux = loop {
        match UnixStream::connect(faux) {
            Ok(f) => break f,
            Err(e) => {
                assert!(Instant::now() < limite, "connexion a la board simulee: {e}");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    flux.set_read_timeout(Some(DELAI_LECTURE)).unwrap();
    BufReader::new(flux)
}

fn dire(r: &mut BufReader<UnixStream>, ligne: &str) {
    r.get_ref().write_all(format!("{ligne}\n").as_bytes()).unwrap();
}

/// Lit des lignes d etat jusqu a la premiere qui verifie `ok` (ou 5 s) ; rend le `pc` de cette ligne.
fn etat_jusqu_a(r: &mut BufReader<UnixStream>, ok: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let limite = Instant::now() + Duration::from_secs(5);
    let mut vues = Vec::new();
    while Instant::now() < limite {
        let l = ligne(r);
        if let Some(pc) = l.get("pc") {
            if ok(pc) {
                return pc.clone();
            }
        }
        vues.push(l);
    }
    panic!("etat attendu jamais publie ; lignes vues: {vues:?}");
}

const ETAT_REPOS: &str = r#"{"etat":"repos","raison":"-","rappel":"-","promenade_debut":"-","pile":83,"besoins":{"bouger":70,"jouer":60,"dormir":50}}"#;

#[test]
fn le_pc_recoit_l_etat_sans_manifeste_ni_ack() {
    let (d, _faux) = demarrer_faux_ble("pc_hello");
    let mut r = connecter(&d, "{\"hello\":\"osampo-pc\"}");
    let l = ligne(&mut r);
    assert!(l["ack"].is_null(), "pas d ack pour osampo-pc: {l}");
    let pc = &l["pc"];
    assert_eq!(pc["board"], "hors_ligne", "{l}");
    assert_eq!(pc["etat"], "-");
    assert_eq!(pc["icone"], "assis");
    assert_eq!(pc["pile"], -1);
    assert!(pc["besoins"].is_null());
    assert!(pc["rappel_en_cours"].is_null());
    assert_eq!(pc["prochain_rappel"], "aucun", "pas de rappel hors widget osampo: {l}");
}

#[test]
fn l_etat_de_la_board_et_la_coupure_du_lien_sont_publies() {
    let (d, faux) = demarrer_faux_ble("pc_lien");
    let mut pc = connecter(&d, "{\"hello\":\"osampo-pc\"}");
    let _ = ligne(&mut pc);

    let mut b = board(&faux);
    dire(&mut b, ETAT_REPOS);
    let v = etat_jusqu_a(&mut pc, |p| p["board"] == "connectee" && p["etat"] == "repos");
    assert_eq!(v["pile"], 83);
    assert_eq!(v["besoins"]["jouer"], 60);

    dire(&mut b, r#"{"etat":"dort","raison":"-","rappel":"-","promenade_debut":"-","pile":80,"besoins":{"bouger":70,"jouer":60,"dormir":50}}"#);
    let v = etat_jusqu_a(&mut pc, |p| p["etat"] == "dort");
    assert_eq!(v["icone"], "niche");

    // coupure : hors ligne en moins de 1 s, l etat garde, la pile a -1
    let t0 = Instant::now();
    drop(b);
    let v = etat_jusqu_a(&mut pc, |p| p["board"] == "hors_ligne");
    assert!(t0.elapsed() < Duration::from_secs(1), "lien coupe vu en {:?}", t0.elapsed());
    assert_eq!(v["pile"], -1);
    assert_eq!(v["etat"], "dort");
    assert_eq!(v["icone"], "niche");
}

#[test]
fn un_evenement_de_la_board_est_publie_en_moins_d_une_seconde() {
    let (d, faux) = demarrer_faux_ble("pc_evt");
    let mut pc = connecter(&d, "{\"hello\":\"osampo-pc\"}");
    let _ = ligne(&mut pc);
    let mut b = board(&faux);
    dire(&mut b, ETAT_REPOS);
    etat_jusqu_a(&mut pc, |p| p["etat"] == "repos");
    let t0 = Instant::now();
    dire(&mut b, r#"{"evt":"os","source":"board"}"#);
    let v = etat_jusqu_a(&mut pc, |p| !p["dernier_geste"].is_null());
    assert!(t0.elapsed() < Duration::from_secs(1), "evenement vu en {:?}", t0.elapsed());
    assert_eq!(v["dernier_geste"]["geste"], "os");
    assert_eq!(v["dernier_geste"]["source"], "board");
}

#[test]
fn les_commandes_du_pc_vont_a_la_board_et_recoivent_une_reponse() {
    let (d, faux) = demarrer_faux_ble("pc_cmd");
    let mut pc = connecter(&d, "{\"hello\":\"osampo-pc\"}");
    let _ = ligne(&mut pc);

    // lien coupe : refus, rien n est mis en file
    (pc.get_ref()).write_all(b"{\"cmd\":\"chatouille\"}\n").unwrap();
    let rep = reponse(&mut pc);
    assert_eq!(rep, serde_json::json!({"reponse":"chatouille","ok":false,"erreur":"board hors ligne"}));

    let mut b = board(&faux);
    dire(&mut b, ETAT_REPOS);
    etat_jusqu_a(&mut pc, |p| p["etat"] == "repos");
    (pc.get_ref()).write_all(b"{\"cmd\":\"chatouille\"}\n").unwrap();
    let (rep, deja_vu) = reponse_et_etats(&mut pc);
    assert_eq!(rep, serde_json::json!({"reponse":"chatouille","ok":true}));
    // la board recoit la ligne de commande (apres la ligne cfg / trames eventuelles)
    let mut vu = None;
    for _ in 0..50 {
        let mut l = String::new();
        if b.read_line(&mut l).unwrap_or(0) == 0 {
            break;
        }
        if l.contains("\"cmd\"") {
            vu = Some(l);
            break;
        }
    }
    assert_eq!(vu.as_deref().map(str::trim_end), Some(r#"{"cmd":"chatouille"}"#));
    // l etat au geste a pu arriver avant la reponse (voir `reponse_et_etats`)
    let v = match deja_vu.filter(|p| !p["dernier_geste"].is_null()) {
        Some(p) => p,
        None => etat_jusqu_a(&mut pc, |p| !p["dernier_geste"].is_null()),
    };
    assert_eq!(v["dernier_geste"]["geste"], "chatouille");
    assert_eq!(v["dernier_geste"]["source"], "pc");

    // un rappel inconnu
    (pc.get_ref()).write_all(b"{\"cmd\":\"accepter\",\"rappel\":\"sortie,1230\"}\n").unwrap();
    let rep = reponse(&mut pc);
    assert_eq!(rep["ok"], false);
    assert_eq!(rep["erreur"], "aucun rappel sortie,1230 en cours");
}

#[test]
fn une_ligne_illisible_donne_commande_inconnue_et_garde_la_connexion() {
    let (d, _faux) = demarrer_faux_ble("pc_inconnue");
    let mut pc = connecter(&d, "{\"hello\":\"osampo-pc\"}");
    let _ = ligne(&mut pc);
    let envois: [&[u8]; 3] = [b"pas du json\n", b"{\"cmd\":\"brosse\"}\n", b"\xff\xfe\n"];
    for envoi in envois {
        (pc.get_ref()).write_all(envoi).unwrap();
        let rep = reponse(&mut pc);
        assert_eq!(
            rep,
            serde_json::json!({"reponse":"?","ok":false,"erreur":"commande inconnue"}),
            "pour {envoi:?}"
        );
    }
    // la connexion est gardee : une commande valide recoit sa reponse
    (pc.get_ref()).write_all(b"{\"cmd\":\"os\"}\n").unwrap();
    assert_eq!(reponse(&mut pc)["erreur"], "board hors ligne");
}

/// La prochaine ligne de reponse (`reponse` present), en sautant les lignes d etat.
fn reponse(r: &mut BufReader<UnixStream>) -> serde_json::Value {
    reponse_et_etats(r).0
}

/// Comme `reponse`, et rend aussi le dernier `pc` vu avant elle : la reponse et la ligne
/// d etat qu elle provoque partent de deux taches, l ordre de leur arrivee n est pas garanti
/// (SPEC-002 ne le fixe pas). Un test qui jette l etat vu avant la reponse l attendrait
/// ensuite jusqu a la repetition de 10 s.
fn reponse_et_etats(r: &mut BufReader<UnixStream>) -> (serde_json::Value, Option<serde_json::Value>) {
    let mut dernier_pc = None;
    for _ in 0..20 {
        let l = ligne(r);
        if l.get("reponse").is_some() {
            return (l, dernier_pc);
        }
        if let Some(pc) = l.get("pc") {
            dernier_pc = Some(pc.clone());
        }
    }
    panic!("aucune reponse recue");
}

#[test]
fn l_etat_est_repete_au_moins_toutes_les_10_secondes() {
    let (d, _faux) = demarrer_faux_ble("pc_repete");
    let mut pc = connecter(&d, "{\"hello\":\"osampo-pc\"}");
    pc.get_ref().set_read_timeout(Some(Duration::from_secs(13))).unwrap();
    let _ = ligne(&mut pc);
    let t0 = Instant::now();
    let _ = ligne(&mut pc); // rien ne change : c est la repetition
    let ecoule = t0.elapsed();
    assert!(
        ecoule >= Duration::from_secs(8) && ecoule <= Duration::from_secs(11),
        "repetition apres {ecoule:?}"
    );
}

#[test]
fn ble_et_faux_ble_s_excluent() {
    let (racine, socket) = preparer("exclusifs");
    ecrire_manifeste(&racine, "demo3", CLES_DEMO3);
    let statut = Command::new(env!("CARGO_BIN_EXE_osampo-daemon"))
        .arg("--widgets-dir")
        .arg(&racine)
        .arg("--socket")
        .arg(&socket)
        .arg("--ble")
        .arg("--faux-ble")
        .arg(socket.with_file_name("b.sock"))
        .status()
        .unwrap();
    assert_eq!(statut.code(), Some(2));
}
