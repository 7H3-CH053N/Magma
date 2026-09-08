//! Remembering vectors, so a vault is embedded once rather than once per query.
//!
//! Without this every search would run the whole vault through the encoder,
//! which takes minutes. With it, only passages whose text changed are computed
//! again, and a query costs one forward pass.
//!
//! The cache lives beside the model in the app's data directory, never in the
//! vault: it is derived from the notes, not part of them, and a folder of
//! markdown must stay a folder of markdown.

use magma_core::Similarity;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How often the cache is allowed to hit the disk while it is filling.
///
/// Saving after every batch sounded safe and was quadratic: the whole file is
/// rewritten each time, so a vault of a few thousand passages wrote hundreds of
/// megabytes to keep a few. Five seconds bounds the loss from a kill to five
/// seconds of encoding, which is the only thing the frequency was ever buying.
const SAVE_EVERY: Duration = Duration::from_secs(5);

/// File magic and format version. A format change bumps the digit, and an
/// unrecognised file is treated as no cache rather than as an error: a stale
/// cache costs one re-embedding, a hard failure costs the feature.
const MAGIC: &[u8; 8] = b"MGEMB01\n";

/// A stable 64-bit hash of the passage text.
///
/// Deliberately not `DefaultHasher`: that is explicitly allowed to change
/// between Rust releases, and this key is written to disk. A silent
/// cache-wide miss after a toolchain upgrade is not a bug anyone would notice,
/// only an unexplained slowdown. FNV-1a is fixed forever.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// Hash plus byte length. The length is not redundant: on a hash collision two
/// different passages would otherwise swap meanings, and that is a wrong answer
/// rather than a slow one.
type Key = (u64, u32);

fn key_of(text: &str) -> Key {
    (fnv1a(text.as_bytes()), text.len() as u32)
}

/// An encoder that remembers what it has already embedded.
pub struct Cached<S: Similarity> {
    inner: S,
    entries: Mutex<HashMap<Key, Vec<f32>>>,
    path: PathBuf,
    /// Set when something was added since the last save, so an unchanged vault
    /// does not rewrite the file on every query.
    dirty: Mutex<bool>,
    /// When set, unknown passages are answered with a zero vector instead of
    /// being encoded. See [`Self::lookup_only`].
    lookup_only: bool,
    last_save: Mutex<Instant>,
}

impl<S: Similarity> Cached<S> {
    /// Wrap an encoder, loading whatever was cached for it before.
    ///
    /// Vectors from another model are discarded rather than reused: they are
    /// numbers in a different space, and comparing across the two produces
    /// confident nonsense.
    pub fn open(inner: S, path: PathBuf) -> Self {
        let entries = read_file(&path, inner.id()).unwrap_or_default();
        Self {
            inner,
            entries: Mutex::new(entries),
            path,
            dirty: Mutex::new(false),
            lookup_only: false,
            // Due immediately, so the very first batch is written rather
            // than held back: a run killed in its first seconds should still
            // leave something behind. Only the ones after it are throttled.
            last_save: Mutex::new(
                Instant::now()
                    .checked_sub(SAVE_EVERY)
                    .unwrap_or_else(Instant::now),
            ),
        }
    }

    /// The encoder underneath, for a caller that wants to drive batches itself.
    pub fn inner(&self) -> &S {
        &self.inner
    }

    /// Which of these have no vector yet, deduplicated and in order.
    pub fn missing(&self, texts: &[String]) -> Vec<String> {
        let entries = match self.entries.lock() {
            Ok(e) => e,
            Err(_) => return texts.to_vec(),
        };
        let mut seen = std::collections::HashSet::new();
        texts
            .iter()
            .filter(|t| {
                let key = key_of(t);
                !entries.contains_key(&key) && seen.insert(key)
            })
            .cloned()
            .collect()
    }

    /// Take vectors computed elsewhere, in the same order as their texts.
    pub fn insert_many(&self, texts: &[String], vectors: Vec<Vec<f32>>) -> Result<(), String> {
        if texts.len() != vectors.len() {
            return Err("vectors do not line up with their texts".into());
        }
        let mut entries = self.entries.lock().map_err(|_| "cache poisoned")?;
        for (text, vector) in texts.iter().zip(vectors) {
            entries.insert(key_of(text), vector);
        }
        *self.dirty.lock().map_err(|_| "cache poisoned")? = true;
        Ok(())
    }

    /// Save, but not more often than [`SAVE_EVERY`].
    pub fn save_if_due(&self) -> Result<(), String> {
        let due = {
            let last = self.last_save.lock().map_err(|_| "cache poisoned")?;
            last.elapsed() >= SAVE_EVERY
        };
        if !due {
            return Ok(());
        }
        self.save()?;
        *self.last_save.lock().map_err(|_| "cache poisoned")? = Instant::now();
        Ok(())
    }

