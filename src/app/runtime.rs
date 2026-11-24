use super::switching::RepeatSwitch;
use super::*;

pub(super) struct CycleStatus {
    pub(super) missing_frontmost_window: Option<AppInstanceId>,
    pub(super) close_recheck: Option<Instant>,
    pub(super) active: Vec<WorkspaceId>,
    pub(super) layouts: Vec<status_item::WorkspaceLayout>,
    pub(super) menu: Vec<status_item::DisplayWorkspaces>,
    pub(super) failed: usize,
    pub(super) hud: Option<UiHudEvent>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct RuntimeDebug {
    frontmost: Option<AppInstanceId>,
    focused: Option<crate::PhysicalWindowId>,
    active_workspaces: Vec<WorkspaceId>,
    displays: Vec<DebugDisplay>,
    tiles: Vec<NavigationTile>,
    floating: Vec<DebugFloatingWindow>,
    /// Managed windows left visible across workspace switches because they
    /// repeatedly reject parking positions.
    parking_exempt: Vec<crate::PhysicalWindowId>,
    targets: DebugTargets,
}

#[derive(Clone, Debug, serde::Serialize)]
struct DebugFloatingWindow {
    window: crate::PhysicalWindowId,
    workspace: WorkspaceId,
    /// Present exactly when the window is parked off-screen.
    parked_original_frame: Option<crate::Rect>,
}

#[derive(Clone, Debug, serde::Serialize)]
struct DebugDisplay {
    id: String,
    name: String,
    frame: crate::Rect,
    workspace: WorkspaceId,
    axis: crate::Axis,
    fullscreen_layout: bool,
    layout: Vec<crate::Rect>,
}

#[derive(Clone, Debug, serde::Serialize)]
struct DebugTargets {
    left: Option<crate::PhysicalWindowId>,
    down: Option<crate::PhysicalWindowId>,
    up: Option<crate::PhysicalWindowId>,
    right: Option<crate::PhysicalWindowId>,
}

pub(super) struct LayoutStatus {
    pub(super) failed: usize,
    pub(super) blocked: bool,
}

#[derive(Clone)]
pub(super) struct ResolvedWorkspace {
    pub(super) workspace: WorkspaceId,
    pub(super) selector: DisplaySelector,
    pub(super) display_id: String,
    pub(super) output: crate::Rect,
}

pub(super) struct WorkspaceRuntime<B = MacWorkspaceBackend> {
    pub(super) backend: B,
    pub(super) scope: ManagementScope,
    pub(super) workspaces: Workspaces,
    pub(super) known_apps: [HashMap<AppInstanceId, String>; WorkspaceId::COUNT],
    pub(super) constrained:
        [HashMap<crate::PhysicalWindowId, (crate::Rect, crate::Rect)>; WorkspaceId::COUNT],
    /// Target frames reported as `Applied` on the latest layout pass, awaiting
    /// observation readback. Electron apps accept `AXSize` writes synchronously
    /// and clamp to their aspect-ratio or size limits asynchronously afterward.
    pub(super) applied: [HashMap<crate::PhysicalWindowId, cycle::AppliedFrame>; WorkspaceId::COUNT],
    /// Consecutive bounced or rejected applications per window for the same
    /// target frame; past `APPLY_ATTEMPT_LIMIT` the target is frozen.
    pub(super) apply_attempts:
        [HashMap<crate::PhysicalWindowId, (crate::Rect, u8)>; WorkspaceId::COUNT],
    pub(super) parked: HashMap<crate::PhysicalWindowId, crate::Rect>,
    /// When each window was last parked. Apps apply position writes
    /// asynchronously, so a fresh park must settle before an on-screen
    /// observation may be trusted as a genuine reappearance.
    pub(super) recently_parked: HashMap<crate::PhysicalWindowId, Instant>,
    /// When each window last un-parked itself (confirmed by the window server).
    pub(super) recent_heals: HashMap<crate::PhysicalWindowId, Instant>,
    /// Consecutive re-parks undone by the app; past the limit the window's
    /// reappearance is honored instead of fought.
    pub(super) repark_attempts: HashMap<crate::PhysicalWindowId, u8>,
    /// Windows that repeatedly un-park themselves. They remain managed and
    /// tiled, but are left visible across workspace switches for the session.
    pub(super) parking_exempt: HashSet<crate::PhysicalWindowId>,
    /// Bound windows omitted from the AX snapshot but still listed by CG,
    /// with their settling and close-recheck state.
    pub(super) omitted_windows: HashMap<crate::PhysicalWindowId, cycle::Omission>,
    /// Owned windows Accessibility reports gone while the window server still
    /// lists them, and when that began.
    pub(super) inaccessible_since: HashMap<crate::PhysicalWindowId, Instant>,
    /// Smallest width and height each window has refused to shrink below.
    pub(super) minimum_sizes: HashMap<crate::PhysicalWindowId, (i32, i32)>,
    /// Windows the user floated with Toggle Float. They stay floating (and
    /// survive agent restarts) until toggled back, unlike a rule's windows.
    pub(super) manual_floating: HashSet<crate::PhysicalWindowId>,
    /// The Scratchpad Lab's windows; hidden ones are parked and owned by no workspace.
    pub(super) scratchpad: crate::scratchpad::ScratchpadState,
    pub(super) config: Config,
    pub(super) foreground_lanes: HashMap<String, DisplaySelector>,
    /// Global focus history across displays: the workspace focused now and the
    /// one focused before it. `Option+Tab` reads it, so it bounces between the
    /// last two workspaces wherever they live.
    pub(super) last_focused_workspace: Option<WorkspaceId>,
    pub(super) previous_focused_workspace: Option<WorkspaceId>,
    /// Focus left behind by a workspace switch or move. Focus follow ignores
    /// it until Accessibility reports a different focused window, so a hidden
    /// window that keeps (or has not yet lost) focus cannot pull the display
    /// back to its workspace.
    pub(super) suppressed_focus: Option<crate::PhysicalWindowId>,
    /// The frontmost application at that moment, for when Accessibility
    /// reports no focused window for it (or one that is not frontmost).
    pub(super) suppressed_app: Option<crate::AppInstanceId>,
    /// The window a focus command just focused, until Accessibility reports it.
    pub(super) pending_focus: Option<super::commands::PendingFocus>,
    pub(super) warp_context: super::commands::WarpContext,
    pub(super) pending_fullscreen_focus_restore: HashSet<WorkspaceId>,
    pub(super) exact_spacing_workspaces: HashSet<WorkspaceId>,
    pub(super) persisted_workspace_state: crate::workspace::WorkspaceState,
    /// Title-free placements that survive application relaunches and reboots.
    pub(super) memory: MemoryState,
    pub(super) journal: JournalState,
    pub(super) displays: super::displays::DisplayWatch,
}

impl WorkspaceRuntime<MacWorkspaceBackend> {
    pub(super) fn new(
        scope: ManagementScope,
        parked: HashMap<crate::PhysicalWindowId, crate::Rect>,
        config: Config,
        workspace_state: Option<crate::workspace::WorkspaceState>,
    ) -> Self {
        let backend = MacWorkspaceBackend::new();
        Self::with_backend_config_state(scope, backend, parked, config, workspace_state)
    }
}

impl<B: WorkspaceBackend> WorkspaceRuntime<B> {
    /// Rules apply to windows from now on; named workspaces and display assignments update.
    /// New named workspaces get their hotkeys at the next restart.
    pub(super) fn replace_config(&mut self, config: Config) {
        if config.spacing() != self.config.spacing()
            || config.display_overrides() != self.config.display_overrides()
        {
            self.exact_spacing_workspaces.extend(WorkspaceId::all());
        }
        config.configure_workspaces(&mut self.workspaces);
        self.config = config;
    }

