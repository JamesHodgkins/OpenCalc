//! Thumbnails of worksheet files for the Home page: the top of the first page,
//! drawn by the same code as the export. A file is read, calculated and drawn on
//! a thread of its own so the window never waits for it; the picture appears on
//! the card when it is ready. Pictures are kept for the session only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use opencalc_core::eval::evaluate_all;
use opencalc_core::persist;
use opencalc_core::registry::UnitRegistry;

use crate::export::{render_pages, Source};
use crate::theme::{PaletteKey, Theme};

/// Pixels per document unit: a page is about 280 pixels wide.
const SCALE: f32 = 0.36;

/// What identifies one version of a file.
pub type Stamp = (SystemTime, u64);

struct Request {
    path: PathBuf,
    stamp: Stamp,
}

struct Done {
    path: PathBuf,
    stamp: Stamp,
    image: Result<ColorImage, String>,
}

enum State {
    Pending,
    Ready(TextureHandle),
    Failed,
}

#[derive(Default)]
struct Entries {
    map: HashMap<PathBuf, (Stamp, State)>,
}

/// The thumbnails the Home page has asked for. Shared by reference: asking for one
/// never needs `&mut`, so a page that only reads the app state can use it.
#[derive(Default)]
pub struct Thumbs {
    entries: Mutex<Entries>,
    worker: OnceLock<(Sender<Request>, Mutex<Receiver<Done>>)>,
}

impl Thumbs {
    /// The picture for `path` as it is now (`stamp`), if it is ready. The first call
    /// starts drawing it; the window is asked to repaint when it is done.
    pub fn texture(&self, ctx: &Context, path: &Path, stamp: Stamp) -> Option<TextureHandle> {
        let (to_worker, from_worker) = self.worker.get_or_init(|| spawn(ctx.clone()));
        let mut entries = self.entries.lock().ok()?;
        // Take in what the worker has finished since the last look.
        if let Ok(from_worker) = from_worker.lock() {
            while let Ok(done) = from_worker.try_recv() {
                let state = match done.image {
                    Ok(image) => State::Ready(ctx.load_texture(
                        format!("thumb-{}", done.path.display()),
                        image,
                        TextureOptions::LINEAR,
                    )),
                    Err(_) => State::Failed,
                };
                // A picture of an older version of the file is no use.
                if entries
                    .map
                    .get(&done.path)
                    .is_some_and(|(s, _)| *s == done.stamp)
                {
                    entries.map.insert(done.path, (done.stamp, state));
                }
            }
        }
        match entries.map.get(path) {
            Some((s, State::Ready(texture))) if *s == stamp => Some(texture.clone()),
            Some((s, _)) if *s == stamp => None,
            _ => {
                entries.map.insert(path.to_owned(), (stamp, State::Pending));
                let _ = to_worker.send(Request {
                    path: path.to_owned(),
                    stamp,
                });
                None
            }
        }
    }

    /// Whether drawing `path` was tried and failed (a file that is not a worksheet).
    #[cfg(test)]
    pub fn failed(&self, path: &Path) -> bool {
        self.entries
            .lock()
            .is_ok_and(|e| matches!(e.map.get(path), Some((_, State::Failed))))
    }
}