    /// Answer only from what is already cached, and never encode.
    ///
    /// This is how a *query* must run. Encoding on demand means the first
    /// search after a restart pays for the whole vault, one forward pass per
    /// passage, inside a tool call that has a timeout — which is not slow, it
    /// is broken: the call dies, nothing is kept, and the next one starts over.
    ///
    /// An unindexed passage comes back as a zero vector, so its cosine is zero
    /// and it simply does not rank on meaning. It is still found by words.
    /// Filling the cache is [`crate::index_vault`]'s job, where the work is
    /// visible, interruptible and saved as it goes.
    pub fn lookup_only(mut self, yes: bool) -> Self {
        self.lookup_only = yes;
        self
    }

    /// Width of the vectors held, for making a neutral one.
    fn width(&self) -> usize {
        self.entries
            .lock()
            .ok()
            .and_then(|e| e.values().next().map(|v| v.len()))
            .unwrap_or(1)
    }

    /// How many vectors are held. Mostly for tests and for telling a user
    /// whether a first run is still ahead of them.
    pub fn len(&self) -> usize {
        self.entries.lock().map(|e| e.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write the cache out. Cheap and skipped when nothing changed.
    pub fn save(&self) -> Result<(), String> {
        let mut dirty = self.dirty.lock().map_err(|_| "cache poisoned")?;
        if !*dirty {
            return Ok(());
        }
        let entries = self.entries.lock().map_err(|_| "cache poisoned")?;
        write_file(&self.path, self.inner.id(), &entries)?;
        *dirty = false;
        Ok(())
    }

    /// Drop vectors for passages that no longer exist.
    ///
    /// Edited and deleted notes leave their old passages behind; without this
    /// the file only ever grows. Called with every current passage text.
    pub fn retain_only(&self, live: &[String]) {
        let keep: std::collections::HashSet<Key> = live.iter().map(|t| key_of(t)).collect();
        if let (Ok(mut entries), Ok(mut dirty)) = (self.entries.lock(), self.dirty.lock()) {
            let before = entries.len();
            entries.retain(|k, _| keep.contains(k));
            if entries.len() != before {
                *dirty = true;
            }
        }
    }
}

impl<S: Similarity> Similarity for Cached<S> {
    fn id(&self) -> &str {
        self.inner.id()
    }

    /// Queries are not cached. Each one is new, and remembering them would grow
    /// the file for nothing.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        self.inner.embed_query(text)
    }

    fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        // Which of these have never been seen. Kept in order, and deduplicated,
        // because a vault happily contains the same paragraph twice.
        let mut missing: Vec<String> = Vec::new();
        {
            let entries = self.entries.lock().map_err(|_| "cache poisoned")?;
            let mut seen = std::collections::HashSet::new();
            for text in texts {
                let key = key_of(text);
                if !entries.contains_key(&key) && seen.insert(key) {
                    missing.push(text.clone());
                }
            }
        }

        if !missing.is_empty() && !self.lookup_only {
            let fresh = self.inner.embed_passages(&missing)?;
            if fresh.len() != missing.len() {
                return Err("model returned the wrong number of vectors".into());
            }
            {
                let mut entries = self.entries.lock().map_err(|_| "cache poisoned")?;
                for (text, vector) in missing.iter().zip(fresh) {
                    entries.insert(key_of(text), vector);
                }
            }
            *self.dirty.lock().map_err(|_| "cache poisoned")? = true;
            // Save as we go rather than at the end. Encoding a vault takes
            // minutes and the process can be killed at any point in them; a
            // cache written only on a clean finish would keep nothing at all
            // from an interrupted run, and every attempt would start over.
            // Throttled, because the whole file is rewritten each time.
            self.save_if_due()?;
        }

        let zero = vec![0f32; self.width()];
        let entries = self.entries.lock().map_err(|_| "cache poisoned")?;
        Ok(texts
            .iter()
            .map(|t| {
                entries
                    .get(&key_of(t))
                    .cloned()
                    .unwrap_or_else(|| zero.clone())
            })
            .collect())
    }
}

