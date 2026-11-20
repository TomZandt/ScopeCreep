//! Copies of a display (#208, ADR 0039), driven at the Feed seam: synthetic frames through the Feed loop, the
//! Panels drawn and read back. Two clean pictures keep separate settings and draw different pictures; a duplicate
//! starts from a copy of the original's settings; the numbers are stable; closing forgets settings; a picture zoom
//! and Link reach every picture instance. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::DisplaySet;
use scopecreep::feed_loop::FeedLoop;
use scopecreep::layout_tree::{Dir, LayoutTree};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::page_edit::{self, Room};
use scopecreep::settings::PanelSettings;
use std::sync::{Arc, Mutex};

const CLEAN: &str = "Channel Viewer";
const AREA: [f32; 4] = [0.0, 0.0, 200.0, 120.0];
const ROOM: Room = Room {
    area: [0.0, 0.0, 1256.0, 716.0],
    gap: 12.0,
    min: [120.0, 105.0],
};

struct Bench {
    rig: Rig,
    feed: FeedLoop,
    set: DisplaySet,
    settings: Arc<Mutex<PanelSettings>>,
    tree: LayoutTree,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let set = DisplaySet::standard(&rig.device, &rig.queue, TARGET, false);
        let settings = Arc::new(Mutex::new(PanelSettings::default()));
        let feed = FeedLoop::new(
            set.clone(),
            settings.clone(),
            OutputConversion::new(&rig.device, &rig.queue),
        );
        let names: Vec<String> = set.keys();
        let tree = LayoutTree::default_for(&names.iter().map(String::as_str).collect::<Vec<_>>());
        Bench {
            rig,
            feed,
            set,
            settings,
            tree,
        }
    }

    fn index(&self, key: &str) -> usize {
        self.set.keys().iter().position(|k| k == key).unwrap()
    }

    /// A frame of three colours side by side, and a moment for the Feed loop to take it.
    fn play(&mut self) {
        let (w, h) = (48u32, 27u32);
        let pixels: Vec<f32> = (0..h)
            .flat_map(|_| {
                (0..w).flat_map(move |x| match x * 3 / w {
                    0 => [0.9, 0.1, 0.1, 1.0],
                    1 => [0.1, 0.8, 0.1, 1.0],
                    _ => [0.1, 0.1, 0.9, 1.0],
                })
            })
            .collect();
        let Bench { rig, feed, .. } = self;
        rig.play(w, h, &pixels, |r| {
            r.step(feed);
        });
    }

    fn set_choice(&self, key: &str, setting: &str, value: &str) {
        self.settings.lock().unwrap().set(key, setting, value);
    }

    /// Let the Feed loop see a change of settings or Panels, and measure the frame on screen again.
    fn settle(&mut self) {
        let Bench { rig, feed, .. } = self;
        rig.wait(20, |r| {
            r.step(feed);
        });
    }

    fn draw(&self, index: usize) -> Vec<u8> {
        self.rig.render(200, 120, |pass| {
            self.set.draw(pass, index, AREA, [200.0, 120.0], 1.0);
        })
    }

    /// Close a Panel, then sweep as the Window does.
    fn close(&mut self, panel: usize) -> Vec<usize> {
        let mut settings = self.settings.lock().unwrap();
        page_edit::close(&self.set, &mut self.tree, &mut settings, panel, |_| false);
        page_edit::sweep(&self.set, &mut self.tree, &mut settings)
    }

    fn duplicate(&mut self, key: &str, dir: Dir) -> usize {
        let from = self.index(key);
        let mut settings = self.settings.lock().unwrap();
        page_edit::duplicate(&self.set, &mut self.tree, &mut settings, from, dir, ROOM).unwrap()
    }
}

fn mean(image: &[u8]) -> [f32; 3] {
    let n = (image.len() / 4) as f32;
    let mut sum = [0.0f32; 3];
    for p in image.chunks(4) {
        for c in 0..3 {
            sum[c] += p[c] as f32;
        }
    }
    sum.map(|s| s / n)
}

