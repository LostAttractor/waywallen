//! Observe compositor display power independently of screen locking.
//!
//! GNOME exposes a global DPMS mode through Mutter. Other desktops without
//! this interface keep their existing auto replay behavior.

use std::sync::Arc;

use anyhow::Result;
use futures_util::StreamExt;
use tokio::sync::watch;
use zbus::{fdo::PropertiesProxy, proxy::CacheProperties, Connection};

use crate::wallframe::routing::Router;

const SERVICE: &str = "org.gnome.Mutter.DisplayConfig";
const PATH: &str = "/org/gnome/Mutter/DisplayConfig";
const PROPERTY: &str = "PowerSaveMode";

pub async fn run(router: Arc<Router>, connection: Connection, mut shutdown: watch::Receiver<bool>) {
    let (updates, mut state) = watch::channel(false);
    let monitor = monitor(connection, updates);
    tokio::pin!(monitor);

    loop {
        tokio::select! {
            result = shutdown.changed() => {
                if result.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            result = &mut monitor => {
                if let Err(error) = result {
                    log::warn!("display_power: monitor stopped: {error:#}");
                }
                break;
            }
            result = state.changed() => {
                if result.is_err() {
                    break;
                }
                let off = *state.borrow_and_update();
                log::info!("display_power: screen off={off}");
                router.update_screen_off_state(off).await;
            }
        }
    }
    // Losing the monitor must not leave an unowned pause behind.
    router.update_screen_off_state(false).await;
}

async fn monitor(connection: Connection, updates: watch::Sender<bool>) -> Result<()> {
    let properties = PropertiesProxy::builder(&connection)
        .destination(SERVICE)?
        .path(PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    // Subscribe before reading, including owner changes so a compositor which
    // starts later or reconnects is detected without periodic polling.
    let mut owners = properties.inner().receive_owner_changed().await?;
    let mut changes = properties.receive_properties_changed().await?;
    update(&properties, &updates).await;

    loop {
        tokio::select! {
            owner = owners.next() => {
                match owner {
                    Some(Some(_)) => update(&properties, &updates).await,
                    Some(None) => { updates.send_replace(false); }
                    None => break,
                }
            }
            change = changes.next() => {
                let Some(change) = change else { break };
                let args = change.args()?;
                if args.interface_name.as_str() == SERVICE
                    && (args.changed_properties.contains_key(PROPERTY)
                        || args.invalidated_properties.contains(&PROPERTY))
                {
                    // Read uncached state: also handles invalidated properties
                    // and avoids retaining an old owner's power mode.
                    update(&properties, &updates).await;
                }
            }
        }
    }
    Ok(())
}

async fn update(properties: &PropertiesProxy<'_>, updates: &watch::Sender<bool>) {
    let mode = match properties.get(SERVICE.try_into().unwrap(), PROPERTY).await {
        Ok(value) => i32::try_from(value).ok(),
        Err(error) => {
            // The interface is normally absent on non-GNOME desktops.
            log::debug!("display_power: power mode unavailable: {error}");
            None
        }
    };
    // Mutter: 0 = on, 1 = standby, 2 = suspend, 3 = off, -1 = unsupported.
    let off = matches!(mode, Some(1..=3));
    updates.send_if_modified(|previous| {
        if *previous == off {
            false
        } else {
            *previous = off;
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, process::Stdio, time::Duration};
    use tokio::{io::AsyncBufReadExt, process::Command};

    struct DisplayConfig {
        mode: i32,
    }

    #[zbus::interface(name = "org.gnome.Mutter.DisplayConfig")]
    impl DisplayConfig {
        #[zbus(property)]
        fn power_save_mode(&self) -> i32 {
            self.mode
        }

        #[zbus(property)]
        fn set_power_save_mode(&mut self, mode: i32) {
            self.mode = mode;
        }
    }

    async fn expect_state(state: &mut watch::Receiver<bool>, expected: bool) {
        tokio::time::timeout(
            Duration::from_secs(3),
            state.wait_for(|value| *value == expected),
        )
        .await
        .expect("display power update timed out")
        .unwrap();
    }

    #[tokio::test]
    async fn initial_dpms_changes_invalidation_and_compositor_restart() {
        // Never change the real desktop's power state, even when tests run
        // inside an active GNOME session.
        let mut bus = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut address = String::new();
        tokio::io::BufReader::new(bus.stdout.take().unwrap())
            .read_line(&mut address)
            .await
            .unwrap();
        let server = zbus::connection::Builder::address(address.trim())
            .unwrap()
            .serve_at(PATH, DisplayConfig { mode: 3 })
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = zbus::connection::Builder::address(address.trim())
            .unwrap()
            .build()
            .await
            .unwrap();
        let (updates, mut state) = watch::channel(false);
        let monitor_task = tokio::spawn(monitor(client.clone(), updates));
        expect_state(&mut state, true).await;

        let properties = PropertiesProxy::builder(&client)
            .destination(SERVICE)
            .unwrap()
            .path(PATH)
            .unwrap()
            .build()
            .await
            .unwrap();
        for mode in [0i32, 1, 0, 2, 0, 3, -1] {
            properties
                .set(SERVICE.try_into().unwrap(), PROPERTY, &mode.into())
                .await
                .unwrap();
            expect_state(&mut state, matches!(mode, 1..=3)).await;
        }

        let interface = server
            .object_server()
            .interface::<_, DisplayConfig>(PATH)
            .await
            .unwrap();
        interface.get_mut().await.mode = 3;
        server
            .emit_signal(
                None::<&str>,
                PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(
                    SERVICE,
                    HashMap::<&str, zbus::zvariant::Value<'_>>::new(),
                    vec![PROPERTY],
                ),
            )
            .await
            .unwrap();
        expect_state(&mut state, true).await;

        server.release_name(SERVICE).await.unwrap();
        expect_state(&mut state, false).await;
        server.request_name(SERVICE).await.unwrap();
        expect_state(&mut state, true).await;
        properties
            .set(SERVICE.try_into().unwrap(), PROPERTY, &0i32.into())
            .await
            .unwrap();
        expect_state(&mut state, false).await;

        monitor_task.abort();
        let _ = monitor_task.await;
        bus.kill().await.unwrap();
    }
}