    pub(super) fn execute(
        &mut self,
        command: WorkerCommand,
        cancelled: &AtomicBool,
    ) -> Option<WorkerResult> {
        if let WorkerCommand::Action(_, outputs) | WorkerCommand::Gesture(_, _, outputs) = &command
        {
            self.synchronize_foreground(outputs);
            self.follow_displays(outputs);
        }
        let (result, outputs) = match command {
            WorkerCommand::Action(action, outputs) => (
                self.execute_action(action, outputs.clone(), cancelled),
                outputs,
            ),
            WorkerCommand::Gesture(gesture, preview, outputs) => {
                let window = match &gesture {
                    FinalGesture::Drop(window, _) | FinalGesture::Resize(window, _) => window,
                };
                for attempts in &mut self.apply_attempts {
                    attempts.remove(window);
                }
                let result =
                    self.with_warp_context(super::commands::WarpContext::Suppressed, |rt| {
                        match gesture {
                            FinalGesture::Drop(window, point) => {
                                rt.drop_window(window, point, preview, outputs.clone(), cancelled)
                            }
                            FinalGesture::Resize(window, frame) => {
                                rt.resize_window(window, frame, preview, outputs.clone(), cancelled)
                            }
                        }
                    });
                (result, outputs)
            }
            WorkerCommand::RestoreAll(outputs) => {
                if let Some(outputs) = &outputs {
                    self.note_outputs(outputs);
                }
                let requested = self.parked.len();
                // Restored scratchpad windows are ordinary windows when tiling resumes.
                self.scratchpad.clear();
                let failed = self.restore_all();
                self.journal_restore("restore all", requested, failed);
                // Pause, quit and restart all restore first; the layout they
                // resume from must be on disk before the process can exit.
                self.flush_workspace_state();
                return Some(WorkerResult::Restored { failed });
            }
            WorkerCommand::Stop => {
                self.journal_event(crate::journal::JournalEvent::Stop);
                self.flush_workspace_state();
                return None;
            }
        };
        self.sync_scratchpad(&outputs);
        self.check_arrangement(&outputs);
        Some(WorkerResult::Status {
            result,
            debug: Box::new(self.debug_state(&outputs)),
        })
    }