#[test]
fn two_clean_pictures_keep_separate_settings_and_draw_different_pictures() {
    let mut b = Bench::new("instances-two-clean");
    b.play();
    let second = b.duplicate(CLEAN, Dir::Row);
    assert_eq!(b.set.key(second), "Channel Viewer 2");
    b.set_choice("Channel Viewer", "channel", "red");
    b.set_choice("Channel Viewer 2", "channel", "blue");
    b.settle();
    let (first, second) = (b.index(CLEAN), b.index("Channel Viewer 2"));
    // The picture is a red, a green and a blue third. Drawn as the red channel the left third is bright and the
    // right dark; as the blue channel it is the other way round.
    let (red, blue) = (b.draw(first), b.draw(second));
    let at = |image: &[u8], x: usize| image[(60 * 200 + x) * 4] as i32;
    let (left, right) = (35, 165);
    assert!(at(&red, left) > at(&red, right) + 60, "red channel");
    assert!(at(&blue, right) > at(&blue, left) + 60, "blue channel");
}

#[test]
fn a_duplicate_starts_with_a_copy_of_the_originals_settings_and_state() {
    let mut b = Bench::new("instances-copy");
    b.play();
    b.set_choice(CLEAN, "channel", "green");
    b.settings
        .lock()
        .unwrap()
        .set_state(CLEAN, "loupe_zoom", "300");
    b.settle();
    let copy = b.duplicate(CLEAN, Dir::Col);
    b.settle();
    let key = b.set.key(copy);
    let settings = b.settings.lock().unwrap().clone();
    let original = b.set.settings_of(b.index(CLEAN), &settings);
    let copied = b.set.settings_of(copy, &settings);
    assert_eq!(copied, original, "same settings");
    assert_eq!(copied.get("channel").map(String::as_str), Some("green"));
    assert_eq!(
        settings
            .state_of(&key, &["loupe_zoom"])
            .get("loupe_zoom")
            .map(String::as_str),
        Some("300")
    );
    // Independent afterwards: changing the copy leaves the original alone.
    drop(settings);
    b.set_choice(&key, "channel", "red");
    let settings = b.settings.lock().unwrap().clone();
    assert_eq!(
        b.set
            .settings_of(b.index(CLEAN), &settings)
            .get("channel")
            .map(String::as_str),
        Some("green")
    );
}

#[test]
fn the_copy_is_measured_by_the_feed_thread_from_the_next_frame() {
    // The Window and the Feed thread hold clones of one display set: a Panel made after the Feed loop was
    // built is seen by it and draws the frame on screen without waiting for a new one.
    let mut b = Bench::new("instances-feed-sees");
    b.play();
    let copy = b.duplicate(CLEAN, Dir::Row);
    b.settle();
    let first = b.index(CLEAN);
    assert_eq!(mean(&b.draw(copy)), mean(&b.draw(first)));
    assert!(mean(&b.draw(copy))[0] > 5.0, "the copy shows the picture");
}

#[test]
fn numbers_are_stable_while_panels_move_close_and_return() {
    let mut b = Bench::new("instances-numbers");
    let two = b.duplicate(CLEAN, Dir::Row);
    let three = b.duplicate(CLEAN, Dir::Row);
    assert_eq!(
        (b.set.key(two), b.set.key(three)),
        (
            "Channel Viewer 2".to_string(),
            "Channel Viewer 3".to_string()
        )
    );
    let ids = (b.set.id(two), b.set.id(three));
    // Moving Panels around does not renumber.
    b.tree.swap(two, three);
    b.tree.swap(b.index(CLEAN), two);
    assert_eq!(b.set.key(two), "Channel Viewer 2");
    assert_eq!(b.set.key(three), "Channel Viewer 3");
    // Closing the first copy never renumbers the second: its key and id stay, even though its index moved up.
    assert_eq!(b.close(two), vec![two]);
    let still = b.index("Channel Viewer 3");
    assert_eq!(b.set.id(still), ids.1);
    assert!(!b.set.keys().contains(&"Channel Viewer 2".to_string()));
    assert!(b.tree.contains(still));
    // A new copy takes the free number; the living one keeps its own.
    let again = b.duplicate(CLEAN, Dir::Col);
    assert_eq!(b.set.key(again), "Channel Viewer 2");
    assert_eq!(b.set.key(b.index("Channel Viewer 3")), "Channel Viewer 3");
    assert_ne!(b.set.id(again), ids.0, "an id is never reused");
}