fn spawn(ctx: Context) -> (Sender<Request>, Mutex<Receiver<Done>>) {
    let (to_worker, requests) = channel::<Request>();
    let (done, from_worker) = channel::<Done>();
    // Calculating a worksheet can recurse deeply: give the thread the stack the
    // evaluator is sized for.
    let _ = std::thread::Builder::new()
        .name("thumbnails".to_owned())
        .stack_size(opencalc_core::worker::EVAL_STACK)
        .spawn(move || {
            while let Ok(request) = requests.recv() {
                let image = render(&request.path);
                let sent = done.send(Done {
                    path: request.path,
                    stamp: request.stamp,
                    image,
                });
                if sent.is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
    (to_worker, Mutex::new(from_worker))
}

/// The first page of the worksheet in the file, small.
pub fn render(path: &Path) -> Result<ColorImage, String> {
    let bytes = persist::read_limited(path).map_err(|e| e.to_string())?;
    let loaded = persist::load_bytes(&bytes).map_err(|e| e.to_string())?;
    let doc = loaded.worksheet;
    let units = doc.units().unwrap_or_else(|_| UnitRegistry::builtin());
    let output = evaluate_all(&doc.math_regions(), &doc.plot_inputs(), &units);
    let theme = Theme::palette(PaletteKey::Neutral);
    let source = Source {
        doc: &doc,
        output: &output,
        theme: &theme,
    };
    const STOP: &str = "first page done";
    let mut first = None;
    let result = render_pages(&source, SCALE, |_, image, _| {
        first = Some(ColorImage::from_rgb(
            [image.width, image.height],
            &image.rgb(),
        ));
        // One page is all that is wanted: stop the rest being drawn.
        Err(STOP.to_owned())
    });
    match (first, result) {
        (Some(image), _) => Ok(image),
        (None, Err(reason)) => Err(reason),
        (None, Ok(_)) => Err("the worksheet has no pages".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::{Point, RegionKind, Worksheet};

    fn write_sample(name: &str) -> PathBuf {
        let mut ws = Worksheet::new("sample");
        let r = ws.new_region(RegionKind::math("F := 10 kN"), Point::new(80.0, 80.0));
        ws.apply(opencalc_core::edit::Edit::InsertRegions {
            regions: vec![(0, r)],
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("opencalc-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, persist::save_bytes(&ws).unwrap()).unwrap();
        path
    }

    #[test]
    fn a_worksheet_file_is_drawn_as_its_first_page_with_something_on_it() {
        let path = write_sample("sample.ocalc");
        let image = render(&path).expect("drawn");
        assert!(
            image.size[0] > 150 && image.size[1] > image.size[0],
            "{:?}",
            image.size
        );
        let dark = image
            .pixels
            .iter()
            .filter(|p| p.r() < 128 && p.g() < 128 && p.b() < 128)
            .count();
        assert!(dark > 20, "the equation is on the page: {dark} dark pixels");
    }

    #[test]
    fn something_that_is_not_a_worksheet_is_an_error_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("opencalc-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("junk.ocalc");
        std::fs::write(&path, b"this is not a zip").unwrap();
        assert!(render(&path).is_err());
        assert!(render(&dir.join("not-there.ocalc")).is_err());
    }

    #[test]
    fn asking_starts_the_drawing_and_the_picture_turns_up_later() {
        let path = write_sample("later.ocalc");
        let stamp = {
            let m = std::fs::metadata(&path).unwrap();
            (m.modified().unwrap(), m.len())
        };
        let ctx = Context::default();
        let thumbs = Thumbs::default();
        assert!(
            thumbs.texture(&ctx, &path, stamp).is_none(),
            "not ready at once"
        );
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut got = None;
        while got.is_none() && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(20));
            got = thumbs.texture(&ctx, &path, stamp);
        }
        let texture = got.expect("a picture arrives");
        assert!(texture.size()[0] > 150);
        // A changed file is drawn again; the old picture is not offered for it.
        let newer = (stamp.0, stamp.1 + 1);
        assert!(thumbs.texture(&ctx, &path, newer).is_none());

        // And a file that cannot be drawn is remembered as failed.
        let junk = path.with_file_name("junk2.ocalc");
        std::fs::write(&junk, b"nope").unwrap();
        let junk_stamp = (SystemTime::now(), 4);
        let _ = thumbs.texture(&ctx, &junk, junk_stamp);
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !thumbs.failed(&junk) && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(20));
            let _ = thumbs.texture(&ctx, &junk, junk_stamp);
        }
        assert!(thumbs.failed(&junk));
    }
}
