//! Notification de bureau simple (sans action), pour les suites de commande de SPEC-004.

use std::collections::HashMap;

use zbus::zvariant::Value;

/// `org.freedesktop.Notifications.Notify` avec un résumé et un texte. Une erreur D-Bus est
/// tracée sur stderr, jamais fatale.
pub async fn notifier(resume: &str, corps: &str) {
    let r: zbus::Result<()> = async {
        let conn = zbus::Connection::session().await?;
        let hints: HashMap<&str, Value<'_>> = HashMap::new();
        conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &("Osampo", 0u32, "", resume, corps, Vec::<&str>::new(), hints, -1i32),
        )
        .await?;
        Ok(())
    }
    .await;
    if let Err(e) = r {
        eprintln!("notification impossible : {e}");
    }
}