#[test]
fn closing_a_panel_forgets_its_settings() {
    let mut b = Bench::new("instances-forget");
    let copy = b.duplicate(CLEAN, Dir::Row);
    let key = b.set.key(copy);
    b.set_choice(&key, "channel", "red");
    b.set_choice(CLEAN, "channel", "blue");
    b.close(copy);
    assert!(
        !b.settings.lock().unwrap().to_text().contains(&key),
        "nothing kept for the copy"
    );
    // The first instance stays (closed) so Add can bring it back, but with its settings forgotten.
    let first = b.index(CLEAN);
    assert!(
        b.close(first).is_empty(),
        "the first instance is never removed"
    );
    assert!(
        !b.settings
            .lock()
            .unwrap()
            .to_text()
            .contains("Channel Viewer\t")
    );
    assert!(!b.tree.contains(first));
    assert_eq!(b.set.keys().iter().filter(|k| *k == CLEAN).count(), 1);
}

#[test]
fn replace_keeps_place_and_size_and_starts_from_the_new_displays_defaults() {
    let mut b = Bench::new("instances-replace");
    let before = b.tree.cells(ROOM.area, ROOM.gap);
    let panel = b.index(CLEAN);
    b.set_choice("Vectorscope", "targets", "75");
    // Vectorscope is on the page, so replacing makes "Vectorscope 2" with defaults, not a copy.
    let new = {
        let mut settings = b.settings.lock().unwrap();
        page_edit::replace(
            &b.set,
            &mut b.tree,
            &mut settings,
            panel,
            "Vectorscope",
            |_| false,
        )
        .unwrap()
    };
    assert_eq!(b.set.key(new), "Vectorscope 2");
    let settings = b.settings.lock().unwrap().clone();
    let defs = b.set.setting_defs(new);
    assert_eq!(
        settings.resolve("Vectorscope 2", defs),
        settings.resolve("nothing", defs),
        "the defaults"
    );
    assert_ne!(
        settings.resolve("Vectorscope", defs),
        settings.resolve("Vectorscope 2", defs)
    );
    let after = b.tree.cells(ROOM.area, ROOM.gap);
    assert_eq!(
        after[new], before[panel],
        "the new Panel has the old place and size"
    );
    assert!(!b.tree.contains(b.index(CLEAN)), "the old Panel is closed");
    // Replacing a copy removes it: a duplicate does not linger closed, and the new Panel keeps its place.
    let copy = b.duplicate("Vectorscope", Dir::Row);
    let place = b.tree.cells(ROOM.area, ROOM.gap)[copy];
    let count = b.set.len();
    let new = {
        let mut settings = b.settings.lock().unwrap();
        let new =
            page_edit::replace(&b.set, &mut b.tree, &mut settings, copy, CLEAN, |_| false).unwrap();
        assert_eq!(
            page_edit::sweep(&b.set, &mut b.tree, &mut settings),
            vec![copy]
        );
        new
    };
    assert_eq!(
        b.set.key(new),
        CLEAN,
        "the closed first instance comes back"
    );
    assert_eq!(b.set.len(), count - 1, "the copy is gone");
    assert_eq!(b.tree.cells(ROOM.area, ROOM.gap)[new], place);
    assert!(b.tree.contains(new));
}

#[test]
fn closing_a_panel_another_page_holds_keeps_its_settings() {
    let mut b = Bench::new("instances-shared-close");
    let vec = b.index("Vectorscope");
    b.set_choice("Vectorscope", "targets", "75");
    assert!(vectorscope_settings_kept(&b));
    let mut settings = b.settings.lock().unwrap();
    page_edit::close(&b.set, &mut b.tree, &mut settings, vec, |p| p == vec);
    drop(settings);
    assert!(vectorscope_settings_kept(&b), "another page still shows it");
}

#[test]
fn replacing_a_panel_another_page_holds_keeps_its_settings() {
    let mut b = Bench::new("instances-shared-replace");
    let vec = b.index("Vectorscope");
    b.set_choice("Vectorscope", "targets", "75");
    let mut settings = b.settings.lock().unwrap();
    page_edit::replace(&b.set, &mut b.tree, &mut settings, vec, "Skin Tone", |p| {
        p == vec
    })
    .unwrap();
    drop(settings);
    assert!(vectorscope_settings_kept(&b), "another page still shows it");
}

#[test]
fn the_last_page_to_close_a_panel_forgets_its_settings() {
    let mut b = Bench::new("instances-last-close");
    let vec = b.index("Vectorscope");
    b.set_choice("Vectorscope", "targets", "75");
    let mut settings = b.settings.lock().unwrap();
    page_edit::close(&b.set, &mut b.tree, &mut settings, vec, |_| false);
    drop(settings);
    assert!(!vectorscope_settings_kept(&b));
}

