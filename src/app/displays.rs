//! Display changes. Parked positions are chosen against the connected displays;
//! when a display is added or removed they may land on screen, and recovery
//! frames recorded on a removed display would restore off every screen.

use super::*;

/// The displays the runtime last saw, and the hidden windows still to re-park
/// since they changed.
#[derive(Default)]
pub(super) struct DisplayWatch {
    known: Option<macos::DisplayOutputs>,
    repark_pending: HashSet<crate::PhysicalWindowId>,
}

impl<B: WorkspaceBackend> WorkspaceRuntime<B> {
    /// How tiles fill `output`: its frame with the configured gaps, padding and width cap, as
    /// the `[[displays]]` entry for the display at `output` overrides them.
    pub(super) fn layout_settings(&self, output: crate::Rect) -> Settings {
        let display = self.displays.known.as_ref().and_then(|outputs| {
            outputs
                .displays()
                .iter()
                .find(|display| display.frame == output)
                .map(|display| (outputs, display))
        });
        let spacing = match display {
            Some((outputs, display)) => self
                .config
                .spacing_on(|selector| outputs.selects(selector, display)),
            None => self.config.spacing(),
        };
        Settings::for_output(output).spaced(spacing)
    }

    /// Record the current displays. When they differ from the last ones, every
    /// parked window is due a re-park against them.
    pub(super) fn note_outputs(&mut self, outputs: &macos::DisplayOutputs) {
        if self
            .displays
            .known
            .as_ref()
            .is_some_and(|known| !known.frames().eq(outputs.frames()))
        {
            log_event!("displays changed: re-parking hidden windows");
            self.displays.repark_pending = self.parked.keys().cloned().collect();
            self.repark_attempts.clear();
            // macOS moves windows off a removed display, and Spaceman parks and
            // restores them: a frame read back against a target applied before
            // the change says nothing about what the app accepts, so it must
            // not freeze the tile as refused.
            self.applied.iter_mut().for_each(HashMap::clear);
        }
        self.displays.known = Some(outputs.clone());
    }

    /// Note the displays, then re-park each hidden window still due one. Runs
    /// before focus follow and heal, which would otherwise take a window the
    /// new display uncovered for one the user brought back. A write that fails
    /// (the app is briefly inaccessible) stays due and is retried next time.
    pub(super) fn follow_displays(&mut self, outputs: &macos::DisplayOutputs) {
        self.note_outputs(outputs);
        if self.displays.repark_pending.is_empty() {
            return;
        }
        let visible: HashSet<_> = self
            .visible_workspaces(outputs)
            .into_iter()
            .map(|resolved| resolved.workspace)
            .collect();
        let displays: Vec<_> = outputs.frames().collect();
        for window in std::mem::take(&mut self.displays.repark_pending) {
            if !self.parked.contains_key(&window) || self.parking_exempt.contains(&window) {
                continue;
            }
            // Hidden scratchpad windows intentionally have no workspace owner, but still need to
            // stay parked off-screen when a newly connected display covers their parking edge.
            let output = if let Some(owner) = self.workspaces.workspace_of_any(&window) {
                if visible.contains(&owner) {
                    continue;
                }
                output_for(&self.workspaces, owner, outputs)
            } else if self
                .scratchpad
                .entry(&window)
                .is_some_and(|entry| entry.shown_on.is_none())
            {
                outputs.main().frame
            } else {
                continue;
            };
            if !self.repark_for_displays(&window, output, &displays) {
                self.displays.repark_pending.insert(window);
            }
        }
    }

    /// Whether `window` is parked for `displays` now.
    fn repark_for_displays(
        &mut self,
        window: &crate::PhysicalWindowId,
        output: crate::Rect,
        displays: &[crate::Rect],
    ) -> bool {
        let Some(current) = self.backend.window_server_frame(window) else {
            return false;
        };
        let target = crate::geometry::parked_frame(output, displays, current);
        if (target.x, target.y) == (current.x, current.y) {
            return true;
        }
        let outcome = self.backend.set_position(window, target);
        log_event!("displays changed: re-park {window:?} at {target:?} ({outcome:?})");
        if outcome != macos::ApplyOutcome::Applied {
            return false;
        }
        self.recently_parked.insert(window.clone(), Instant::now());
        true
    }

    /// Whether `window` still awaits its re-park after a display change. Until
    /// then a new display may show it, which is no reappearance to honor.
    pub(super) fn awaiting_display_repark(&self, window: &crate::PhysicalWindowId) -> bool {
        self.displays.repark_pending.contains(window)
    }