    fn execute_action(
        &mut self,
        action: WorkerAction,
        outputs: macos::DisplayOutputs,
        cancelled: &AtomicBool,
    ) -> Result<CycleStatus, String> {
        // A switch keeps a focus command still in flight: pressing the active
        // workspace's shortcut again steps from the window the last press
        // focused, which Accessibility may not report yet. Every other action
        // here starts from whatever focus is reported now.
        if matches!(
            action,
            WorkerAction::SelectWorkspace(_)
                | WorkerAction::WorkspaceBackAndForth
                | WorkerAction::MoveWorkspaceToDisplay(_)
                | WorkerAction::Swipe(..)
                | WorkerAction::MoveFocused(_)
                | WorkerAction::MoveApp(..)
        ) {
            self.pending_focus = None;
        }
        let warp_context = if matches!(
            action,
            WorkerAction::SwitchWorkspace(_)
                | WorkerAction::WorkspaceBackAndForth
                | WorkerAction::MoveWorkspaceToDisplay(_)
                | WorkerAction::CycleFocus
                | WorkerAction::Focus(_)
                | WorkerAction::MoveFocused(_)
        ) {
            super::commands::WarpContext::Explicit
        } else {
            super::commands::WarpContext::Suppressed
        };
        self.with_warp_context(warp_context, |rt| match action {
            WorkerAction::Retile => {
                let mut status = rt.cycle(outputs.clone(), cancelled)?;
                if !rt.exact_spacing_workspaces.is_empty() && !rt.backend.mouse_down() {
                    for r in rt.visible_workspaces(&outputs) {
                        if rt.scope.manages_selector(&r.selector)
                            && rt.exact_spacing_workspaces.contains(&r.workspace)
                            && !rt.workspaces.windows(r.workspace).is_empty()
                        {
                            status.failed +=
                                rt.layout_workspace(r.workspace, r.output, cancelled).failed;
                        }
                    }
                }
                Ok(status)
            }
            WorkerAction::SwitchWorkspace(workspace) => {
                rt.switch_with(workspace, outputs, cancelled, RepeatSwitch::CycleFocus)
            }
            // A menu-bar or HUD click on the workspace already up re-focuses it;
            // clicking it again must not jump to another window.
            WorkerAction::SelectWorkspace(workspace) => {
                rt.switch_with(workspace, outputs, cancelled, RepeatSwitch::KeepFocus)
            }
            WorkerAction::WorkspaceBackAndForth => rt.workspace_back_and_forth(outputs, cancelled),
            WorkerAction::MoveWorkspaceToDisplay(target) => {
                rt.move_workspace_to_display(target, outputs, cancelled)
            }
            WorkerAction::Swipe(swipe, point) => rt.swipe(swipe, point, outputs, cancelled),
            WorkerAction::MoveFocused(workspace) => rt.move_focused(workspace, outputs, cancelled),
            WorkerAction::MoveApp(bundle_id, workspace) => {
                rt.move_app(&bundle_id, workspace, outputs, cancelled)
            }
            WorkerAction::CycleFocus => rt.cycle_focus(outputs, cancelled),
            WorkerAction::Focus(direction) => rt.focus_relative(direction, outputs, cancelled),
            WorkerAction::Reorder(direction) => rt.reorder_focused(direction, outputs, cancelled),
            WorkerAction::ToggleLayout => rt.toggle_layout(outputs, cancelled),
            WorkerAction::Resize(direction) => rt.resize_focused(direction, outputs, cancelled),
            WorkerAction::Balance => rt.balance_focused(outputs, cancelled),
            WorkerAction::ToggleFullscreenLayout => rt.toggle_fullscreen_layout(outputs, cancelled),
            WorkerAction::ToggleFloat => rt.toggle_float(outputs, cancelled),
            WorkerAction::Scratchpad(command) => rt.scratchpad_command(command, outputs, cancelled),
            WorkerAction::MarkWrong => {
                rt.mark_wrong();
                Ok(rt.status(0, &outputs))
            }
        })
    }

