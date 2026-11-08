//! UI-thread icon cache and bounded, identity-checked updates to the displayed grid.

#[cfg(windows)]
mod implementation {
    #[cfg(test)]
    use super::super::GridItem;
    use super::super::MainWindow;
    use quicknest::infrastructure::windows::icons::{
        IconPixels, IconResult, IconService, RequestStatus,
    };
    use slint::Model;
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::mpsc::TryRecvError;
    use std::time::{Duration, Instant};

    const RESULT_BUDGET: usize = 8;
    const REQUEST_BUDGET: usize = 8;
    const REQUEST_SCAN_BUDGET: usize = 64;
    const ROW_BUDGET: usize = 64;
    const DEFAULT_FAILURE_COOLDOWN: Duration = Duration::from_secs(60);
    // Audit B3: bounded success cache. Entries beyond these caps are evicted
    // oldest-first; an evicted source simply becomes a cache miss and is
    // re-requested through the normal bounded queue.
    const DEFAULT_MAX_CACHE_ENTRIES: usize = 256;
    const DEFAULT_MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;
    const DEFAULT_REVALIDATE_INTERVAL: Duration = Duration::from_secs(5);

    trait Queue {
        fn request(&self, source: &str) -> RequestStatus;
        fn poll(&self) -> Result<IconResult, TryRecvError>;
        fn advance_generation(&self) -> u64;
    }

    impl Queue for IconService {
        fn request(&self, source: &str) -> RequestStatus {
            self.request(source, source)
        }

        fn poll(&self) -> Result<IconResult, TryRecvError> {
            self.poll()
        }

        fn advance_generation(&self) -> u64 {
            self.advance_generation()
        }
    }

    #[derive(Default)]
    struct VisibleRows {
        // Row indices belong to the displayed model, never to controller.item_rows().
        rows: Vec<(String, Option<String>)>,
        by_source: HashMap<String, Vec<usize>>,
        sources: Vec<String>,
        revision: u64,
    }

