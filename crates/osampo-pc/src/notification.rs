//! Notification de bureau du rappel (SPEC-005) : une notification `Notify` à chaque passage
//! en `reclame`, ses actions relayées au daemon, retirée quand le rappel n'est plus en
//! réclamation. Le son `wouf` part avec elle (SPEC-008).

use std::collections::HashMap;

use futures_util::future::pending;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use zbus::zvariant::Value;

use crate::sons;
use crate::statuts::Rappel;

const DEST: &str = "org.freedesktop.Notifications";
const CHEMIN: &str = "/org/freedesktop/Notifications";

/// Action choisie par l'utilisateur sur la notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRappel {
    /// `accepter` ou `reporter`
    pub cmd: &'static str,
    /// `id` du rappel (`rappel_en_cours.id`)
    pub rappel: String,
}

/// Ce que la tâche D-Bus doit faire.
#[derive(Debug, PartialEq, Eq)]
enum Ordre {
    Ouvrir { rappel: String, corps: String, report_min: i64 },
    Fermer,
}

/// Ce que `rappel_change` décide d'un changement de `rappel_en_cours`.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Rien,
    Ouvrir,
    Fermer,
}

/// `reclamee` : l'id du rappel dont la notification a été ouverte (même si l'utilisateur l'a
/// fermée depuis : pas de nouvelle notification avant un nouveau passage en `reclame`),
/// `None` s'il n'y en a pas.
fn decider(reclamee: Option<&str>, nouveau: Option<&Rappel>) -> Decision {
    match nouveau {
        Some(r) if r.reclame() && notifiable(r) => {
            if reclamee == Some(r.id.as_str()) {
                Decision::Rien
            } else {
                Decision::Ouvrir
            }
        }
        _ if reclamee.is_some() => Decision::Fermer,
        _ => Decision::Rien,
    }
}

/// Sortie, sport, dodo et la réclamation de la jauge ; pas de notification pour `repas`.
pub(crate) fn notifiable(r: &Rappel) -> bool {
    matches!(r.categorie.as_str(), "sortie" | "sport" | "dodo" | "promenade")
}

fn corps(r: &Rappel) -> String {
    if r.categorie == "promenade" {
        "Sage à 0 : promenade".to_string()
    } else {
        format!("Rappel {} de {}", r.categorie, r.heure)
    }
}

/// `[rappels] report_min` de `osampo.toml`, relu à chaque notification ; 10 (défaut) si le
/// fichier manque ou est invalide.
pub(crate) fn report_min() -> i64 {
    std::fs::read_to_string(osampo_config::chemin_par_defaut())
        .ok()
        .and_then(|t| osampo_config::lire(&t).ok())
        .map_or(10, |r| r.report_min)
}

pub struct Notification {
    ordres: mpsc::UnboundedSender<Ordre>,
    reclamee: Option<String>,
}

impl Notification {
    /// Rend aussi le canal des actions choisies, que la boucle principale relaie au daemon.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ActionRappel>) {
        let (actions_tx, actions_rx) = mpsc::unbounded_channel();
        let (ordres, ordres_rx) = mpsc::unbounded_channel();
        tokio::spawn(tache(ordres_rx, actions_tx));
        (Notification { ordres, reclamee: None }, actions_rx)
    }

    /// Appelée à chaque changement de `rappel_en_cours` (`None` : plus de rappel).
    pub fn rappel_change(&mut self, rappel: Option<&Rappel>, calme: bool) {
        match decider(self.reclamee.as_deref(), rappel) {
            Decision::Rien => {}
            Decision::Fermer => {
                self.reclamee = None;
                let _ = self.ordres.send(Ordre::Fermer);
            }
            Decision::Ouvrir => {
                let Some(r) = rappel else { return };
                self.reclamee = Some(r.id.clone());
                let _ = self.ordres.send(Ordre::Ouvrir {
                    rappel: r.id.clone(),
                    corps: corps(r),
                    report_min: report_min(),
                });
                sons::jouer("wouf", calme);
            }
        }
    }
}

struct Session {
    proxy: zbus::Proxy<'static>,
    /// `ActionInvoked` et `NotificationClosed`, mêlés.
    signaux: futures_util::stream::Select<
        zbus::proxy::SignalStream<'static>,
        zbus::proxy::SignalStream<'static>,
    >,
}

async fn session() -> zbus::Result<Session> {
    let conn = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(&conn, DEST, CHEMIN, DEST).await?;
    // Abonnement avant tout `Notify` : aucune action ne peut être manquée.
    let actions = proxy.receive_signal("ActionInvoked").await?;
    let fermetures = proxy.receive_signal("NotificationClosed").await?;
    Ok(Session { proxy, signaux: futures_util::stream::select(actions, fermetures) })
}

/// Une notification ouverte : son numéro côté serveur et le rappel qu'elle porte.
struct Ouverte {
    nid: u32,
    rappel: String,
}