    #[cfg(test)]
    pub(super) fn with_backend(
        scope: ManagementScope,
        backend: B,
        parked: HashMap<crate::PhysicalWindowId, crate::Rect>,
    ) -> Self {
        Self::with_backend_and_config(scope, backend, parked, Config::default())
    }

    #[cfg(test)]
    pub(super) fn with_backend_and_config(
        scope: ManagementScope,
        backend: B,
        parked: HashMap<crate::PhysicalWindowId, crate::Rect>,
        config: Config,
    ) -> Self {
        Self::with_backend_config_state(scope, backend, parked, config, None)
    }

    pub(super) fn with_backend_config_state(
        scope: ManagementScope,
        backend: B,
        parked: HashMap<crate::PhysicalWindowId, crate::Rect>,
        config: Config,
        workspace_state: Option<crate::workspace::WorkspaceState>,
    ) -> Self {
        let restored_workspace_state = workspace_state.is_some();
        let mut workspaces = Workspaces::default();
        config.configure_workspaces(&mut workspaces);
        if let Some(state) = workspace_state {
            workspaces.restore(state);
        }
        let pending_fullscreen_focus_restore = if restored_workspace_state {
            WorkspaceId::all()
                .filter(|workspace| {
                    workspaces.uses_fullscreen_layout(*workspace)
                        && scope.manages_selector(workspaces.assignment(*workspace))
                })
                .collect()
        } else {
            HashSet::new()
        };
        let known_apps = std::array::from_fn(|index| {
            workspaces
                .app_instances(WorkspaceId::from_index(index))
                .map(|app| (app.clone(), String::new()))
                .collect()
        });
        let persisted_workspace_state = workspaces.state();
        let manual_floating = super::commands::restored_floating(&workspaces);
        let has_spacing = !config.spacing().is_none() || !config.display_overrides().is_empty();
        Self {
            backend,
            scope,
            workspaces,
            known_apps,
            constrained: std::array::from_fn(|_| HashMap::new()),
            applied: std::array::from_fn(|_| HashMap::new()),
            apply_attempts: std::array::from_fn(|_| HashMap::new()),
            parked,
            recently_parked: HashMap::new(),
            recent_heals: HashMap::new(),
            repark_attempts: HashMap::new(),
            parking_exempt: HashSet::new(),
            omitted_windows: HashMap::new(),
            inaccessible_since: HashMap::new(),
            minimum_sizes: HashMap::new(),
            manual_floating,
            scratchpad: crate::scratchpad::ScratchpadState::default(),
            config,
            foreground_lanes: HashMap::new(),
            last_focused_workspace: None,
            previous_focused_workspace: None,
            suppressed_focus: None,
            suppressed_app: None,
            pending_focus: None,
            warp_context: super::commands::WarpContext::Direct,
            pending_fullscreen_focus_restore,
            exact_spacing_workspaces: WorkspaceId::all().filter(|_| has_spacing).collect(),
            persisted_workspace_state,
            memory: MemoryState::default(),
            journal: JournalState::default(),
            displays: super::displays::DisplayWatch::default(),
        }
    }

