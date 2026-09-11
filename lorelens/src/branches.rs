use super::*;

impl Lens {
  pub(super) fn branch_menu(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().downgrade();
    let current = self.status.branch.clone();
    let local = self.local_branches.clone();
    let remote = self.remote_branches.clone();
    let root = self.root.clone();
    Button::new("branch-menu")
      .icon(IconName::GitBranch)
      .h(px(36.))
      .min_w(px(140.))
      .label(if self.connected { format!("{current} ▾") } else { t("Branch · not connected") })
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
            let delete_view = view.clone();
            let delete_root = root.clone();
            let delete_branch = branch.clone();
            let archive_view = view.clone();
            let archive_root = root.clone();
            let archive_branch = branch.clone();
            submenu
              .item(PopupMenuItem::new(t("Switch to branch")).disabled(is_current).on_click(move |_, _, cx| {
                let _ = switch_view.update(cx, |this, cx| {
                  if this.root == switch_root && this.local_branches.contains(&source) {
                    this.command(commands::switch_branch_args(source.clone()), "Switch local branch", false, true, cx);
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
              .separator()
              .item(PopupMenuItem::new(t("Delete local branch…")).disabled(is_current).on_click(move |_, window, cx| {
                let _ = delete_view.update(cx, |this, cx| {
                  if this.root == delete_root {
                    this.archive_branch_dialog(delete_branch.clone(), false, window, cx);
                  }
                });
              }))
              .item(PopupMenuItem::new(t("Archive branch…")).disabled(is_current).on_click(move |_, window, cx| {
                let _ = archive_view.update(cx, |this, cx| {
                  if this.root == archive_root {
                    this.archive_branch_dialog(archive_branch.clone(), true, window, cx);
                  }
                });
              }))
          });
        }
        menu = menu.separator();
        let remote = remote.clone();
        let local = local.clone();
        let remote_view = view.clone();
        let remote_root = root.clone();
        menu.submenu(t("Remote"), window, cx, move |mut submenu, window, cx| {
          if remote.is_empty() {
            return submenu.label(t("No remote branches"));
          }
          for branch in &remote {
            let branch = branch.clone();
            let available_locally = local.contains(&branch);
            let branch_view = remote_view.clone();
            let branch_root = remote_root.clone();
            submenu = submenu.submenu(branch.clone(), window, cx, move |branch_menu, _, _| {
              let checkout_branch = branch.clone();
              let checkout_view = branch_view.clone();
              let checkout_root = branch_root.clone();
              branch_menu.item(
                PopupMenuItem::new(if available_locally { t("Available locally") } else { t("Check out branch") })
                  .disabled(available_locally)
                  .on_click(move |_, _, cx| {
                    let _ = checkout_view.update(cx, |this, cx| {
                      if this.root == checkout_root && !this.local_branches.contains(&checkout_branch) && this.remote_branches.contains(&checkout_branch) {
                        this.command(commands::switch_branch_args(checkout_branch.clone()), "Check out remote branch", false, true, cx);
                      }
                    });
                  }),
              )
            });
          }
          submenu
        })
      })
  }
}