    struct State {
        cache: HashMap<String, slint::Image>,
        /// Audit B3: insertion order for oldest-first eviction, plus the
        /// running pixel-byte total of cached images.
        lru: VecDeque<String>,
        cached_bytes: u64,
        max_cache_entries: usize,
        max_cache_bytes: u64,
        /// Audit B3: last revalidation per source; one cached source per tick
        /// older than the interval is re-requested so a replaced file on the
        /// same path eventually refreshes.
        revalidated: HashMap<String, Instant>,
        cached_at: HashMap<String, Instant>,
        revalidate_interval: Duration,
        failures: HashMap<String, Instant>,
        invalidated_at: HashMap<String, u64>,
        pending: HashMap<String, u64>,
        generation: u64,
        disconnected: bool,
        failure_cooldown: Duration,
        visible: VisibleRows,
        requests: VecDeque<String>,
        dirty: VecDeque<String>,
        dirty_cursors: HashMap<String, usize>,
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                cache: HashMap::new(),
                revalidated: HashMap::new(),
                cached_at: HashMap::new(),
                revalidate_interval: DEFAULT_REVALIDATE_INTERVAL,
                max_cache_entries: DEFAULT_MAX_CACHE_ENTRIES,
                max_cache_bytes: DEFAULT_MAX_CACHE_BYTES,
                cached_bytes: 0,
                lru: VecDeque::new(),
                failures: HashMap::new(),
                invalidated_at: HashMap::new(),
                pending: HashMap::new(),
                generation: 0,
                disconnected: false,
                failure_cooldown: DEFAULT_FAILURE_COOLDOWN,
                visible: VisibleRows::default(),
                requests: VecDeque::new(),
                dirty: VecDeque::new(),
                dirty_cursors: HashMap::new(),
            }
        }
    }

    impl State {
        fn pixel_bytes(image: &slint::Image) -> u64 {
            let size = image.size();
            u64::from(size.width) * u64::from(size.height) * 4
        }

        /// Audit B3: records a cached image under the bounded LRU and evicts
        /// oldest entries beyond the entry/byte caps. Grid rows keep their
        /// cloned pixels; eviction only frees the cache side, and an evicted
        /// source becomes a normal miss that the bounded queue re-requests.
        fn remember_cached(&mut self, source: &str, image: &slint::Image) {
            let new_bytes = Self::pixel_bytes(image);
            if let Some(old) = self.cache.insert(source.to_owned(), image.clone()) {
                self.cached_bytes = self.cached_bytes.saturating_sub(Self::pixel_bytes(&old));
                if let Some(position) = self.lru.iter().position(|existing| existing == source) {
                    self.lru.remove(position);
                }
            }
            self.cached_bytes += new_bytes;
            self.cached_at.insert(source.to_owned(), Instant::now());
            self.lru.push_back(source.to_owned());
            while self.lru.len() > self.max_cache_entries
                || (self.cached_bytes > self.max_cache_bytes && self.lru.len() > 1)
            {
                let Some(evicted) = self.lru.pop_front() else {
                    break;
                };
                if let Some(removed) = self.cache.remove(&evicted) {
                    self.cached_bytes = self
                        .cached_bytes
                        .saturating_sub(Self::pixel_bytes(&removed));
                }
                self.revalidated.remove(&evicted);
                self.cached_at.remove(&evicted);
            }
        }

        /// Audit B3: the oldest cached source whose file may have changed and
        /// that has no in-flight work. The lru order bounds the scan.
        fn oldest_revalidatable(&self) -> Option<String> {
            self.lru
                .iter()
                .find(|source| {
                    !self.pending.contains_key(*source)
                        && self
                            .cached_at
                            .get(*source)
                            .is_some_and(|cached| cached.elapsed() >= self.revalidate_interval)
                        && self
                            .revalidated
                            .get(*source)
                            .is_none_or(|last| last.elapsed() >= self.revalidate_interval)
                })
                .cloned()
        }

        fn rebuild_requests(&mut self) {
            self.requests = self
                .visible
                .sources
                .iter()
                .filter(|source| !self.cache.contains_key(*source))
                .cloned()
                .collect();
        }

        fn mark_dirty(&mut self, source: &str) {
            // A newer completion must revisit rows already patched with the
            // previous image, even when shared-source backfill spans ticks.
            if self.visible.by_source.contains_key(source)
                && self.dirty_cursors.insert(source.to_owned(), 0).is_none()
            {
                self.dirty.push_back(source.to_owned());
            }
        }
    }

    struct IconContext {
        queue: Box<dyn Queue>,
        state: RefCell<State>,
    }

    thread_local! {
        static ICON_CONTEXT: RefCell<Option<Rc<IconContext>>> = const { RefCell::new(None) };
    }

    fn context() -> Option<Rc<IconContext>> {
        ICON_CONTEXT.with(|slot| slot.borrow().clone())
    }

    /// Installs a new UI-thread cache and a single background STA worker.
    pub(crate) fn install(directory: PathBuf) {
        let context = Rc::new(IconContext {
            queue: Box::new(IconService::start(directory)),
            state: RefCell::new(State::default()),
        });
        ICON_CONTEXT.with(|slot| *slot.borrow_mut() = Some(context));
    }

    /// Pure cache lookup during GridItem construction. Requests are submitted
    /// only after sync_visible_sources describes the model actually on screen.
    pub(crate) fn image_for(source: &str) -> slint::Image {
        context()
            .and_then(|context| context.state.borrow().cache.get(source).cloned())
            .unwrap_or_default()
    }

    /// Call immediately after setting the grid model. Sidebar-only refreshes
    /// must not change this snapshot, including during spring-loaded group changes.
    pub(crate) fn sync_visible_sources(rows: Vec<(String, Option<String>)>) {
        let Some(context) = context() else { return };
        let mut state = context.state.borrow_mut();
        let mut visible = VisibleRows {
            rows,
            revision: state.visible.revision.wrapping_add(1),
            ..VisibleRows::default()
        };
        for (index, (_, source)) in visible.rows.iter().enumerate() {
            if let Some(source) = source.as_ref().filter(|source| !source.trim().is_empty()) {
                if !visible.by_source.contains_key(source) {
                    visible.sources.push(source.clone());
                }
                visible
                    .by_source
                    .entry(source.clone())
                    .or_default()
                    .push(index);
            }
        }
        if state.visible.sources != visible.sources {
            state.generation = context.queue.advance_generation();
            // Cancellation is not an extraction failure. Old native completions
            // may still populate the cache, but cannot clear new pending requests.
            state.pending.clear();
        }
        state.visible = visible;
        state.dirty.clear();
        state.dirty_cursors.clear();
        state.rebuild_requests();
        for source in state.visible.sources.clone() {
            if state.cache.contains_key(&source) {
                state.mark_dirty(&source);
            }
        }
    }

    /// Forget success/failure for a changed source and retry on a later tick.
    /// In-flight results older than this invalidation cannot restore stale data.
    pub(crate) fn invalidate(source: &str) {
        let Some(context) = context() else { return };
        let mut state = context.state.borrow_mut();
        state.generation = context.queue.advance_generation();
        let generation = state.generation;
        state.invalidated_at.insert(source.to_owned(), generation);
        // Audit B3: the invalidation bookkeeping is bounded; oldest stamps
        // beyond the cap can no longer influence any in-flight result.
        if state.invalidated_at.len() > 128 {
            let mut stamps: Vec<(String, u64)> = state
                .invalidated_at
                .iter()
                .map(|(source, generation)| (source.clone(), *generation))
                .collect();
            stamps.sort_by_key(|(_, generation)| *generation);
            while stamps.len() > 64 {
                let (source, _) = stamps.remove(0);
                state.invalidated_at.remove(&source);
            }
        }
        if let Some(removed) = state.cache.remove(source) {
            state.cached_bytes = state
                .cached_bytes
                .saturating_sub(State::pixel_bytes(&removed));
        }
        if let Some(position) = state.lru.iter().position(|existing| existing == source) {
            state.lru.remove(position);
        }
        state.revalidated.remove(source);
        state.cached_at.remove(source);
        state.failures.remove(source);
        state.pending.clear();
        state.rebuild_requests();
    }

    /// Audit B3: forget every cached/failure decision after a whole-library
    /// replacement (overwrite import, restore). The next refresh re-requests
    /// all displayed sources through the bounded queue.
    pub(crate) fn invalidate_all() {
        let Some(context) = context() else { return };
        let mut state = context.state.borrow_mut();
        state.generation = context.queue.advance_generation();
        state.pending.clear();
        state.cache.clear();
        state.lru.clear();
        state.cached_bytes = 0;
        state.failures.clear();
        state.revalidated.clear();
        state.cached_at.clear();
        state.invalidated_at.clear();
        state.rebuild_requests();
    }

    /// Failed sources retry at most once per cooldown (60 seconds by default).
    /// A one-second minimum prevents accidental per-tick retry storms.
    #[allow(dead_code)]
    pub(crate) fn set_failure_cooldown(cooldown: Duration) {
        if let Some(context) = context() {
            context.state.borrow_mut().failure_cooldown = cooldown.max(Duration::from_secs(1));
        }
    }

    fn image_from_pixels(pixels: IconPixels) -> Option<slint::Image> {
        let expected = (pixels.width as usize)
            .checked_mul(pixels.height as usize)?
            .checked_mul(4)?;
        if pixels.width == 0 || pixels.height == 0 || pixels.rgba.len() != expected {
            return None;
        }
        let mut buffer =
            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(pixels.width, pixels.height);
        buffer.make_mut_bytes().copy_from_slice(&pixels.rgba);
        Some(slint::Image::from_rgba8(buffer))
    }

    struct Patch {
        index: usize,
        id: String,
        source: String,
        revision: u64,
        image: slint::Image,
    }

    impl IconContext {
        fn drain_and_submit(&self) {
            let mut state = self.state.borrow_mut();
            for _ in 0..RESULT_BUDGET {
                let result = match self.queue.poll() {
                    Ok(result) => result,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        state.pending.clear();
                        state.disconnected = true;
                        break;
                    }
                };
                if state.pending.get(&result.key) == Some(&result.generation) {
                    state.pending.remove(&result.key);
                }
                if state
                    .invalidated_at
                    .get(&result.key)
                    .is_some_and(|minimum| result.generation < *minimum)
                {
                    continue;
                }
                match result.pixels.and_then(image_from_pixels) {
                    Some(image) => {
                        state.failures.remove(&result.key);
                        // Audit B3: identical pixels (a revalidation of an
                        // unchanged file) must not emit row notifications.
                        let unchanged = state
                            .cache
                            .get(&result.key)
                            .is_some_and(|existing| same_image(existing, &image));
                        state.remember_cached(&result.key, &image);
                        if !unchanged {
                            state.mark_dirty(&result.key);
                        }
                    }
                    None => {
                        if !state.cache.contains_key(&result.key) {
                            state.failures.insert(result.key.clone(), Instant::now());
                            state.mark_dirty(&result.key);
                        }
                    }
                }
            }
            if state.disconnected {
                return;
            }
            // Audit B3: budgeted revalidation — at most one cached source per
            // tick is re-requested so an externally replaced file eventually
            // refreshes without the UI thread ever touching the file system.
            let mut submitted = 0;
            if submitted < REQUEST_BUDGET
                && let Some(source) = state.oldest_revalidatable()
                && let RequestStatus::Accepted = self.queue.request(&source)
            {
                let generation = state.generation;
                state.pending.insert(source.clone(), generation);
                state.revalidated.insert(source.clone(), Instant::now());
                submitted += 1;
            }
            for _ in 0..state.requests.len().min(REQUEST_SCAN_BUDGET) {
                if submitted == REQUEST_BUDGET {
                    break;
                }
                let Some(source) = state.requests.pop_front() else {
                    break;
                };
                if state.cache.contains_key(&source) {
                    continue;
                }
                let cooling_down = state
                    .failures
                    .get(&source)
                    .is_some_and(|failed| failed.elapsed() < state.failure_cooldown);
                if !state.pending.contains_key(&source) && !cooling_down {
                    match self.queue.request(&source) {
                        RequestStatus::Accepted => {
                            let generation = state.generation;
                            state.pending.insert(source.clone(), generation);
                            submitted += 1;
                        }
                        RequestStatus::Full => {
                            state.requests.push_front(source);
                            break;
                        }
                        RequestStatus::Disconnected => {
                            state.pending.clear();
                            state.disconnected = true;
                            state.requests.push_front(source);
                            break;
                        }
                    }
                }
                state.requests.push_back(source);
            }
        }

        fn patches(&self) -> Vec<Patch> {
            let mut state = self.state.borrow_mut();
            let mut patches = Vec::new();
            // Count visited targets as well as successful updates, so stale
            // rows and already-cached icons also obey the per-tick budget.
            for _ in 0..ROW_BUDGET {
                let Some(source) = state.dirty.pop_front() else {
                    break;
                };
                let cursor = state
                    .dirty_cursors
                    .get(&source)
                    .copied()
                    .unwrap_or_default();
                let rows = state.visible.by_source.get(&source);
                let index = rows.and_then(|rows| rows.get(cursor)).copied();
                let more = rows.is_some_and(|rows| cursor + 1 < rows.len());
                if more {
                    state.dirty_cursors.insert(source.clone(), cursor + 1);
                    state.dirty.push_back(source.clone());
                } else {
                    state.dirty_cursors.remove(&source);
                }
                let image = state.cache.get(&source).cloned().or_else(|| {
                    state
                        .failures
                        .contains_key(&source)
                        .then(slint::Image::default)
                });
                if let Some(index) = index
                    && let Some(image) = image
                {
                    patches.push(Patch {
                        index,
                        id: state.visible.rows[index].0.clone(),
                        source,
                        revision: state.visible.revision,
                        image,
                    });
                }
            }
            patches
        }
    }

    fn same_image(left: &slint::Image, right: &slint::Image) -> bool {
        // Slint Image PartialEq is not content equality (an empty Image is
        // not even reflexively equal). Compare decoded pixel bytes so an
        // unchanged revalidation stays silent.
        if left.size() != right.size() {
            return false;
        }
        let empty = |image: &slint::Image| image.size().width == 0 && image.size().height == 0;
        if empty(left) && empty(right) {
            return true;
        }
        match (left.to_rgba8(), right.to_rgba8()) {
            (Some(left_buffer), Some(right_buffer)) => {
                left_buffer.as_bytes() == right_buffer.as_bytes()
            }
            _ => false,
        }
    }

    /// Drains and submits bounded work, then updates only matching displayed
    /// rows with Model::set_row_data. Returns the number of changed row icons.
    pub(crate) fn tick(ui: &MainWindow) -> usize {
        let Some(context) = context() else { return 0 };
        context.drain_and_submit();
        let model = ui.get_items();
        let mut changed = 0;
        for patch in context.patches() {
            let matches_snapshot = {
                let state = context.state.borrow();
                state.visible.revision == patch.revision
                    && state
                        .visible
                        .rows
                        .get(patch.index)
                        .is_some_and(|(id, source)| {
                            id == &patch.id && source.as_deref() == Some(patch.source.as_str())
                        })
            };
            if !matches_snapshot {
                continue;
            }
            if let Some(mut row) = model.row_data(patch.index)
                && row.id.as_str() == patch.id
                && !same_image(&row.icon, &patch.image)
            {
                row.icon = patch.image;
                model.set_row_data(patch.index, row);
                changed += 1;
            }
        }
        changed
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::cell::Cell;
        use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

        struct ControlledQueue {
            requests: SyncSender<(String, u64)>,
            results: Receiver<IconResult>,
            generation: Cell<u64>,
        }

        impl Queue for ControlledQueue {
            fn request(&self, source: &str) -> RequestStatus {
                match self
                    .requests
                    .try_send((source.to_owned(), self.generation.get()))
                {
                    Ok(()) => RequestStatus::Accepted,
                    Err(TrySendError::Full(_)) => RequestStatus::Full,
                    Err(TrySendError::Disconnected(_)) => RequestStatus::Disconnected,
                }
            }

            fn poll(&self) -> Result<IconResult, TryRecvError> {
                self.results.try_recv()
            }

            fn advance_generation(&self) -> u64 {
                let generation = self.generation.get() + 1;
                self.generation.set(generation);
                generation
            }
        }

        struct Fixture {
            ui: MainWindow,
            requests: Receiver<(String, u64)>,
            results: SyncSender<IconResult>,
        }

        fn fixture(capacity: usize) -> Fixture {
            i_slint_backend_testing::init_no_event_loop();
            let (request_tx, request_rx) = sync_channel(capacity);
            let (result_tx, result_rx) = sync_channel(32);
            let context = Rc::new(IconContext {
                queue: Box::new(ControlledQueue {
                    requests: request_tx,
                    results: result_rx,
                    generation: Cell::new(0),
                }),
                state: RefCell::new(State::default()),
            });
            ICON_CONTEXT.with(|slot| *slot.borrow_mut() = Some(context));
            Fixture {
                ui: MainWindow::new().unwrap(),
                requests: request_rx,
                results: result_tx,
            }
        }

        fn show_rows(ui: &MainWindow, rows: &[(&str, &str)]) {
            let items: Vec<_> = rows
                .iter()
                .map(|(id, source)| GridItem {
                    id: (*id).into(),
                    name: format!("name-{id}").into(),
                    path: format!("path-{id}").into(),
                    note: format!("note-{id}").into(),
                    item_type: "fixture".into(),
                    path_missing: true,
                    missing_reason: "fixture missing".into(),
                    artwork: "terminal".into(),
                    batch_selected: true,
                    favorite: true,
                    launch_count: 7,
                    last_used: "12345".into(),
                    icon: image_for(source),
                })
                .collect();
            ui.set_items(slint::ModelRc::new(slint::VecModel::from(items)));
            sync_visible_sources(
                rows.iter()
                    .map(|(id, source)| ((*id).to_owned(), Some((*source).to_owned())))
                    .collect(),
            );
        }

        fn pixels() -> IconPixels {
            IconPixels {
                width: 2,
                height: 1,
                rgba: vec![255, 0, 0, 255, 0, 255, 0, 128],
            }
        }

        fn assert_empty_image(image: slint::Image) {
            assert_eq!(image.size().width, 0);
            assert_eq!(image.size().height, 0);
        }

        fn assert_same_row(mut actual: GridItem, mut expected: GridItem) {
            assert_eq!(actual.icon.size(), expected.icon.size());
            assert_eq!(
                actual
                    .icon
                    .to_rgba8()
                    .map(|buffer| buffer.as_bytes().to_vec()),
                expected
                    .icon
                    .to_rgba8()
                    .map(|buffer| buffer.as_bytes().to_vec()),
            );
            // Slint's empty Image is unequal even to its clone. Compare every
            // other GridItem field with one shared, nonempty image instead.
            let comparable = image_from_pixels(pixels()).unwrap();
            actual.icon = comparable.clone();
            expected.icon = comparable;
            assert_eq!(actual, expected);
        }

        fn complete(fixture: &Fixture, request: (String, u64), pixels: Option<IconPixels>) {
            fixture
                .results
                .try_send(IconResult {
                    key: request.0,
                    generation: request.1,
                    pixels,
                })
                .unwrap();
        }

        #[test]
        fn tick_updates_shared_source_rows_in_place_and_preserves_all_other_state() {
            let fixture = fixture(32);
            let ui = &fixture.ui;
            show_rows(
                ui,
                &[
                    ("first", "shared"),
                    ("second", "shared"),
                    ("other", "other"),
                ],
            );
            ui.set_active_item_id("second".into());
            ui.set_batch_count(3);
            ui.set_dragging_item_id("first".into());
            ui.set_item_drag_active(true);
            let group_scroll = ui.get_group_scroll_offset();
            let category_scroll = ui.get_category_scroll_offset();
            let original_model = ui.get_items();
            let before: Vec<_> = original_model.iter().collect();
            assert_eq!(tick(ui), 0);
            let request = fixture.requests.try_recv().unwrap();
            assert_eq!(request.0, "shared");
            complete(&fixture, request, Some(pixels()));
            assert_eq!(tick(ui), 2);
            assert!(
                ui.get_items() == original_model,
                "must retain the model object"
            );
            let image = image_for("shared");
            let buffer = image.to_rgba8().unwrap();
            assert_eq!(buffer.as_bytes(), pixels().rgba);
            for (index, mut expected) in before.into_iter().enumerate() {
                if index < 2 {
                    expected.icon = image.clone();
                }
                assert_same_row(original_model.row_data(index).unwrap(), expected);
            }
            assert_eq!(ui.get_active_item_id(), "second");
            assert_eq!(ui.get_batch_count(), 3);
            assert_eq!(ui.get_dragging_item_id(), "first");
            assert!(ui.get_item_drag_active());
            assert_eq!(ui.get_group_scroll_offset(), group_scroll);
            assert_eq!(ui.get_category_scroll_offset(), category_scroll);
            assert_eq!(tick(ui), 0, "unchanged results must not notify rows again");
        }

        #[test]
        fn full_queue_marks_only_accepted_work_pending_and_later_ticks_refill_it() {
            let fixture = fixture(1);
            show_rows(&fixture.ui, &[("a", "a"), ("b", "b"), ("c", "c")]);
            assert_eq!(tick(&fixture.ui), 0);
            let context = context().unwrap();
            assert_eq!(context.state.borrow().pending.len(), 1);
            assert!(context.state.borrow().pending.contains_key("a"));
            let first = fixture.requests.try_recv().unwrap();
            complete(&fixture, first, Some(pixels()));
            assert_eq!(tick(&fixture.ui), 1);
            let second = fixture.requests.try_recv().unwrap();
            assert_eq!(second.0, "b");
            assert_eq!(context.state.borrow().pending.len(), 1);
            assert!(!context.state.borrow().pending.contains_key("c"));
            complete(&fixture, second, Some(pixels()));
            assert_eq!(tick(&fixture.ui), 1);
            let third = fixture.requests.try_recv().unwrap();
            assert_eq!(third.0, "c");
            complete(&fixture, third, Some(pixels()));
            assert_eq!(tick(&fixture.ui), 1);
            assert!(context.state.borrow().pending.is_empty());
            assert!(fixture.requests.try_recv().is_err());
        }

        #[test]
        fn each_tick_has_request_result_and_row_budgets() {
            let fixture = fixture(128);
            let names: Vec<_> = (0..REQUEST_BUDGET * 2)
                .map(|index| format!("source-{index}"))
                .collect();
            let rows: Vec<_> = names
                .iter()
                .map(|source| (source.as_str(), source.as_str()))
                .collect();
            show_rows(&fixture.ui, &rows);
            assert_eq!(tick(&fixture.ui), 0);
            let mut accepted: Vec<_> = fixture.requests.try_iter().collect();
            assert_eq!(accepted.len(), REQUEST_BUDGET);
            assert_eq!(tick(&fixture.ui), 0);
            accepted.extend(fixture.requests.try_iter());
            assert_eq!(accepted.len(), REQUEST_BUDGET * 2);
            for request in accepted {
                complete(&fixture, request, Some(pixels()));
            }
            assert_eq!(tick(&fixture.ui), RESULT_BUDGET);
            assert_eq!(context().unwrap().state.borrow().cache.len(), RESULT_BUDGET);
            assert_eq!(tick(&fixture.ui), RESULT_BUDGET);

            let names: Vec<_> = (0..ROW_BUDGET + 5)
                .map(|index| format!("row-{index}"))
                .collect();
            let rows: Vec<_> = names.iter().map(|id| (id.as_str(), "shared")).collect();
            show_rows(&fixture.ui, &rows);
            assert_eq!(tick(&fixture.ui), 0);
            complete(
                &fixture,
                fixture.requests.try_recv().unwrap(),
                Some(pixels()),
            );
            assert_eq!(tick(&fixture.ui), ROW_BUDGET);
            assert_eq!(
                tick(&fixture.ui),
                5,
                "shared-source updates continue next tick"
            );
            assert_eq!(tick(&fixture.ui), 0);
        }

        #[test]
        fn results_for_removed_or_switched_sources_only_populate_cache() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("deleted", "old-a"), ("switched", "old-b")]);
            tick(&fixture.ui);
            let old: Vec<_> = fixture.requests.try_iter().collect();
            show_rows(&fixture.ui, &[("new", "current")]);
            assert!(context().unwrap().state.borrow().pending.is_empty());
            assert!(context().unwrap().state.borrow().failures.is_empty());
            for request in old {
                complete(&fixture, request, Some(pixels()));
            }
            assert_eq!(tick(&fixture.ui), 0);
            let row = fixture.ui.get_items().row_data(0).unwrap();
            assert_eq!(row.id, "new");
            assert_empty_image(row.icon);
            assert_eq!(image_for("old-a").size().width, 2);
            assert_eq!(image_for("old-b").size().width, 2);
            assert_eq!(fixture.requests.try_recv().unwrap().0, "current");
        }

        #[test]
        fn row_identity_check_prevents_backfill_into_a_reordered_unsynced_model() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("a", "source-a"), ("b", "source-b")]);
            tick(&fixture.ui);
            let requests: Vec<_> = fixture.requests.try_iter().collect();
            let original = fixture.ui.get_items();
            let swapped = vec![original.row_data(1).unwrap(), original.row_data(0).unwrap()];
            fixture
                .ui
                .set_items(slint::ModelRc::new(slint::VecModel::from(swapped)));
            for request in requests {
                complete(&fixture, request, Some(pixels()));
            }
            assert_eq!(
                tick(&fixture.ui),
                0,
                "stale positions must never update different IDs"
            );
            assert!(
                fixture
                    .ui
                    .get_items()
                    .iter()
                    .all(|row| row.icon.size().width == 0 && row.icon.size().height == 0)
            );
            sync_visible_sources(vec![
                ("b".into(), Some("source-b".into())),
                ("a".into(), Some("source-a".into())),
            ]);
            assert_eq!(
                tick(&fixture.ui),
                2,
                "fresh displayed mapping can use the cached results"
            );
        }

        #[test]
        fn changed_source_on_same_row_id_cannot_receive_old_icon() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("same-id", "before")]);
            tick(&fixture.ui);
            let old = fixture.requests.try_recv().unwrap();
            show_rows(&fixture.ui, &[("same-id", "after")]);
            complete(&fixture, old, Some(pixels()));
            assert_eq!(tick(&fixture.ui), 0);
            assert_empty_image(fixture.ui.get_items().row_data(0).unwrap().icon);
            assert_eq!(fixture.requests.try_recv().unwrap().0, "after");
        }

        #[test]
        fn older_native_completion_cannot_clear_new_generation_pending_for_same_source() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("old", "shared")]);
            tick(&fixture.ui);
            let old = fixture.requests.try_recv().unwrap();
            show_rows(&fixture.ui, &[("new", "shared"), ("extra", "extra")]);
            tick(&fixture.ui);
            let new = fixture.requests.try_recv().unwrap();
            assert_eq!(new.0, "shared");
            assert!(new.1 > old.1);
            complete(&fixture, old, Some(pixels()));
            assert_eq!(tick(&fixture.ui), 1);
            assert_eq!(
                context().unwrap().state.borrow().pending.get("shared"),
                Some(&new.1)
            );
        }

        #[test]
        fn newer_shared_source_completion_revisits_rows_already_patched_in_an_earlier_tick() {
            let fixture = fixture(32);
            let names: Vec<_> = (0..ROW_BUDGET + 2)
                .map(|index| format!("row-{index}"))
                .collect();
            let mut rows: Vec<_> = names.iter().map(|id| (id.as_str(), "shared")).collect();
            show_rows(&fixture.ui, &rows);
            tick(&fixture.ui);
            let old = fixture.requests.try_recv().unwrap();
            rows.push(("extra", "extra"));
            show_rows(&fixture.ui, &rows);
            tick(&fixture.ui);
            let current = fixture.requests.try_recv().unwrap();
            assert_eq!(current.0, "shared");
            complete(&fixture, old, Some(pixels()));
            assert_eq!(tick(&fixture.ui), ROW_BUDGET);
            let mut newest = pixels();
            newest.rgba[0] = 37;
            complete(&fixture, current, Some(newest.clone()));
            assert_eq!(tick(&fixture.ui), ROW_BUDGET);
            assert_eq!(tick(&fixture.ui), 2);
            for row in fixture.ui.get_items().iter().take(names.len()) {
                assert_eq!(row.icon.to_rgba8().unwrap().as_bytes(), newest.rgba);
            }
        }

        #[test]
        fn failures_are_cached_without_row_updates_and_retry_after_cooldown_or_invalidation() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("row", "failing")]);
            tick(&fixture.ui);
            let request = fixture.requests.try_recv().unwrap();
            complete(&fixture, request, None);
            for _ in 0..10 {
                assert_eq!(tick(&fixture.ui), 0);
            }
            assert!(
                fixture.requests.try_recv().is_err(),
                "failure must not cause a retry storm"
            );
            set_failure_cooldown(Duration::ZERO);
            let context = context().unwrap();
            assert_eq!(
                context.state.borrow().failure_cooldown,
                Duration::from_secs(1)
            );
            context
                .state
                .borrow_mut()
                .failures
                .insert("failing".into(), Instant::now() - Duration::from_secs(2));
            assert_eq!(tick(&fixture.ui), 0);
            let retry = fixture.requests.try_recv().unwrap();
            invalidate("failing");
            complete(&fixture, retry, Some(pixels()));
            assert_eq!(
                tick(&fixture.ui),
                0,
                "invalidated in-flight data must be ignored"
            );
            assert_empty_image(image_for("failing"));
            let current = fixture.requests.try_recv().unwrap();
            complete(&fixture, current, Some(pixels()));
            assert_eq!(tick(&fixture.ui), 1);
            assert!(!context.state.borrow().failures.contains_key("failing"));
        }

        #[test]
        fn disconnected_result_channel_clears_pending_without_marking_failure() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("row", "source")]);
            tick(&fixture.ui);
            let Fixture {
                ui,
                requests,
                results,
            } = fixture;
            assert!(requests.try_recv().is_ok());
            drop(results);
            assert_eq!(tick(&ui), 0);
            let context = context().unwrap();
            assert!(context.state.borrow().pending.is_empty());
            assert!(context.state.borrow().failures.is_empty());
            assert!(context.state.borrow().disconnected);
            assert!(requests.try_recv().is_err());
        }

        #[test]
        fn disconnected_request_channel_never_creates_pending_work() {
            let Fixture {
                ui,
                requests,
                results: _results,
            } = fixture(32);
            show_rows(&ui, &[("row", "source")]);
            drop(requests);
            assert_eq!(tick(&ui), 0);
            let context = context().unwrap();
            assert!(context.state.borrow().pending.is_empty());
            assert!(context.state.borrow().failures.is_empty());
            assert!(context.state.borrow().disconnected);
        }

        #[test]
        fn failed_retry_reverts_a_previous_success_to_default_exactly_once() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("row", "source")]);
            tick(&fixture.ui);
            complete(
                &fixture,
                fixture.requests.try_recv().unwrap(),
                Some(pixels()),
            );
            assert_eq!(tick(&fixture.ui), 1);
            let model = fixture.ui.get_items();
            let mut expected = model.row_data(0).unwrap();
            assert_eq!(expected.icon.size().width, 2);
            invalidate("source");
            assert_eq!(tick(&fixture.ui), 0);
            complete(&fixture, fixture.requests.try_recv().unwrap(), None);
            assert_eq!(tick(&fixture.ui), 1);
            expected.icon = slint::Image::default();
            assert_same_row(model.row_data(0).unwrap(), expected);
            assert_empty_image(model.row_data(0).unwrap().icon);
            assert!(fixture.ui.get_items() == model);
            for _ in 0..3 {
                assert_eq!(tick(&fixture.ui), 0);
            }
            assert!(fixture.requests.try_recv().is_err());
        }

        #[test]
        fn malformed_rgba_keeps_existing_row_and_enters_failure_cooldown() {
            let fixture = fixture(32);
            show_rows(&fixture.ui, &[("row", "source")]);
            tick(&fixture.ui);
            let before = fixture.ui.get_items().row_data(0).unwrap();
            let invalid = IconPixels {
                width: 2,
                height: 2,
                rgba: vec![0; 3],
            };
            complete(
                &fixture,
                fixture.requests.try_recv().unwrap(),
                Some(invalid),
            );
            assert_eq!(tick(&fixture.ui), 0);
            assert_same_row(fixture.ui.get_items().row_data(0).unwrap(), before);
            assert!(
                context()
                    .unwrap()
                    .state
                    .borrow()
                    .failures
                    .contains_key("source")
            );
        }

        fn different_pixels() -> IconPixels {
            IconPixels {
                width: 2,
                height: 1,
                rgba: vec![0, 0, 255, 255, 255, 255, 0, 64],
            }
        }

        /// Audit B3: the success cache evicts oldest-first beyond the entry
        /// cap and tracks its pixel-byte total. A source evicted from the
        /// cache is a plain miss: the bounded queue re-requests it, which the
        /// revalidation/queue tests cover.
        #[test]
        fn success_cache_is_lru_bounded_by_entries_and_bytes() {
            let _fixture = fixture(32);
            let context = context().unwrap();
            let image = image_from_pixels(pixels()).unwrap();
            {
                let mut state = context.state.borrow_mut();
                state.max_cache_entries = 2;
                state.max_cache_bytes = u64::MAX;
                state.remember_cached("a", &image);
                state.remember_cached("b", &image);
                assert_eq!(state.lru.as_slices().0, &["a".to_string(), "b".to_string()]);
                // Inserting "c" beyond the cap evicts "a" oldest-first.
                state.remember_cached("c", &image);
                assert!(!state.cache.contains_key("a"));
                assert!(state.cache.contains_key("b") && state.cache.contains_key("c"));
                assert_eq!(state.cached_bytes, 16, "two 2x1 RGBA images = 16 bytes");
                // Touching "b" refreshes its order; inserting "d" then evicts
                // "c", the least recent.
                state.remember_cached("b", &image);
                assert_eq!(state.cached_bytes, 16, "overwrite adjusts, never doubles");
                state.remember_cached("d", &image);
                assert!(!state.cache.contains_key("c"));
                assert!(state.cache.contains_key("b") && state.cache.contains_key("d"));
                assert_eq!(state.lru.len(), 2);
                // The byte cap alone also evicts: one 64x64 image (16 KiB)
                // exceeds a 4 KiB budget.
                state.max_cache_bytes = 4096;
                state.remember_cached("big", &slint::Image::default());
                assert!(
                    state.cache.contains_key("big"),
                    "the newest entry always stays"
                );
                assert!(state.cache.len() <= 2, "byte cap evicted older entries");
            }
        }

        /// Audit B3: budgeted revalidation re-requests one cached source per
        /// tick; a changed file replaces the row image, an unchanged file
        /// stays silent.
        #[test]
        fn revalidation_refreshes_changed_files_without_churn() {
            let fixture = fixture(32);
            let context = context().unwrap();
            context.state.borrow_mut().revalidate_interval = Duration::ZERO;
            show_rows(&fixture.ui, &[("row", "source")]);
            assert_eq!(tick(&fixture.ui), 0);
            let first = fixture.requests.try_recv().unwrap();
            complete(&fixture, first.clone(), Some(pixels()));
            assert_eq!(tick(&fixture.ui), 1);

            // The next tick revalidates: an identical extraction keeps the
            // row silent.
            assert_eq!(tick(&fixture.ui), 0, "identical pixels must not patch");
            let revalidated = fixture.requests.try_recv().unwrap();
            assert_eq!(revalidated.0, "source");
            complete(&fixture, revalidated, Some(pixels()));
            assert_eq!(tick(&fixture.ui), 0, "no row notification for same image");

            // A changed file replaces the displayed image.
            assert_eq!(tick(&fixture.ui), 0);
            let changed = fixture.requests.try_recv().unwrap();
            complete(&fixture, changed, Some(different_pixels()));
            assert_eq!(tick(&fixture.ui), 1);
            let row = fixture.ui.get_items().row_data(0).unwrap();
            let buffer = row.icon.to_rgba8().unwrap();
            assert_eq!(buffer.as_bytes(), different_pixels().rgba);
        }
    }
}

#[cfg(windows)]
pub(super) use implementation::invalidate_all;
#[cfg(windows)]
pub(super) use implementation::{image_for, install, sync_visible_sources, tick};
#[cfg(windows)]
#[allow(unused_imports)]
pub(super) use implementation::{invalidate, set_failure_cooldown};

#[cfg(not(windows))]
pub(super) fn install(_directory: std::path::PathBuf) {}
#[cfg(not(windows))]
pub(super) fn image_for(_source: &str) -> slint::Image {
    slint::Image::default()
}
#[cfg(not(windows))]
pub(super) fn sync_visible_sources(_rows: Vec<(String, Option<String>)>) {}
#[cfg(not(windows))]
pub(super) fn tick(_ui: &super::MainWindow) -> usize {
    0
}
#[cfg(not(windows))]
#[allow(dead_code)]
pub(super) fn invalidate(_source: &str) {}
#[cfg(not(windows))]
#[allow(dead_code)]
pub(super) fn invalidate_all() {}
#[cfg(not(windows))]
#[allow(dead_code)]
pub(super) fn set_failure_cooldown(_cooldown: std::time::Duration) {}