    pub(super) fn restore_window_memory(&mut self, memory: crate::window_memory::WindowMemory) {
        self.memory = MemoryState {
            memory,
            started_at: Some(Instant::now()),
            ..MemoryState::default()
        };
    }

    pub(super) fn visible_workspaces(
        &self,
        outputs: &macos::DisplayOutputs,
    ) -> Vec<ResolvedWorkspace> {
        outputs
            .displays()
            .iter()
            .filter_map(|display| {
                let candidates = || {
                    self.workspaces
                        .active_lanes()
                        .filter(|(selector, _)| outputs.resolve(selector).id == display.id)
                };
                let selected = self
                    .foreground_lanes
                    .get(&display.id)
                    .and_then(|foreground| {
                        candidates().find(|(selector, _)| *selector == foreground)
                    })
                    .or_else(|| {
                        candidates()
                            .filter(|(selector, _)| selector_available(selector, outputs))
                            .min_by_key(|(_, workspace)| *workspace)
                    })
                    .or_else(|| candidates().min_by_key(|(_, workspace)| *workspace))?;
                Some(ResolvedWorkspace {
                    workspace: selected.1,
                    selector: selected.0.clone(),
                    display_id: display.id.clone(),
                    output: display.frame,
                })
            })
            .collect()
    }

    pub(super) fn is_visible(
        &self,
        workspace: WorkspaceId,
        outputs: &macos::DisplayOutputs,
    ) -> bool {
        self.visible_workspaces(outputs)
            .iter()
            .any(|resolved| resolved.workspace == workspace)
    }

    pub(super) fn synchronize_foreground(&mut self, outputs: &macos::DisplayOutputs) {
        let disconnected: Vec<_> = self
            .foreground_lanes
            .extract_if(|id, _| !outputs.displays().iter().any(|display| &display.id == id))
            .collect();
        for (_, selector) in disconnected {
            self.foreground_lanes
                .insert(outputs.resolve(&selector).id.clone(), selector);
        }
    }

    pub(super) fn bundle_id(&self, app: &AppInstanceId) -> Option<&str> {
        self.memory
            .bundles
            .get(app)
            .or_else(|| {
                self.known_apps
                    .iter()
                    .find_map(|apps| apps.get(app))
                    .filter(|bundle| !bundle.is_empty())
            })
            .map(String::as_str)
    }

    pub(super) fn ensure_managed_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<DisplaySelector, String> {
        if !self.workspaces.is_enabled(workspace) {
            return Err(format!("Workspace {workspace} is not configured"));
        }
        let selector = self.workspaces.assignment(workspace).clone();
        if !self.scope.manages_selector(&selector) {
            return Err(format!(
                "Workspace {workspace} is outside the managed display scope"
            ));
        }
        Ok(selector)
    }