#[test]
fn undoing_the_close_or_replace_of_a_duplicate_brings_it_back() {
    use scopecreep::pages::Pages;
    for replace in [false, true] {
        let b = Bench::new("instances-undo-duplicate");
        let mut pages = Pages::new(b.tree.clone());
        let mut settings = PanelSettings::default();
        let from = b.index(CLEAN);
        let copy = pages
            .change(|t| page_edit::duplicate(&b.set, t, &mut settings, from, Dir::Row, ROOM))
            .unwrap();
        let count = b.set.len();
        pages.change(|t| {
            if replace {
                page_edit::replace(&b.set, t, &mut settings, copy, "Vectorscope", |_| false);
            } else {
                page_edit::close(&b.set, t, &mut settings, copy, |_| false);
            }
        });
        // The Window sweeps after every change: the copy a step still names is not removed.
        pages.sweep(&b.set, &mut settings);
        assert!(pages.can_undo(), "replace={replace}");
        assert!(pages.undo());
        assert!(pages.tree().contains(copy), "replace={replace}: back");
        pages.sweep(&b.set, &mut settings);
        assert_eq!(b.set.key(copy), "Channel Viewer 2");
        // Undo the duplicate too: nothing names the copy now, so it goes.
        assert!(pages.undo());
        pages.sweep(&b.set, &mut settings);
        assert!(!pages.can_undo());
        assert_eq!(
            b.set.len(),
            count - 1,
            "replace={replace}: the copy is gone"
        );
    }
}

#[test]
fn a_display_that_cannot_be_copied_is_refused_with_its_own_notice() {
    use scopecreep::gamut_warning::GamutWarning;
    use scopecreep::page_edit::DuplicateRefused;
    let b = Bench::new("instances-no-copy");
    let mut set = b.set.clone();
    set.register(Arc::new(GamutWarning::new(
        &b.rig.device,
        &b.rig.queue,
        TARGET,
    )));
    let only = set.len() - 1;
    let mut tree = LayoutTree::empty(set.len());
    tree.add(only, ROOM.area, ROOM.gap, ROOM.min);
    let mut settings = PanelSettings::default();
    let refused = page_edit::duplicate(&set, &mut tree, &mut settings, only, Dir::Row, ROOM)
        .expect_err("no maker, no copy");
    assert_eq!(refused, DuplicateRefused::NoCopy);
    assert!(
        !refused.notice().contains("No room"),
        "{}",
        refused.notice()
    );
    assert_eq!(tree.panels(), vec![only], "nothing changed");
    assert_eq!(set.len(), only + 1, "no stray display");
    // A Panel with no room still says so.
    let tight = Room {
        area: [0.0, 0.0, 200.0, 120.0],
        ..ROOM
    };
    let mut tree =
        LayoutTree::default_for(&b.set.keys().iter().map(String::as_str).collect::<Vec<_>>());
    let refused = page_edit::duplicate(&b.set, &mut tree, &mut settings, 0, Dir::Row, tight)
        .expect_err("no room");
    assert!(matches!(refused, DuplicateRefused::NoRoom(_)));
    assert!(refused.notice().contains("No room"));
}

/// Whether the Vectorscope's chosen settings are still kept.
fn vectorscope_settings_kept(b: &Bench) -> bool {
    let settings = b.settings.lock().unwrap().clone();
    let defs = b.set.setting_defs(b.index("Vectorscope"));
    settings.resolve("Vectorscope", defs) != settings.resolve("nothing", defs)
}

#[test]
fn a_picture_zoom_and_link_reach_every_picture_instance() {
    let mut b = Bench::new("instances-zoom");
    b.play();
    let copy = b.duplicate(CLEAN, Dir::Row);
    b.settle();
    let first = b.index(CLEAN);
    let contents = vec![AREA; b.set.len()];
    b.set.set_link_from_strip(true, &contents, 1.0);
    b.set.set_zoom(first, AREA, 1.0, 700.0);
    for panel in [first, copy] {
        let r = b.set.zoom(panel, AREA, 1.0).unwrap();
        assert!((r.percent - 700.0).abs() < 1.0, "Panel {panel}: {r:?}");
    }
    // The strip reads the shared zoom, and Fit brings both back.
    assert_eq!(b.set.view_strip(&contents, 1.0).readout, "700 %");
    b.set.fit_all(&contents, 1.0);
    for panel in [first, copy] {
        let r = b.set.zoom(panel, AREA, 1.0).unwrap();
        assert!(r.percent <= r.fit + 0.01, "Fit: {r:?}");
    }
}

