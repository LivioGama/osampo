//! Item StatusNotifierItem (SPEC-004) : propriétés, inscription auprès du watcher avec
//! nouvel essai toutes les 5 s, mise à jour sur changement d'état.

use std::time::Duration;

use ksni::{Category, Status, ToolTip, Tray, TrayMethods};
use tokio::sync::mpsc;

use crate::menu::{self, Action};
use crate::statuts::{self, Vue};

pub const DELAI_INSCRIPTION: Duration = Duration::from_secs(5);


pub struct Icone {
    pub vue: Vue,
    pub actions: mpsc::UnboundedSender<Action>,
    /// Prévenu quand le watcher disparaît : le superviseur se réinscrit.
    hors_ligne: mpsc::UnboundedSender<()>,
}

impl Tray for Icone {
    fn id(&self) -> String {
        "osampo".into()
    }

    fn title(&self) -> String {
        "Osampo".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn status(&self) -> Status {
        if statuts::attention(&self.vue) {
            Status::NeedsAttention
        } else {
            Status::Active
        }
    }

    fn icon_theme_path(&self) -> String {
        crate::ressources::icones().display().to_string()
    }

    fn icon_name(&self) -> String {
        statuts::nom_icone(&self.vue)
    }

    // Les hôtes qui affichent l'état « attention » cherchent cette icône-là.
    fn attention_icon_name(&self) -> String {
        statuts::nom_icone(&self.vue)
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: statuts::nom_icone(&self.vue),
            icon_pixmap: Vec::new(),
            title: statuts::titre(&self.vue).into(),
            description: statuts::lignes(&self.vue).join("\n"),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.actions.send(Action::Reglages);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        menu::construire(&self.vue)
    }

    fn watcher_offline(&self, _raison: ksni::OfflineReason) -> bool {
        let _ = self.hors_ligne.send(());
        false // on arrête ce service : le superviseur en relance un dans 5 s
    }
}

/// Garde l'icône inscrite : essaie toutes les 5 s tant que le watcher est absent, et de
/// nouveau s'il disparaît. `vues` apporte chaque nouvel état à afficher.
pub async fn superviser(mut vues: mpsc::UnboundedReceiver<Vue>, actions: mpsc::UnboundedSender<Action>) {
    let mut vue: Vue = None;
    loop {
        let (hors_tx, mut hors_rx) = mpsc::unbounded_channel();
        let icone = Icone { vue: vue.clone(), actions: actions.clone(), hors_ligne: hors_tx };
        let handle = match icone.spawn().await {
            Ok(h) => h,
            Err(e) => {
                eprintln!("inscription de l'icône impossible : {e} ; nouvel essai dans 5 s");
                let fin = tokio::time::sleep(DELAI_INSCRIPTION);
                tokio::pin!(fin);
                loop {
                    tokio::select! {
                        _ = &mut fin => break,
                        v = vues.recv() => match v {
                            Some(v) => vue = v,
                            None => return,
                        },
                    }
                }
                continue;
            }
        };
        loop {
            tokio::select! {
                v = vues.recv() => match v {
                    Some(v) => {
                        vue = v.clone();
                        handle.update(move |t: &mut Icone| t.vue = v.clone()).await;
                    }
                    None => return,
                },
                _ = hors_rx.recv() => {
                    eprintln!("StatusNotifierWatcher disparu ; nouvelle inscription dans 5 s");
                    tokio::time::sleep(DELAI_INSCRIPTION).await;
                    break;
                }
            }
        }
    }
}