fn read_file(path: &Path, model_id: &str) -> Option<HashMap<Key, Vec<f32>>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }
    let id = read_string(&mut file)?;
    if id != model_id {
        return None;
    }
    let dim = read_u32(&mut file)? as usize;
    let count = read_u32(&mut file)? as usize;
    let mut out = HashMap::with_capacity(count);
    for _ in 0..count {
        let hash = read_u64(&mut file)?;
        let len = read_u32(&mut file)?;
        let mut floats = vec![0f32; dim];
        for slot in floats.iter_mut() {
            let mut b = [0u8; 4];
            file.read_exact(&mut b).ok()?;
            *slot = f32::from_le_bytes(b);
        }
        out.insert((hash, len), floats);
    }
    Some(out)
}

fn write_file(path: &Path, model_id: &str, entries: &HashMap<Key, Vec<f32>>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let dim = entries.values().next().map(|v| v.len()).unwrap_or(0);
    // A ragged cache cannot be written in a fixed-width format, and would mean
    // the encoder changed shape under us. Better to keep nothing than to write
    // a file that reads back as garbage.
    if entries.values().any(|v| v.len() != dim) {
        return Err("vectors of mixed width; refusing to write the cache".into());
    }

    // Write beside the target and rename, so an interrupted save leaves the
    // previous cache intact rather than a truncated one.
    let tmp = path.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    file.write_all(MAGIC).map_err(|e| e.to_string())?;
    write_string(&mut file, model_id)?;
    file.write_all(&(dim as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&(entries.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    for ((hash, len), vector) in entries {
        file.write_all(&hash.to_le_bytes())
            .map_err(|e| e.to_string())?;
        file.write_all(&len.to_le_bytes())
            .map_err(|e| e.to_string())?;
        for f in vector {
            file.write_all(&f.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn read_u32(file: &mut std::fs::File) -> Option<u32> {
    let mut b = [0u8; 4];
    file.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b))
}

fn read_u64(file: &mut std::fs::File) -> Option<u64> {
    let mut b = [0u8; 8];
    file.read_exact(&mut b).ok()?;
    Some(u64::from_le_bytes(b))
}

fn read_string(file: &mut std::fs::File) -> Option<String> {
    let len = read_u32(file)? as usize;
    let mut bytes = vec![0u8; len];
    file.read_exact(&mut bytes).ok()?;
    String::from_utf8(bytes).ok()
}

fn write_string(file: &mut std::fs::File, s: &str) -> Result<(), String> {
    file.write_all(&(s.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(s.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Counts what it was asked to embed, so a test can tell a cache hit from a
    /// second forward pass.
    struct Counting {
        id: String,
        calls: AtomicUsize,
        texts: AtomicUsize,
    }

    impl Counting {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                calls: AtomicUsize::new(0),
                texts: AtomicUsize::new(0),
            }
        }
    }

    impl Similarity for Counting {
        fn id(&self) -> &str {
            &self.id
        }
        fn embed_query(&self, _: &str) -> Result<Vec<f32>, String> {
            Ok(vec![1.0, 0.0, 0.0])
        }
        fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.texts.fetch_add(texts.len(), Ordering::SeqCst);
            Ok(texts
                .iter()
                .map(|t| vec![t.len() as f32, 1.0, 0.0])
                .collect())
        }
    }

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("magma-embed-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("vectors.bin")
    }

    fn texts(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_second_pass_over_the_same_vault_embeds_nothing() {
        let cache = Cached::open(Counting::new("m"), tmp("repeat"));
        let batch = texts(&["eins", "zwei", "drei"]);
        let first = cache.embed_passages(&batch).unwrap();
        let second = cache.embed_passages(&batch).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            cache.inner.texts.load(Ordering::SeqCst),
            3,
            "embedded twice"
        );
    }

    #[test]
    fn only_the_new_passages_are_embedded() {
        let cache = Cached::open(Counting::new("m"), tmp("delta"));
        cache.embed_passages(&texts(&["eins", "zwei"])).unwrap();
        cache.embed_passages(&texts(&["zwei", "drei"])).unwrap();
        // "zwei" was known; only "drei" cost a forward pass.
        assert_eq!(cache.inner.texts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn a_repeated_paragraph_is_embedded_once() {
        let cache = Cached::open(Counting::new("m"), tmp("dupe"));
        let out = cache
            .embed_passages(&texts(&["gleich", "gleich", "anders"]))
            .unwrap();
        assert_eq!(cache.inner.texts.load(Ordering::SeqCst), 2);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], out[1]);
    }

    #[test]
    fn vectors_survive_a_restart() {
        let path = tmp("persist");
        let batch = texts(&["eins", "zwei"]);
        let before = {
            let cache = Cached::open(Counting::new("m"), path.clone());
            let v = cache.embed_passages(&batch).unwrap();
            cache.save().unwrap();
            v
        };
        let cache = Cached::open(Counting::new("m"), path);
        let after = cache.embed_passages(&batch).unwrap();
        assert_eq!(before, after);
        assert_eq!(
            cache.inner.texts.load(Ordering::SeqCst),
            0,
            "the file was not read back"
        );
    }

    #[test]
    fn another_models_vectors_are_not_reused() {
        // The dangerous case: numbers from a different space compare fine and
        // mean nothing, so this must miss rather than hit.
        let path = tmp("model");
        {
            let cache = Cached::open(Counting::new("old-model"), path.clone());
            cache.embed_passages(&texts(&["eins"])).unwrap();
            cache.save().unwrap();
        }
        let cache = Cached::open(Counting::new("new-model"), path);
        cache.embed_passages(&texts(&["eins"])).unwrap();
        assert_eq!(cache.inner.texts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_deleted_passage_stops_taking_up_room() {
        let path = tmp("retain");
        let cache = Cached::open(Counting::new("m"), path);
        cache
            .embed_passages(&texts(&["eins", "zwei", "drei"]))
            .unwrap();
        assert_eq!(cache.len(), 3);
        cache.retain_only(&texts(&["eins", "drei"]));
        assert_eq!(cache.len(), 2);
    }

    // The bug this replaced: `save` existed, was tested, and nothing ever
    // called it. Every process start re-encoded the whole vault, so the cache
    // was a unit that worked inside a feature that did not.
    #[test]
    fn vectors_reach_the_disk_without_anyone_asking() {
        let path = tmp("autosave");
        {
            let cache = Cached::open(Counting::new("m"), path.clone());
            cache.embed_passages(&texts(&["eins", "zwei"])).unwrap();
            // Deliberately no save() here, and no clean shutdown either.
        }
        let cache = Cached::open(Counting::new("m"), path);
        assert_eq!(cache.len(), 2, "nothing was written");
    }

    // The other half: a query must never encode. Encoding on demand meant the
    // first search after a restart ran the whole vault through the model inside
    // a tool call with a timeout, which killed the call and kept nothing.
    #[test]
    fn a_lookup_only_cache_never_runs_the_model() {
        let cache = Cached::open(Counting::new("m"), tmp("lookup")).lookup_only(true);
        let out = cache.embed_passages(&texts(&["nie gesehen"])).unwrap();
        assert_eq!(cache.inner.texts.load(Ordering::SeqCst), 0);
        // A neutral vector, so an unindexed passage simply does not rank on
        // meaning. It is still found by words.
        assert!(out[0].iter().all(|f| *f == 0.0), "{:?}", out[0]);
    }

    #[test]
    fn a_lookup_only_cache_still_answers_from_what_it_has() {
        let path = tmp("lookupwarm");
        {
            let warm = Cached::open(Counting::new("m"), path.clone());
            warm.embed_passages(&texts(&["bekannt"])).unwrap();
        }
        let cache = Cached::open(Counting::new("m"), path).lookup_only(true);
        let out = cache
            .embed_passages(&texts(&["bekannt", "unbekannt"]))
            .unwrap();
        assert!(
            out[0].iter().any(|f| *f != 0.0),
            "the cached vector was lost"
        );
        assert!(out[1].iter().all(|f| *f == 0.0));
        assert_eq!(cache.inner.texts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn missing_reports_only_what_is_new_and_says_it_once() {
        let cache = Cached::open(Counting::new("m"), tmp("missing"));
        cache.embed_passages(&texts(&["bekannt"])).unwrap();
        let todo = cache.missing(&texts(&["bekannt", "neu", "neu"]));
        assert_eq!(todo, vec!["neu".to_string()]);
    }

    #[test]
    fn insert_many_refuses_vectors_that_do_not_line_up() {
        // The silent-corruption case: one vector short and every passage after
        // it takes its neighbour's meaning.
        let cache = Cached::open(Counting::new("m"), tmp("lineup"));
        let err = cache
            .insert_many(&texts(&["eins", "zwei"]), vec![vec![1.0, 0.0]])
            .unwrap_err();
        assert!(err.contains("line up"), "{err}");
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn a_corrupt_file_costs_a_re_embedding_not_the_feature() {
        let path = tmp("corrupt");
        std::fs::write(&path, b"not a cache at all").unwrap();
        let cache = Cached::open(Counting::new("m"), path);
        assert!(cache.is_empty());
        assert!(cache.embed_passages(&texts(&["eins"])).is_ok());
    }

    #[test]
    fn nothing_is_written_when_nothing_changed() {
        let path = tmp("clean");
        let cache = Cached::open(Counting::new("m"), path.clone());
        cache.save().unwrap();
        assert!(!path.exists(), "an empty cache wrote a file");
    }
}