#[test]
fn the_displays_titles_are_the_plain_names_and_the_window_adds_numbers() {
    let b = Bench::new("instances-titles");
    b.set.duplicate(b.index(CLEAN));
    b.set.duplicate(b.index("Vectorscope"));
    b.set.duplicate(b.index("Vectorscope"));
    let titles = b
        .set
        .titles(&Default::default(), &b.settings.lock().unwrap());
    let names: Vec<_> = titles.iter().map(|t| t.name.as_str()).collect();
    // The set says the display's name alone; a number is added by the Window only where a page holds two of a
    // display (pages::title_names).
    assert_eq!(names.iter().filter(|n| **n == "Channel Viewer").count(), 2);
    assert_eq!(names.iter().filter(|n| **n == "Vectorscope").count(), 3);
    let keys = b.set.keys();
    let shown = scopecreep::pages::title_names(
        &b.set.names(),
        &keys,
        &(0..b.set.len()).collect::<Vec<_>>(),
    );
    for expected in [
        "Channel Viewer",
        "Channel Viewer 2",
        "Vectorscope",
        "Vectorscope 2",
        "Vectorscope 3",
    ] {
        assert!(
            shown.iter().any(|n| n == expected),
            "{expected} in {shown:?}"
        );
    }
}

#[test]
fn duplicate_and_replace_are_one_undo_step_each() {
    use scopecreep::layout_history::LayoutHistory;
    let mut b = Bench::new("instances-undo");
    let mut history = LayoutHistory::default();
    let start = b.tree.clone();
    let panel = b.index(CLEAN);
    // Duplicate: one step; undo takes the copy off the page and the sweep removes it.
    {
        let mut settings = b.settings.lock().unwrap();
        history.change(&mut b.tree, |t| {
            page_edit::duplicate(&b.set, t, &mut settings, panel, Dir::Row, ROOM).unwrap()
        });
    }
    let copies = b.set.len();
    assert!(history.undo(&mut b.tree));
    assert_eq!(b.tree, start, "one undo is back where it began");
    assert!(!history.can_undo(), "and that was one step");
    {
        let mut settings = b.settings.lock().unwrap();
        for removed in page_edit::sweep(&b.set, &mut b.tree, &mut settings) {
            history.forget_index(removed);
        }
    }
    assert_eq!(b.set.len(), copies - 1, "the undone copy does not linger");
    // Replace: one step; undo puts the old Panel back in the same place.
    let before = b.tree.cells(ROOM.area, ROOM.gap);
    {
        let mut settings = b.settings.lock().unwrap();
        history.change(&mut b.tree, |t| {
            page_edit::replace(&b.set, t, &mut settings, panel, "Vectorscope", |_| false).unwrap()
        });
    }
    assert!(history.undo(&mut b.tree));
    assert!(!history.can_undo());
    assert_eq!(b.tree.cells(ROOM.area, ROOM.gap)[panel], before[panel]);
    assert!(b.tree.contains(panel));
}

#[test]
fn removing_a_display_keeps_the_history_naming_the_same_panels() {
    use scopecreep::layout_history::LayoutHistory;
    let mut b = Bench::new("instances-history-shift");
    let mut history = LayoutHistory::default();
    let copy = b.duplicate(CLEAN, Dir::Row);
    let skin = b.index("Skin Tone");
    // A step taken while the copy (index 7) exists: close "Skin Tone".
    history.change(&mut b.tree, |t| t.close(skin));
    // The copy is closed and removed (no undo of its own here): indexes above it, none, but history must forget it.
    let mut settings = b.settings.lock().unwrap();
    b.tree.close(copy);
    let removed = page_edit::sweep(&b.set, &mut b.tree, &mut settings);
    assert_eq!(removed, vec![copy]);
    for r in removed {
        history.forget_index(r);
    }
    assert!(history.undo(&mut b.tree));
    assert!(b.tree.contains(skin), "undo brings Skin tone back");
    assert!(
        !b.tree.contains(copy.min(b.set.len())),
        "and not the removed copy"
    );
}