    pub(super) fn exit_fullscreen_if_split(&mut self, workspace: WorkspaceId) {
        if !self.workspaces.uses_fullscreen_layout(workspace) {
            return;
        }
        let bound_count = self
            .workspaces
            .engine(workspace)
            .model()
            .workspace
            .slots
            .iter()
            .filter(|slot| slot.bound_window().is_some())
            .count();
        if bound_count > 1 {
            log_event!("workspace {workspace}: exited Fullscreen Layout ({bound_count} tiles)");
            self.workspaces.set_fullscreen_layout(workspace, false);
            self.pending_fullscreen_focus_restore.remove(&workspace);
        }
    }

    pub(super) fn spatial_target(
        &self,
        focused: &crate::PhysicalWindowId,
        direction: Direction,
        outputs: &macos::DisplayOutputs,
    ) -> Option<(WorkspaceId, crate::PhysicalWindowId)> {
        let tiles = self.navigation_tiles(outputs);
        crate::navigation::target(&tiles, focused, direction)
            .map(|target| (target.workspace, target.window.clone()))
    }

    pub(super) fn navigation_tiles(&self, outputs: &macos::DisplayOutputs) -> Vec<NavigationTile> {
        self.navigation_tiles_for(&self.visible_workspaces(outputs))
    }

    pub(super) fn navigation_tiles_for(
        &self,
        visible: &[ResolvedWorkspace],
    ) -> Vec<NavigationTile> {
        let mut tiles = Vec::new();
        for resolved in visible {
            let settings = self.layout_settings(resolved.output);
            let mut placements = self
                .workspaces
                .engine(resolved.workspace)
                .placements(&settings);
            if self.workspaces.uses_fullscreen_layout(resolved.workspace) {
                for placement in &mut placements {
                    placement.frame = settings.single();
                }
            }
            tiles.extend(placements.into_iter().filter_map(|placement| {
                placement.window.map(|window| NavigationTile {
                    workspace: resolved.workspace,
                    window,
                    frame: placement.frame,
                })
            }));
        }
        tiles
    }

    pub(super) fn debug_state(&mut self, outputs: &macos::DisplayOutputs) -> RuntimeDebug {
        let visible = self.visible_workspaces(outputs);
        let tiles = self.navigation_tiles_for(&visible);
        let focused = self.backend.focused_window();
        let target = |direction| {
            focused.as_ref().and_then(|focused| {
                crate::navigation::target(&tiles, focused, direction)
                    .map(|tile| tile.window.clone())
            })
        };
        let targets = DebugTargets {
            left: target(Direction::Left),
            down: target(Direction::Down),
            up: target(Direction::Up),
            right: target(Direction::Right),
        };
        let floating = self
            .workspaces
            .floating_entries()
            .map(|(window, workspace)| DebugFloatingWindow {
                parked_original_frame: self.parked.get(window).copied(),
                window: window.clone(),
                workspace,
            })
            .collect();
        RuntimeDebug {
            frontmost: self.backend.frontmost_application(),
            focused,
            active_workspaces: visible.iter().map(|resolved| resolved.workspace).collect(),
            displays: visible
                .into_iter()
                .filter_map(|resolved| {
                    let display = outputs
                        .displays()
                        .iter()
                        .find(|display| display.id == resolved.display_id)?;
                    Some(DebugDisplay {
                        id: display.id.clone(),
                        name: display.name.clone(),
                        frame: display.frame,
                        workspace: resolved.workspace,
                        axis: self.workspaces.engine(resolved.workspace).root_axis(),
                        fullscreen_layout: self
                            .workspaces
                            .uses_fullscreen_layout(resolved.workspace),
                        layout: self.preview_layout(resolved.workspace, HUD_PREVIEW_RECT),
                    })
                })
                .collect(),
            tiles,
            floating,
            parking_exempt: {
                let mut windows: Vec<_> = self.parking_exempt.iter().cloned().collect();
                windows.sort_by_key(|window| (window.app.pid, window.window_id));
                windows
            },
            targets,
        }
    }

