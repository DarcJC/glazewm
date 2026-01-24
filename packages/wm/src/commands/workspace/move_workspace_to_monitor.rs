use anyhow::Context;
use wm_common::WmEvent;

use super::{activate_workspace, deactivate_workspace, sort_workspaces};
use crate::{
  commands::container::move_container_within_tree,
  models::{Monitor, Workspace},
  traits::{CommonGetters, PositionGetters, WindowGetters},
  user_config::UserConfig,
  wm_state::WmState,
};

/// Moves the given workspace to the target monitor by its index.
pub fn move_workspace_to_monitor(
  workspace: &Workspace,
  monitor_index: usize,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let monitors = state.monitors();

  let target_monitor = monitors.get(monitor_index).with_context(|| {
    format!("Monitor at index {monitor_index} was not found.")
  })?;

  // Skip if already on the target monitor.
  let origin_monitor = workspace.monitor().context("No monitor.")?;
  if origin_monitor.id() == target_monitor.id() {
    return Ok(());
  }

  move_workspace_to_monitor_impl(workspace, target_monitor, state, config)
}

/// Internal implementation for moving a workspace to a specific monitor.
pub fn move_workspace_to_monitor_impl(
  workspace: &Workspace,
  target_monitor: &Monitor,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let origin_monitor = workspace.monitor().context("No monitor.")?;

  move_container_within_tree(
    &workspace.clone().into(),
    &target_monitor.clone().into(),
    target_monitor.child_count(),
    state,
  )?;

  let windows = workspace
    .descendants()
    .filter_map(|descendant| descendant.as_window_container().ok());

  for window in windows {
    window.set_has_pending_dpi_adjustment(true);

    window.set_floating_placement(
      window
        .floating_placement()
        .translate_to_center(&workspace.to_rect()?),
    );
  }

  // Get currently displayed workspace on the target monitor.
  let displayed_workspace = target_monitor
    .displayed_workspace()
    .context("No displayed workspace.")?;

  state
    .pending_sync
    .queue_cursor_jump()
    .queue_container_to_redraw(workspace.clone())
    .queue_container_to_redraw(displayed_workspace);

  match origin_monitor.child_count() {
    0 => {
      // Prevent origin monitor from having no workspaces.
      activate_workspace(None, Some(origin_monitor), state, config)?;
    }
    _ => {
      // Redraw the workspace on the origin monitor.
      state.pending_sync.queue_container_to_redraw(
        origin_monitor
          .displayed_workspace()
          .context("No displayed workspace.")?,
      );
    }
  }

  // Get empty workspace to destroy (if one is found). Cannot destroy
  // empty workspaces if they're the only workspace on the monitor.
  let workspace_to_destroy =
    target_monitor.workspaces().into_iter().find(|workspace| {
      !workspace.config().keep_alive
        && !workspace.has_children()
        && !workspace.is_displayed()
    });

  if let Some(workspace) = workspace_to_destroy {
    deactivate_workspace(workspace, state)?;
  }

  sort_workspaces(target_monitor, config)?;

  state.emit_event(WmEvent::WorkspaceUpdated {
    updated_workspace: workspace.to_dto()?,
  });

  Ok(())
}
