//! Virtual file system for the designer's project files, backed by
//! `localStorage` for now. A desktop build of the designer would swap
//! this module for one writing real files to disk — callers only ever
//! see `list`/`read`/`write`, so nothing else needs to change for that.

use web_sys::Storage;

const INDEX_KEY: &str = "rad:files";

fn storage() -> Storage {
    web_sys::window()
        .expect("no window")
        .local_storage()
        .expect("localStorage should not throw")
        .expect("localStorage should be available")
}

fn file_key(name: &str) -> String {
    format!("rad:file:{name}")
}

/// Every filename ever written, in write order. There's no real
/// directory to list, so this index is the only record of "what files
/// exist" — `write` keeps it up to date.
pub fn list() -> Vec<String> {
    storage()
        .get_item(INDEX_KEY)
        .ok()
        .flatten()
        .map(|names| names.split('\n').filter(|n| !n.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn remember(name: &str) {
    let mut names = list();
    if !names.iter().any(|n| n == name) {
        names.push(name.to_string());
        storage().set_item(INDEX_KEY, &names.join("\n")).ok();
    }
}

pub fn read(name: &str) -> Option<String> {
    storage().get_item(&file_key(name)).ok().flatten()
}

pub fn write(name: &str, content: &str) {
    remember(name);
    storage().set_item(&file_key(name), content).ok();
}

pub fn remove(name: &str) {
    let names: Vec<String> = list().into_iter().filter(|n| n != name).collect();
    storage().set_item(INDEX_KEY, &names.join("\n")).ok();
    storage().remove_item(&file_key(name)).ok();
}

pub fn exists(name: &str) -> bool {
    list().iter().any(|n| n == name)
}

/// Wipes every file this project has ever written — "Reset project".
/// Only our own `rad:*` keys, not the whole origin's `localStorage`, in
/// case something else on this page ever starts using it too.
pub fn clear_all() {
    let store = storage();
    for name in list() {
        store.remove_item(&file_key(&name)).ok();
    }
    store.remove_item(INDEX_KEY).ok();
}
