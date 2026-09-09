use super::*;

impl Lens {
  pub(super) fn branch_menu(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().downgrade();
    let current = self.status.branch.clone();
    let local = self.local_branches.clone();
    let remote = self.remote_branches.clone();
    let root = self.root.clone();
    Button::new("branch-menu")
      .label(if self.connected { format!("⑂ {current} ▾") } else { t("Branch · not connected") })
      .disabled(!enabled)
      .dropdown_menu(move |mut menu, window, cx| {
        menu = menu.min_w(px(280.)).max_w(px(360.));
        for (label, command, title) in [("Update / Sync", "sync", "Sync"), ("Push…", "push", "Push")] {
          let view = view.clone();
          let root = root.clone();
          menu = menu.item(PopupMenuItem::new(t(label)).on_click(move |_, _, cx| {
            let _ = view.update(cx, |this, cx| {
              if this.root == root {
                this.command(vec![command.into()], title, false, true, cx);
              }
            });
          }));
        }
        menu = menu.separator();
        let switch_view = view.clone();
        menu = menu.item(PopupMenuItem::new(t("Switch local branch…")).on_click(move |_, window, cx| {
          let _ = switch_view.update(cx, |this, cx| this.switch_branch_dialog(window, cx));
        }));
        for (label, is_remote) in [("New local branch…", false), ("New remote branch…", true)] {
          let view = view.clone();
          menu = menu.item(PopupMenuItem::new(t(label)).on_click(move |_, window, cx| {
            let _ = view.update(cx, |this, cx| this.new_branch_dialog(is_remote, window, cx));
          }));
        }
        menu = menu.separator().label(t("Local"));
        for branch in &local {
          let branch = branch.clone();
          let is_current = branch == current;
          let label = if is_current { format!("✓ {branch}") } else { branch.clone() };
          let view = view.clone();
          let root = root.clone();
          let target = current.clone();
          menu = menu.submenu(label, window, cx, move |submenu, _, _| {
            let switch_view = view.clone();
            let switch_root = root.clone();
            let source = branch.clone();
            let merge_view = view.clone();
            let merge_root = root.clone();
            let merge_source = branch.clone();
            submenu
              .item(PopupMenuItem::new(t("Switch to branch")).disabled(is_current).on_click(move |_, _, cx| {
                let _ = switch_view.update(cx, |this, cx| {
                  if this.root == switch_root && this.local_branches.contains(&source) {
                    this.command(
                      vec!["branch".into(), "switch".into(), "--local".into(), "--".into(), source.clone()],
                      "Switch local branch",
                      false,
                      true,
                      cx,
                    );
                  }
                });
              }))
              .item(
                PopupMenuItem::new(tf("Merge into '{target}'…", &[("target", target.to_string())]))
                  .disabled(is_current)
                  .on_click(move |_, window, cx| {
                    let _ = merge_view.update(cx, |this, cx| {
                      if this.root == merge_root {
                        this.merge_branch_dialog(merge_source.clone(), window, cx);
                      }
                    });
                  }),
              )
          });
        }
        menu = menu.separator();
        let remote = remote.clone();
        menu.submenu(t("Remote"), window, cx, move |mut submenu, _, _| {
          if remote.is_empty() {
            return submenu.label(t("No remote branches"));
          }
          for branch in &remote {
            submenu = submenu.label(branch.clone());
          }
          submenu
        })
      })
  }
}
