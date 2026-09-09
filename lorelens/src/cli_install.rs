use super::*;

pub(super) fn cli_missing(cli: &std::path::Path) -> bool {
    if cli.is_absolute() || cli.components().count() > 1 {
        !cli.is_file()
    } else {
        external_tools::resolve("lore", None).is_none()
    }
}

impl Lens {
    pub(super) fn install_cli_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog.title(t("Install Lore CLI")).confirm()
                .button_props(DialogButtonProps::default().cancel_text(t("Cancel")).ok_text(t("Install")))
                .child(t("Lore CLI was not found. Install it with winget install EpicGames.Lore and save its path to the user environment variable LORELENS_LORE_BIN? Accepting also accepts the package and source agreements."))
                .on_ok(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| this.install_cli(cx));
                    true
                })
        });
    }

    fn install_cli(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.notice = t("Installing Lore CLI…");
        let task = cx.background_executor().spawn(async move {
            use std::os::windows::process::CommandExt;
            let output = Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    include_str!("install_lore.ps1"),
                ])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| e.to_string())?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let log = format!("{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
            if !output.status.success() {
                return Err(log);
            }
            let path = stdout
                .lines()
                .find_map(|line| line.strip_prefix("LORELENS_INSTALLED_CLI="))
                .map(PathBuf::from)
                .filter(|path| path.is_absolute() && path.is_file())
                .ok_or_else(|| log.clone())?;
            Ok((path, log))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok((path, log)) => {
                        this.log(log);
                        this.cli = path.clone();
                        this.settings.cli = Some(path);
                        this.error = false;
                        if this.save_settings() {
                            this.notice =
                                t("Lore CLI installed. LORELENS_LORE_BIN has been saved.");
                            this.refresh(cx);
                        }
                    }
                    Err(error) => {
                        this.log(error.clone());
                        this.output = error;
                        this.output_title = t("Install Lore CLI");
                        this.notice = t("Lore CLI installation failed. See command log.");
                        this.error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