    pub(super) fn layout_workspace(
        &mut self,
        workspace: WorkspaceId,
        output: crate::Rect,
        cancelled: &AtomicBool,
    ) -> LayoutStatus {
        self.layout_workspace_during_drag(workspace, output, None, cancelled)
    }

    pub(super) fn layout_workspace_during_drag(
        &mut self,
        workspace: WorkspaceId,
        output: crate::Rect,
        dragged: Option<&crate::PhysicalWindowId>,
        cancelled: &AtomicBool,
    ) -> LayoutStatus {
        let (status, results) = self.apply_layout(workspace, output, dragged, cancelled);
        if dragged.is_some() {
            return status;
        }
        self.refit_minimum_sizes(workspace, output, results, cancelled)
            .unwrap_or(status)
    }

    pub(super) fn apply_layout(
        &mut self,
        workspace: WorkspaceId,
        output: crate::Rect,
        dragged: Option<&crate::PhysicalWindowId>,
        cancelled: &AtomicBool,
    ) -> (LayoutStatus, Vec<macos::ApplyResult>) {
        let mut settings = self.layout_settings(output);
        if dragged.is_none() && self.exact_spacing_workspaces.contains(&workspace) {
            settings.frame_tolerance = 0;
        }
        let engine = self.workspaces.engine_mut(workspace);
        engine.fit_minimums(&settings, &self.minimum_sizes);
        let mut placements = engine.placements(&settings);
        self.apply_fullscreen_layout(workspace, &mut placements, &settings);
        let attempts = &mut self.apply_attempts[workspace.index()];
        for placement in &placements {
            if let Some(window) = placement.window.as_ref() {
                attempts.remove(window);
            }
        }
        let results = match dragged {
            Some(d) => self
                .backend
                .apply_managed_during_drag(&placements, &settings, d, cancelled),
            None => self
                .backend
                .apply_managed(&placements, &settings, cancelled),
        };
        if settings.frame_tolerance == 0 && !cancelled.load(Ordering::Acquire) {
            self.exact_spacing_workspaces.remove(&workspace);
        }
        // Managed layout runs without a fresh observation; the next cycle validates readback.
        self.record_results(workspace, &results);
        self.retire_laid_out(&results);
        let failed = failed_results(&results);
        let expected = placements.iter().filter(|p| p.window.is_some()).count();
        // Deferred means a drag is in progress and layout retries next cycle;
        // it must not be treated as a failure that rolls back a switch.
        let blocked = results.len() != expected
            || results.iter().any(|result| {
                matches!(
                    result.outcome,
                    macos::ApplyOutcome::Gone | macos::ApplyOutcome::Unavailable
                )
            });
        (LayoutStatus { failed, blocked }, results)
    }

    pub(super) fn forget_window(
        &mut self,
        workspace: WorkspaceId,
        window: &crate::PhysicalWindowId,
    ) {
        self.workspaces.forget_window(workspace, window);
        self.clear_fullscreen_restore_if_inactive(workspace);
        self.workspaces.forget_floating(window);
        self.constrained[workspace.index()].remove(window);
        self.applied[workspace.index()].remove(window);
        self.apply_attempts[workspace.index()].remove(window);
        self.omitted_windows.remove(window);
        self.recently_parked.remove(window);
        self.recent_heals.remove(window);
        self.repark_attempts.remove(window);
        self.inaccessible_since.remove(window);
        self.minimum_sizes.remove(window);
        self.manual_floating.remove(window);
    }

    pub(super) fn clear_fullscreen_restore_if_inactive(&mut self, workspace: WorkspaceId) {
        if !self.workspaces.uses_fullscreen_layout(workspace) {
            self.pending_fullscreen_focus_restore.remove(&workspace);
        }
    }