async fn tache(
    mut ordres: mpsc::UnboundedReceiver<Ordre>,
    actions_tx: mpsc::UnboundedSender<ActionRappel>,
) {
    let mut s: Option<Session> = None;
    let mut ouverte: Option<Ouverte> = None;
    loop {
        tokio::select! {
            ordre = ordres.recv() => {
                let Some(ordre) = ordre else { return };
                match ordre {
                    Ordre::Fermer => {
                        if let (Some(s), Some(o)) = (&s, ouverte.take()) {
                            fermer(s, o.nid).await;
                        }
                    }
                    Ordre::Ouvrir { rappel, corps, report_min } => {
                        if s.is_none() {
                            match session().await {
                                Ok(n) => s = Some(n),
                                Err(e) => eprintln!("notification impossible : {e}"),
                            }
                        }
                        let Some(sess) = &s else { continue };
                        if let Some(o) = ouverte.take() {
                            fermer(sess, o.nid).await;
                        }
                        match notifier(sess, &corps, report_min).await {
                            Ok(nid) => ouverte = Some(Ouverte { nid, rappel }),
                            Err(e) => eprintln!("notification impossible : {e}"),
                        }
                    }
                }
            }
            m = async {
                match s.as_mut() {
                    Some(s) => s.signaux.next().await,
                    None => pending().await,
                }
            } => {
                let Some(m) = m else { s = None; ouverte = None; continue };
                match m.header().member().map(|n| n.as_str()) {
                    Some("ActionInvoked") => {
                        let Ok((nid, cle)) = m.body().deserialize::<(u32, String)>() else { continue };
                        let cmd = match cle.as_str() {
                            "accepter" => "accepter",
                            "reporter" => "reporter",
                            _ => continue,
                        };
                        if let Some(o) = ouverte.take_if(|o| o.nid == nid) {
                            let _ = actions_tx.send(ActionRappel { cmd, rappel: o.rappel });
                            if let Some(s) = &s {
                                fermer(s, nid).await;
                            }
                        }
                    }
                    Some("NotificationClosed") => {
                        // Fermée par l'utilisateur (ou le serveur) : le rappel ne change pas.
                        if let Ok((nid, _raison)) = m.body().deserialize::<(u32, u32)>() {
                            ouverte.take_if(|o| o.nid == nid);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn notifier(s: &Session, corps: &str, report_min: i64) -> zbus::Result<u32> {
    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("urgency", Value::from(2u8));
    hints.insert("resident", Value::from(true));
    hints.insert("suppress-sound", Value::from(true));
    let reporter = format!("Reporter {report_min} min");
    let actions = ["accepter", "Accepter", "reporter", reporter.as_str()];
    s.proxy
        .call(
            "Notify",
            &(
                "Osampo",
                0u32,
                icone().as_str(),
                "Le chiot veut sortir",
                corps,
                actions.as_slice(),
                hints,
                0i32,
            ),
        )
        .await
}

async fn fermer(s: &Session, nid: u32) {
    let r: zbus::Result<()> = s.proxy.call("CloseNotification", &(nid,)).await;
    if let Err(e) = r {
        eprintln!("fermeture de la notification impossible : {e}");
    }
}

/// Chemin absolu de l'image du chien qui aboie.
fn icone() -> String {
    {
        let i = crate::ressources::image_notification();
        std::fs::canonicalize(i).unwrap_or_else(|_| i.to_path_buf()).display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rappel(id: &str, categorie: &str, etat: &str) -> Rappel {
        Rappel {
            id: id.into(),
            categorie: categorie.into(),
            heure: "12:30".into(),
            etat: etat.into(),
        }
    }

    #[test]
    fn une_notification_par_passage_en_reclame() {
        let r = rappel("sortie,1230", "sortie", "reclame");
        assert_eq!(decider(None, Some(&r)), Decision::Ouvrir);
        assert_eq!(decider(Some("sortie,1230"), Some(&r)), Decision::Rien);
        let autre = rappel("sport,1800", "sport", "reclame");
        assert_eq!(decider(Some("sortie,1230"), Some(&autre)), Decision::Ouvrir);
    }

    #[test]
    fn retrait_a_null_ou_reporte() {
        let p = rappel("sortie,1230", "sortie", "reporte");
        assert_eq!(decider(Some("sortie,1230"), None), Decision::Fermer);
        assert_eq!(decider(Some("sortie,1230"), Some(&p)), Decision::Fermer);
        assert_eq!(decider(None, None), Decision::Rien);
        assert_eq!(decider(None, Some(&p)), Decision::Rien);
    }

    #[test]
    fn repas_ne_notifie_pas() {
        assert_eq!(decider(None, Some(&rappel("repas,1200", "repas", "reclame"))), Decision::Rien);
    }

    #[test]
    fn corps_du_rappel_et_de_la_jauge() {
        assert_eq!(corps(&rappel("dodo,2200", "dodo", "reclame")), "Rappel dodo de 12:30");
        assert_eq!(corps(&rappel("promenade", "promenade", "reclame")), "Sage à 0 : promenade");
    }

    #[test]
    fn icone_absolue_et_existante() {
        let i = icone();
        let p = std::path::Path::new(&i);
        assert!(p.is_absolute() && p.exists(), "{i}");
    }
}