    /// Recover visible, non-parked owned windows whose on-screen window-server
    /// center sits outside every connected display (for example, macOS leaving
    /// a visible window with a bottom-right sliver after disconnecting a larger
    /// external display). Runs before `snapshot` so the window is observed back
    /// on its display instead of omitted and retired.
    pub(super) fn recover_stranded_visible(
        &mut self,
        outputs: &macos::DisplayOutputs,
        visible: &[ResolvedWorkspace],
        server_frames: &HashMap<u32, crate::Rect>,
        cancelled: &AtomicBool,
        failed: &mut usize,
    ) {
        if cancelled.load(Ordering::Acquire) || self.backend.mouse_down() {
            return;
        }
        let displays: Vec<_> = outputs.frames().collect();
        let has_candidate = visible.iter().any(|resolved| {
            self.scope.manages_selector(&resolved.selector)
                && self
                    .workspaces
                    .windows(resolved.workspace)
                    .into_iter()
                    .any(|window| {
                        !self.parked.contains_key(&window)
                            && server_frames.get(&window.window_id).is_some_and(|frame| {
                                !displays
                                    .iter()
                                    .any(|display| display.contains_center(*frame))
                            })
                    })
        });
        if !has_candidate {
            return;
        }
        let Some(onscreen_frames) = self.backend.onscreen_window_frames() else {
            return;
        };
        for resolved in visible {
            if cancelled.load(Ordering::Acquire) {
                return;
            }
            if !self.scope.manages_selector(&resolved.selector) {
                continue;
            }
            let workspace = resolved.workspace;
            let output = resolved.output;
            let settings = self.layout_settings(output);
            let engine = self.workspaces.engine_mut(workspace);
            engine.fit_minimums(&settings, &self.minimum_sizes);
            let mut placements = engine.placements(&settings);
            self.apply_fullscreen_layout(workspace, &mut placements, &settings);
            let mut recover_tiles = Vec::new();
            for placement in placements {
                let Some(window) = placement.window.as_ref() else {
                    continue;
                };
                if self.parked.contains_key(window) {
                    continue;
                }
                let Some(&frame) = onscreen_frames.get(&window.window_id) else {
                    continue;
                };
                if displays
                    .iter()
                    .any(|display| display.contains_center(frame))
                {
                    continue;
                }
                log_event!(
                    "recover stranded tiled {:?} on workspace {} from {:?} to {:?}",
                    window,
                    workspace,
                    frame,
                    placement.frame
                );
                recover_tiles.push(placement);
            }
            if !recover_tiles.is_empty() {
                let results = self
                    .backend
                    .apply_managed(&recover_tiles, &settings, cancelled);
                self.record_results(workspace, &results);
                let initially_failed = failed_results(&results);
                *failed += self
                    .refit_minimum_sizes(workspace, output, results, cancelled)
                    .map_or(initially_failed, |status| status.failed);
            }
            let floating: Vec<_> = self
                .workspaces
                .floating_entries()
                .filter(|(_, owner)| *owner == workspace)
                .map(|(window, _)| window.clone())
                .collect();
            for window in floating {
                if cancelled.load(Ordering::Acquire) {
                    return;
                }
                if self.parked.contains_key(&window) {
                    continue;
                }
                let Some(&frame) = onscreen_frames.get(&window.window_id) else {
                    continue;
                };
                if displays
                    .iter()
                    .any(|display| display.contains_center(frame))
                {
                    continue;
                }
                let target = clamp_into_output(frame, output);
                let outcome = self.backend.set_position(&window, target);
                log_event!(
                    "recover stranded floating {:?} on workspace {} from {:?} to {:?} ({:?})",
                    window,
                    workspace,
                    frame,
                    target,
                    outcome
                );
                if !matches!(
                    outcome,
                    macos::ApplyOutcome::Applied | macos::ApplyOutcome::Gone
                ) {
                    *failed += 1;
                }
            }
        }
    }

    /// Where a recovery frame is restored: itself while it shows on a connected
    /// display, otherwise moved onto the main display (its own is gone).
    pub(super) fn restore_target(&self, frame: crate::Rect) -> crate::Rect {
        let Some(outputs) = &self.displays.known else {
            return frame;
        };
        let displays: Vec<_> = outputs.frames().collect();
        crate::geometry::onto_displays(frame, &displays, outputs.main().frame)
    }
}

/// Move `frame` (keeping its size) so its top-left sits inside `output`.
fn clamp_into_output(frame: crate::Rect, output: crate::Rect) -> crate::Rect {
    crate::Rect {
        x: frame
            .x
            .clamp(output.x, output.x + (output.width - frame.width).max(0)),
        y: frame
            .y
            .clamp(output.y, output.y + (output.height - frame.height).max(0)),
        ..frame
    }
}