    pub(super) fn apply_fullscreen_layout(
        &mut self,
        workspace: WorkspaceId,
        placements: &mut [crate::Placement],
        settings: &Settings,
    ) {
        if !self.workspaces.uses_fullscreen_layout(workspace) {
            return;
        }
        let frame = settings.single();
        for placement in placements {
            placement.frame = frame;
            self.workspaces
                .engine_mut(workspace)
                .record_placement_target(placement.slot, frame);
        }
    }

    pub(super) fn status(&self, failed: usize, outputs: &macos::DisplayOutputs) -> CycleStatus {
        self.status_for_visible(failed, &self.visible_workspaces(outputs), outputs)
    }

    fn flush_workspace_state(&mut self) {
        if let Err(error) = self.persist_workspace_state_if_changed() {
            log_event!("failed to persist workspace state: {error}");
        }
        self.backend.flush_state();
    }

    pub(super) fn persist_workspace_state_if_changed(&mut self) -> Result<(), String> {
        let state = self.workspaces.state();
        if state == self.persisted_workspace_state {
            return Ok(());
        }
        self.backend.persist_workspace_state(&state)?;
        self.persisted_workspace_state = state;
        Ok(())
    }

    fn preview_layout(&self, workspace: WorkspaceId, output: crate::Rect) -> Vec<crate::Rect> {
        let mut layout = self.workspaces.engine(workspace).preview(output);
        if self.workspaces.uses_fullscreen_layout(workspace) {
            layout.fill(output);
        }
        layout
    }

    pub(super) fn status_for_visible(
        &self,
        failed: usize,
        visible: &[ResolvedWorkspace],
        outputs: &macos::DisplayOutputs,
    ) -> CycleStatus {
        let active: Vec<_> = visible
            .iter()
            .filter(|resolved| self.scope.manages_selector(&resolved.selector))
            .map(|resolved| resolved.workspace)
            .collect();
        CycleStatus {
            missing_frontmost_window: None,
            close_recheck: None,
            layouts: active
                .iter()
                .map(|workspace| status_item::WorkspaceLayout {
                    workspace: workspace.to_string(),
                    mode: self.workspace_preview(*workspace),
                })
                .collect(),
            menu: self.workspace_menu(visible, outputs),
            active,
            failed,
            hud: None,
        }
    }

    /// A display's visible workspace and every other one holding windows, in order:
    /// the rows of its menu section and the stops of a swipe.
    pub(super) fn listed_workspaces(
        &self,
        display_id: &str,
        current: Option<WorkspaceId>,
        outputs: &macos::DisplayOutputs,
    ) -> Vec<WorkspaceId> {
        WorkspaceId::all()
            .filter(|workspace| {
                let selector = self.workspaces.assignment(*workspace);
                self.workspaces.is_enabled(*workspace)
                    && self.scope.manages_selector(selector)
                    && outputs.resolve(selector).id == display_id
                    && (current == Some(*workspace)
                        || !self.workspaces.windows(*workspace).is_empty())
            })
            .collect()
    }

    fn workspace_preview(&self, workspace: WorkspaceId) -> UiHudLayout {
        if self.workspaces.uses_fullscreen_layout(workspace) {
            UiHudLayout::Fullscreen
        } else {
            UiHudLayout::Split(self.preview_layout(workspace, HUD_PREVIEW_RECT))
        }
    }
}

pub(super) fn output_for(
    workspaces: &Workspaces,
    workspace: WorkspaceId,
    outputs: &macos::DisplayOutputs,
) -> crate::Rect {
    outputs.resolve(workspaces.assignment(workspace)).frame
}

fn selector_available(selector: &DisplaySelector, outputs: &macos::DisplayOutputs) -> bool {
    outputs.resolve_exact(selector).is_some()
}

pub(super) fn failed_results(results: &[macos::ApplyResult]) -> usize {
    results
        .iter()
        .filter(|r| r.outcome != macos::ApplyOutcome::Applied)
        .count()
}
