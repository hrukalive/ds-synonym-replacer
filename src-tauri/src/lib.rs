use tauri::Wry;
use std::cmp::{max, min};
use std::collections::HashSet;
use std::default::Default;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs;
use tauri_plugin_store::{with_store, Store, StoreBuilder, StoreCollection};
use std::{fs, io};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use pest::Parser;
use pest_derive::Parser;
use chardetng::EncodingDetector;
use rodio::{Decoder, OutputStream, Sink, Source, Sample, OutputStreamHandle};
use std::time::Duration;
use rodio::cpal::traits::{HostTrait, DeviceTrait};
use rodio::cpal;
use rodio::source::SineWave;


#[derive(Clone, serde::Serialize)]
struct Payload {
    args: Vec<String>,
    cwd: String,
}

#[derive(Parser)]
#[grammar = "textgrid.pest"]
struct TextGridParser;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct TextGridInterval {
    xmin: f32,
    xmax: f32,
    text: String,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct TextGridIntervals {
    name: String,
    intervals: Vec<TextGridInterval>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct TextGrid {
    items: Vec<TextGridIntervals>,
}

fn textgrid_to_string(tg: &TextGrid) -> String {
    let mut result = String::new();
    result.push_str("File type = \"ooTextFile\"\n");
    result.push_str("Object class = \"TextGrid\"\n");
    result.push_str("\n");
    result.push_str("xmin = 0\n");
    result.push_str(&format!("xmax = {}\n", tg.items.iter().map(|item| item.intervals.iter().map(|interval| interval.xmax).fold(0.0/0.0, f32::max)).fold(0.0/0.0, f32::max)));
    result.push_str("tiers? <exists>\n");
    result.push_str(&format!("size = {}\n", tg.items.len()));
    result.push_str("item []:\n");

    for (i, item) in tg.items.iter().enumerate() {
        result.push_str(&format!("    item [{}]:\n", i + 1));
        result.push_str(&format!("        class = \"IntervalTier\"\n"));
        result.push_str(&format!("        name = \"{}\"\n", item.name));
        result.push_str(&format!("        xmin = 0\n"));
        result.push_str(&format!("        xmax = {}\n", item.intervals.iter().map(|interval| interval.xmax).fold(0.0/0.0, f32::max)));
        result.push_str(&format!("        intervals: size = {}\n", item.intervals.len()));

        for (j, interval) in item.intervals.iter().enumerate() {
            result.push_str(&format!("            intervals [{}]:\n", j + 1));
            result.push_str(&format!("                xmin = {}\n", interval.xmin));
            result.push_str(&format!("                xmax = {}\n", interval.xmax));
            result.push_str(&format!("                text = \"{}\"\n", interval.text));
        }
    }
    result
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct ReplaceRule {
    rule_name: String,
    search_terms: Vec<String>,
    replace_options: Vec<String>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct AppProjectState {
    tg_folder: Option<PathBuf>,
    wav_folder: Option<PathBuf>,
    rules: Vec<ReplaceRule>,
    selected_rule_idx: Option<i32>,
    selected_term_idx: Option<i32>,
    selected_opt_idx: Option<i32>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct AppSettings {
    theme: String,
    sound_device: Option<String>,
    volume_factor: f32,
    auto_save: bool,
    auto_next: bool,
}

impl AppSettings {
    pub fn load_from_store<R: tauri::Runtime>(
        store: &Store<R>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let theme = store
            .get("appSettings.theme")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "cupcake".to_string());

        let sound_device = store
            .get("appSettings.sound_device")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let volume_factor = store
            .get("appSettings.volume_factor")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(1.0);

        let auto_save = store
            .get("appSettings.auto_save")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let auto_next = store
            .get("appSettings.auto_next")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(AppSettings {
            theme,
            sound_device,
            volume_factor,
            auto_save,
            auto_next,
        })
    }

    pub fn save_to_store<R: tauri::Runtime>(&self, store: &mut Store<R>) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_value(&self)?;
        store.insert("appSettings.theme".to_string(), json.get("theme").unwrap().clone())?;
        store.insert("appSettings.sound_device".to_string(), json.get("sound_device").unwrap().clone())?;
        store.insert("appSettings.volume_factor".to_string(), json.get("volume_factor").unwrap().clone())?;
        store.insert("appSettings.auto_save".to_string(), json.get("auto_save").unwrap().clone())?;
        store.insert("appSettings.auto_next".to_string(), json.get("auto_next").unwrap().clone())?;
        store.save()?;
        Ok(())
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ItemRecord {
    tg_file: PathBuf,
    tg_stem: String,
    tg_content: TextGrid,
    wav_file: Option<PathBuf>,
    found_mark_idxs: Vec<usize>,
    found_mark_titles: Vec<String>,
    replace_options: Vec<String>,
    selected_options: Vec<Option<i32>>,
    dirty: bool,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct SessionItems {
    items: Vec<ItemRecord>,
    selected_item: Option<i32>,
    selected_mark: Vec<Option<i32>>,
}

/// Internal function that builds a `FadeOut` object.
pub fn fadeout<I: Source>(input: I, duration: Duration, total_duration: Option<Duration>) -> FadeOut<I> where <I as Iterator>::Item: Sample {
    let duration = duration.as_secs() * 1000000000 + duration.subsec_nanos() as u64;
    // let input_dur = input.total_duration().expect("Cannot get input duration.");
    let input_dur = total_duration.unwrap_or_else(|| input.total_duration().expect("Cannot get input duration."));
    let start_fade = input_dur.as_secs() * 1000000000 + input_dur.subsec_nanos() as u64 - duration;

    FadeOut {
        input,
        current_ns: 0.,
        start_fade: start_fade as f32,
        total_ns: duration as f32,
    }
}

/// Filter that modifies reduces the volume to silence over a time period.
#[derive(Clone, Debug)]
pub struct FadeOut<I: Source> where <I as Iterator>::Item: Sample {
    input: I,
    current_ns: f32,
    start_fade: f32,
    total_ns: f32,
}

impl<I: Source> FadeOut<I> where <I as Iterator>::Item: Sample {
    /// Starts the fade to silence.
    #[inline]
    pub fn start(&mut self) {
        self.current_ns = 0.;
    }

    /// Clears the fade out time.
    #[inline]
    pub fn reset(&mut self) {
        self.current_ns = -1.0;
    }
}

impl<I> Iterator for FadeOut<I>
    where
        I: Source,
        I::Item: Sample,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        self.current_ns += 1000000000.0 / (self.input.sample_rate() as f32 * self.channels() as f32);
        let factor = if self.current_ns - self.start_fade <= 0.0 {
            1.
        } else {
            1. - ((self.current_ns - self.start_fade) / self.total_ns)
        };

        self.input.next().map(|value| value.amplify(factor))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<I> ExactSizeIterator for FadeOut<I>
    where
        I: Source + ExactSizeIterator,
        I::Item: Sample,
{
}

impl<I> Source for FadeOut<I>
    where
        I: Source,
        I::Item: Sample,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }
}


fn parse_textgrid(tg_file: PathBuf) -> Result<TextGrid, String> {
    let file = File::open(tg_file).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).map_err(|e| e.to_string())?;

    let mut detector = EncodingDetector::new();
    detector.feed(&buffer, true);
    let encoding = detector.guess(None, true);

    let (decoded, _, _) = encoding.decode(&buffer);
    let content = decoded.to_string();

    let parsed = TextGridParser::parse(Rule::file, &content)
        .map_err(|e| e.to_string())?;

    let mut tg = TextGrid::default();

    for record in parsed {
        if record.as_rule() != Rule::file { continue; }
        for content in record.into_inner() {
            if content.as_rule() != Rule::items { continue; }
            for sub_content in content.into_inner() {
                if sub_content.as_rule() != Rule::single_item { continue; }
                let mut tg_item = TextGridIntervals::default();
                for sub_item in sub_content.into_inner() {
                    match sub_item.as_rule() {
                        Rule::property => {
                            let mut inner_rules = sub_item.into_inner();
                            let name: &str = inner_rules.next().unwrap().as_str();
                            if name == "name" {
                                tg_item.name = inner_rules.next().unwrap().as_str().to_string();
                            }
                        },
                        Rule::single_interval => {
                            let mut tg_interval = TextGridInterval::default();
                            for interval in sub_item.into_inner() {
                                if interval.as_rule() == Rule::property {
                                    let mut inner_rules = interval.into_inner();
                                    let name: &str = inner_rules.next().unwrap().as_str();
                                    let value = inner_rules.next().unwrap().as_str();
                                    match name {
                                        "xmin" => tg_interval.xmin = value.parse::<f32>().map_err(|e| e.to_string())?,
                                        "xmax" => tg_interval.xmax = value.parse::<f32>().map_err(|e| e.to_string())?,
                                        "text" => tg_interval.text = value.to_string(),
                                        _ => {}
                                    }
                                }
                            }
                            tg_item.intervals.push(tg_interval);
                        },
                        _ => {}
                    }
                }
                tg.items.push(tg_item);
            }
        }
    }
    Ok(tg)
}

fn find_marks(rule: &ReplaceRule, tg: &TextGrid) -> (Vec<usize>, Vec<String>) {
    let mut found_mark_idxs = Vec::new();
    let mut found_mark_titles = Vec::new();
    let find_word_set = rule.search_terms.iter().collect::<HashSet<_>>();
    let tg_words = &tg.items.get(0).unwrap().intervals;
    let tg_phones = &tg.items.get(1).unwrap().intervals;
    let mut corr_words = Vec::new();
    let mut i = 0;
    for word in tg_words.iter() {
        while i < tg_phones.len() {
            if (word.xmax.min(tg_phones[i].xmax) - word.xmin.max(tg_phones[i].xmin)) / (tg_phones[i].xmax - tg_phones[i].xmin) > 0.8 {
                corr_words.push(word.text.clone());
                i += 1;
            } else {
                break;
            }
        }
    }
    for (i, item) in tg_phones.iter().enumerate() {
        if find_word_set.contains(&item.text) {
            found_mark_idxs.push(i);
            let mut title = String::new();
            if corr_words.len() == tg_phones.len() {
                title.push_str(format!("({}) ", &corr_words[i]).as_str());
            }
            for j in max(0, i as i32 - 2) as usize .. i {
                title.push_str((tg_phones[j].text.clone() + " ").as_str());
            }
            title.push_str(format!("[{}] ", &tg_phones[i].text.clone()).as_str());
            for j in min(i + 1, tg_phones.len() - 1) .. min(i + 3, tg_phones.len()) {
                title.push_str((tg_phones[j].text.clone() + " ").as_str());
            }
            found_mark_titles.push(title.trim().to_string());
        }
    }
    (found_mark_idxs, found_mark_titles)
}


#[tauri::command]
fn add_rule(rule_name: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    if rule_name.trim().is_empty() { return Ok(()); }
    let mut proj_state = state.lock().unwrap();
    if proj_state.rules.iter().any(|rule| rule.rule_name == rule_name) {
        let selected_rule = proj_state.rules.iter().position(|rule| rule.rule_name == rule_name).unwrap();
        if proj_state.selected_rule_idx != Some(selected_rule as i32) {
            proj_state.selected_rule_idx = Some(selected_rule as i32);
            proj_state.selected_term_idx = if proj_state.rules[selected_rule].search_terms.is_empty() { None } else { Some(0) };
            proj_state.selected_opt_idx = if proj_state.rules[selected_rule].replace_options.is_empty() { None } else { Some(0) };
            let _ = app.emit("sync_app_selection_state", (proj_state.selected_rule_idx, proj_state.selected_term_idx, proj_state.selected_opt_idx));
        }
    } else {
        proj_state.rules.push(ReplaceRule {
            rule_name,
            search_terms: Vec::new().into_iter().collect(),
            replace_options: Vec::new(),
        });
        proj_state.selected_rule_idx = Some((proj_state.rules.len() - 1) as i32);
        proj_state.selected_term_idx = None;
        proj_state.selected_opt_idx = None;
        let _ = app.emit("sync_app_state", proj_state.clone());
    }
    Ok(())
}

#[tauri::command]
fn rename_rule(rule_index: i32, new_name: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    if new_name.trim().is_empty() { return Ok(()); }
    let mut proj_state = state.lock().unwrap();
    if rule_index >= 0 && rule_index < proj_state.rules.len() as i32 && !proj_state.rules.iter().any(|rule| rule.rule_name == new_name) {
        proj_state.rules.get_mut(rule_index as usize).unwrap().rule_name = new_name;
        let _ = app.emit("sync_app_state", proj_state.clone());
    }
    Ok(())
}

#[tauri::command]
fn remove_rule(rule_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index >= 0 && rule_index < proj_state.rules.len() as i32 {
        proj_state.rules.remove(rule_index as usize);
        if let Some(active_rule) = proj_state.selected_rule_idx {
            if active_rule as usize >= proj_state.rules.len() {
                proj_state.selected_rule_idx = if proj_state.rules.is_empty() { None } else { Some((proj_state.rules.len() - 1) as i32) };
                proj_state.selected_term_idx = None;
                proj_state.selected_opt_idx = None;
            }
        }
        let _ = app.emit("sync_app_state", proj_state.clone());
    }
    Ok(())
}

#[tauri::command]
fn select_rule(rule_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 {
        proj_state.selected_rule_idx = None;
        proj_state.selected_term_idx = None;
        proj_state.selected_opt_idx = None;
        let _ = app.emit("sync_app_selection_state", (proj_state.selected_rule_idx, proj_state.selected_term_idx, proj_state.selected_opt_idx));
    } else if proj_state.selected_rule_idx == Some(rule_index) {
    } else {
        proj_state.selected_rule_idx = Some(rule_index);
        proj_state.selected_term_idx = if proj_state.rules.get(rule_index as usize).unwrap().search_terms.is_empty() { None } else { Some(0) };
        proj_state.selected_opt_idx = if proj_state.rules.get(rule_index as usize).unwrap().replace_options.is_empty() { None } else { Some(0) };
        let _ = app.emit("sync_app_selection_state", (proj_state.selected_rule_idx, proj_state.selected_term_idx, proj_state.selected_opt_idx));
    }
    Ok(())
}

#[tauri::command]
fn add_search_term(term: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    if term.trim().is_empty() { return Ok(()); }
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let search_terms = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().search_terms;
        if search_terms.iter().any(|w| w == &term) {
            proj_state.selected_term_idx = Some(search_terms.iter().position(|w| w == &term).unwrap() as i32);
            let _ = app.emit("sync_app_selection_state", (proj_state.selected_rule_idx, proj_state.selected_term_idx, proj_state.selected_opt_idx));
        } else {
            search_terms.push(term);
            proj_state.selected_term_idx = Some(search_terms.len() as i32 - 1);
            let _ = app.emit("sync_app_state", proj_state.clone());
        }
    }
    Ok(())
}

#[tauri::command]
fn remove_search_term(term_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let search_terms = &proj_state.rules.get(rule_index as usize).unwrap().search_terms;
        if term_index >= 0 && term_index < search_terms.len() as i32 {
            proj_state.rules.get_mut(rule_index as usize).unwrap().search_terms.remove(term_index as usize);
            let search_terms = &proj_state.rules.get(rule_index as usize).unwrap().search_terms;
            if let Some(selected_term) = proj_state.selected_term_idx {
                if selected_term as usize >= search_terms.len() {
                    proj_state.selected_term_idx = if search_terms.is_empty() { None } else { Some(search_terms.len() as i32 - 1) };
                }
            }
            let _ = app.emit("sync_app_state", proj_state.clone());
        }
    }
    Ok(())
}

#[tauri::command]
fn rename_search_term(term_index: i32, new_term: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    if new_term.trim().is_empty() { return Ok(()); }
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let search_terms = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().search_terms;
        if term_index >= 0 && term_index < search_terms.len() as i32 && !search_terms.iter().any(|w| w == &new_term) {
            search_terms[term_index as usize] = new_term;
            let _ = app.emit("sync_app_state", proj_state.clone());
        }
    }
    Ok(())
}

#[tauri::command]
fn select_search_term(term_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let search_terms = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().search_terms;
        let new_val = if term_index >= 0 && term_index < search_terms.len() as i32 { Some(term_index) } else { None };
        if proj_state.selected_term_idx != new_val {
            proj_state.selected_term_idx = new_val;
            let _ = app.emit("sync_app_selection_state", (proj_state.selected_rule_idx, proj_state.selected_term_idx, proj_state.selected_opt_idx));
        }
    }
    Ok(())
}

#[tauri::command]
fn add_replace_option(replace_opt: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    if replace_opt.trim().is_empty() { return Ok(()); }
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let replace_options = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().replace_options;
        if replace_options.iter().any(|w| w == &replace_opt) {
            proj_state.selected_opt_idx = Some(replace_options.iter().position(|w| w == &replace_opt).unwrap() as i32);
            let _ = app.emit("sync_app_selection_state", (proj_state.selected_rule_idx, proj_state.selected_term_idx, proj_state.selected_opt_idx));
        } else {
            replace_options.push(replace_opt);
            proj_state.selected_opt_idx = Some(replace_options.len() as i32 - 1);
            let _ = app.emit("sync_app_state", proj_state.clone());
        }
    }
    Ok(())
}

#[tauri::command]
fn remove_replace_option(opt_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let replace_options = &proj_state.rules.get(rule_index as usize).unwrap().replace_options;
        if opt_index >= 0 && opt_index < replace_options.len() as i32 {
            proj_state.rules.get_mut(rule_index as usize).unwrap().replace_options.remove(opt_index as usize);
            let replace_options = &proj_state.rules.get(rule_index as usize).unwrap().replace_options;
            if let Some(selected_opt) = proj_state.selected_opt_idx {
                if selected_opt as usize >= replace_options.len() {
                    proj_state.selected_opt_idx = if replace_options.is_empty() { None } else { Some(replace_options.len() as i32 - 1) };
                }
            }
            let _ = app.emit("sync_app_state", proj_state.clone());
        }
    }
    Ok(())
}

#[tauri::command]
fn rename_replace_option(opt_index: i32, new_opt: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    if new_opt.trim().is_empty() { return Ok(()); }
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let replace_options = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().replace_options;
        if opt_index >= 0 && opt_index < replace_options.len() as i32 && !replace_options.iter().any(|w| w == &new_opt) {
            replace_options[opt_index as usize] = new_opt;
            let _ = app.emit("sync_app_state", proj_state.clone());
        }
    }
    Ok(())
}

#[tauri::command]
fn select_replace_option(opt_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if let Some(rule_index) = proj_state.selected_rule_idx {
        let replace_options = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().replace_options;
        let new_val = if opt_index >= 0 && opt_index < replace_options.len() as i32 { Some(opt_index) } else { None };
        if proj_state.selected_opt_idx != new_val {
            proj_state.selected_opt_idx = new_val;
            let _ = app.emit("sync_app_selection_state", (proj_state.selected_rule_idx, proj_state.selected_term_idx, proj_state.selected_opt_idx));
        }
    }
    Ok(())
}


#[tauri::command]
fn select_item(item_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut session = state.lock().unwrap();
    let new_val = if item_index >= 0 && item_index < session.items.len() as i32 { Some(item_index) } else { None };
    if session.selected_item != new_val {
        session.selected_item = new_val;
        let _ = app.emit("sync_item_selection_state", (session.selected_item, session.selected_item.and_then(|i| Some(session.selected_mark[i as usize]))));
    }
    Ok(())
}

#[tauri::command]
fn next_item(app: tauri::AppHandle, state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut session = state.lock().unwrap();
    let new_val = if session.selected_item < Some(session.items.len() as i32 - 1) { Some(session.selected_item.unwrap() + 1) } else { session.selected_item };
    if session.selected_item != new_val {
        session.selected_item = new_val;
        let _ = app.emit("sync_item_selection_state", (session.selected_item, session.selected_item.and_then(|i| Some(session.selected_mark[i as usize]))));
    }
    Ok(())
}

#[tauri::command]
fn prev_item(app: tauri::AppHandle, state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut session = state.lock().unwrap();
    let new_val = if session.selected_item > Some(0) { Some(session.selected_item.unwrap() - 1) } else { session.selected_item };
    if session.selected_item != new_val {
        session.selected_item = new_val;
        let _ = app.emit("sync_item_selection_state", (session.selected_item, session.selected_item.and_then(|i| Some(session.selected_mark[i as usize]))));
    }
    Ok(())
}

#[tauri::command]
fn select_mark(mark_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut session = state.lock().unwrap();
    if let Some(item_index) = session.selected_item {
        let new_val = if mark_index >= 0 && mark_index < session.items[item_index as usize].found_mark_idxs.len() as i32 { Some(mark_index) } else { None };
        if session.selected_mark[item_index as usize] != new_val {
            session.selected_mark[item_index as usize] = new_val;
            let _ = app.emit("sync_item_selection_state", (session.selected_item, session.selected_mark[item_index as usize]));
        }
    }
    Ok(())
}

#[tauri::command]
fn next_mark(app: tauri::AppHandle, state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut session = state.lock().unwrap();
    if let Some(item_index) = session.selected_item {
        let found_mark_idxs = &session.items[item_index as usize].found_mark_idxs;
        if let Some(mark_index) = session.selected_mark[item_index as usize] {
            if mark_index < found_mark_idxs.len() as i32 - 1 {
                session.selected_mark[item_index as usize] = Some(mark_index + 1);
                let _ = app.emit("sync_item_selection_state", (session.selected_item, mark_index + 1));
            } else if item_index < session.items.len() as i32 - 1 && mark_index == found_mark_idxs.len() as i32 - 1 {
                session.selected_item = Some(item_index + 1);
                session.selected_mark[(item_index + 1) as usize] = Some(0);
                let _ = app.emit("sync_item_selection_state", (item_index + 1, 0));
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn prev_mark(app: tauri::AppHandle, state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut session = state.lock().unwrap();
    if let Some(item_index) = session.selected_item {
        if let Some(mark_index) = session.selected_mark[item_index as usize] {
            if mark_index > 0 {
                session.selected_mark[item_index as usize] = Some(mark_index - 1);
                let _ = app.emit("sync_item_selection_state", (session.selected_item, mark_index - 1));
            } else if item_index > 0 && mark_index == 0 {
                session.selected_item = Some(item_index - 1);
                session.selected_mark[(item_index - 1) as usize] = Some(session.items[(item_index - 1) as usize].found_mark_idxs.len() as i32 - 1);
                let _ = app.emit("sync_item_selection_state", (item_index - 1, session.selected_mark[session.selected_item.unwrap() as usize]));
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn choose_a_replace_option(opt_index: i32, state: State<'_, Mutex<SessionItems>>) -> Result<Option<(Vec<Option<i32>>, bool)>, String> {
    let mut session = state.lock().unwrap();
    if let Some(item_index) = session.selected_item {
        if let Some(mark_index) = session.selected_mark[item_index as usize] {
            let item = session.items.get_mut(item_index as usize).unwrap();
            let new_val = if opt_index > -1 { Some(opt_index) } else { None };
            if item.selected_options[mark_index as usize] != new_val {
                item.selected_options[mark_index as usize] = new_val;
                item.dirty = item.selected_options.iter().any(|x| x.is_some());
                return Ok(Some((item.selected_options.clone(), item.dirty)));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
fn get_config_state(state: State<'_, Mutex<AppProjectState>>) -> Result<AppProjectState, String> {
    Ok(state.lock().unwrap().clone())
}
#[tauri::command]
fn get_session_items(state: State<'_, Mutex<SessionItems>>) -> Result<SessionItems, String> {
    Ok(state.lock().unwrap().clone())
}

#[tauri::command]
fn get_app_settings(state: State<'_, Mutex<AppSettings>>) -> Result<AppSettings, String> {
    Ok(state.lock().unwrap().clone())
}

#[tauri::command]
fn open_folder(target: &str, app_handle: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let folder_path = app_handle.dialog()
        .file()
        .set_directory("G:\\Diffsinger\\datasets\\en\\AlemonEN_raw\\TextGrid")
        .blocking_pick_folder();
    let mut proj_state = state.lock().unwrap();
    match folder_path {
        Some(path) => {
            match target {
                "tg" => {
                    proj_state.tg_folder = Some(path.clone());
                    if proj_state.wav_folder == None {
                        if let Some(parent_path) = path.clone().parent() {
                            proj_state.wav_folder = Some(parent_path.join("wav"));
                        }
                    }
                },
                "wav" => {
                    proj_state.wav_folder = Some(path.clone());
                    if proj_state.tg_folder == None {
                        if let Some(parent_path) = path.clone().parent() {
                            proj_state.tg_folder = Some(parent_path.join("TextGrid"));
                        }
                    }
                },
                _ => return Err("target can only be either tg or wav.".into()),
            }
            let _ = app_handle.emit("sync_folder_state", (proj_state.tg_folder.clone(), proj_state.wav_folder.clone()));
        },
        None => return Err("User did not select a folder.".into()),
    }
    Ok(())
}

#[tauri::command]
fn list_items(app: tauri::AppHandle, project_state: State<'_, Mutex<AppProjectState>>, session_state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let proj_state = project_state.lock().unwrap();
    let mut sess_state = session_state.lock().unwrap();
    if proj_state.tg_folder == None {
        return Err("TextGrid folder must be set.".into());
    }
    match proj_state.selected_rule_idx {
        Some(idx) => {
            if idx >= proj_state.rules.len() as i32 {
                return Err("Internal error: selected rule index out of range.".into());
            }
        },
        None => return Err("Must select a rule first.".into()),
    }
    sess_state.items = Vec::new();
    sess_state.selected_item = None;
    sess_state.selected_mark = Vec::new();
    let tg_folder = proj_state.tg_folder.clone().unwrap();
    let wav_folder = &proj_state.wav_folder;
    let active_rule = proj_state.rules.get(proj_state.selected_rule_idx.unwrap() as usize).unwrap();
    match fs::read_dir(tg_folder) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext.to_lowercase()) == Some("textgrid".into()) {
                            let tg = parse_textgrid(path.clone())?;
                            let (found_mark_idxs, found_mark_titles) = find_marks(active_rule, &tg);
                            if found_mark_idxs.len() > 0 {
                                let item_record = ItemRecord {
                                    tg_file: path.clone(),
                                    tg_stem: path.file_stem().unwrap().to_str().unwrap().to_string(),
                                    tg_content: tg,
                                    wav_file: if let Some(wav_folder_unwrap) = wav_folder {
                                        path.clone().with_extension("wav")
                                            .file_name()
                                            .and_then(
                                                |wav_fn| Some(wav_folder_unwrap.join(wav_fn))
                                            )
                                    } else { None },
                                    selected_options: vec![None; found_mark_idxs.len()],
                                    replace_options: active_rule.replace_options.clone(),
                                    found_mark_idxs,
                                    found_mark_titles,
                                    dirty: false,
                                };
                                sess_state.items.push(item_record);
                                sess_state.selected_item = Some(0);
                                sess_state.selected_mark.push(Some(0));
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            let _ = app.emit("sync_session_state", sess_state.clone());
            Ok(())
        },
        Err(_) => Err("Failed to read the directory".into()),
    }
}

#[tauri::command]
fn save_textgrids(app_handle: tauri::AppHandle, state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut sess_state = state.lock().unwrap();
    let mut resync = false;
    for item in sess_state.items.iter_mut() {
        if item.dirty {
            let opts = &item.replace_options;
            // let _ = item.tg_file.clone().with_extension("textgrid").write_to(&item.tg_content);
            let mut new_tg = item.tg_content.clone();
            for (i, mark_idx) in item.found_mark_idxs.iter().enumerate() {
                if let Some(opt_idx) = item.selected_options[i] {
                    new_tg.items.get_mut(1).unwrap().intervals.get_mut(*mark_idx).unwrap().text = opts[opt_idx as usize].clone();
                }
            }
            let bak_path = item.tg_file.clone().with_extension("TextGrid.bak");
            if !bak_path.exists() {
                fs::copy(&item.tg_file, &bak_path).map_err(|e| e.to_string())?;
            }
            let _ = fs::write(item.tg_file.clone(), textgrid_to_string(&new_tg)).map_err(|e| e.to_string()).and_then(|_| { item.dirty = false; resync = true; Ok(()) });
        }
    }
    if resync {
        let _ = app_handle.emit("sync_session_state", sess_state.clone());
    }
    Ok(())
}

#[tauri::command]
fn init_state(app_handle: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>, session_state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();
    let mut sess_state = session_state.lock().unwrap();
    app_state.clone_from(&AppProjectState::default());
    sess_state.clone_from(&SessionItems::default());
    let _ = app_handle.emit("sync_app_state", app_state.clone());
    let _ = app_handle.emit("sync_session_state", sess_state.clone());
    Ok(())
}

#[tauri::command]
fn save_state(app_handle: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>, _session_state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let file_path = app_handle.dialog()
        .file()
        .add_filter("Replacer Project", &["tgrep"])
        .blocking_save_file();
    match file_path {
        Some(path) => {
            let app_state = state.lock().unwrap();
            // let session_state = session_state.lock().unwrap();
            let app_state_json = serde_json::to_string(&*app_state).map_err(|e| e.to_string())?;
            // let session_state_json = serde_json::to_string(&*session_state).map_err(|e| e.to_string())?;
            // fs::write(path, format!("{{\"app_state\": {}, \"session_state\": {}}}", app_state_json, session_state_json)).map_err(|e| e.to_string())?;
            fs::write(path, format!("{{\"app_state\": {}}}", app_state_json)).map_err(|e| e.to_string())?;
            Ok(())
        },
        None => return Err("User did not select a file.".into()),
    }
}

#[tauri::command]
fn load_state(app_handle: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>, session_state: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let file_path = app_handle.dialog()
        .file()
        .add_filter("Replacer Project", &["tgrep"])
        .blocking_pick_file();
    match file_path {
        Some(pathres) => {
            let file_contents = fs::read_to_string(&pathres.path).map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&file_contents).map_err(|e| e.to_string())?;
            let app_state_json = json["app_state"].to_string();
            // let session_state_json = json["session_state"].to_string();
            let mut app_state = state.lock().unwrap();
            let session_state = session_state.lock().unwrap();
            *app_state = serde_json::from_str(&app_state_json).map_err(|e| e.to_string())?;
            // *session_state = serde_json::from_str(&session_state_json).map_err(|e| e.to_string())?;
            let _ = app_handle.emit("sync_app_state", app_state.clone());
            let _ = app_handle.emit("sync_session_state", session_state.clone());
            Ok(())
        },
        None => return Err("User did not select a file.".into()),
    }
}

#[tauri::command]
fn play_selected(session_state: State<'_, Mutex<SessionItems>>, sound_state: State<'_, Mutex<AppSoundEngine>>, sink: State<'_, Arc<Mutex<Sink>>>) -> Result<(), String> {
    let session_state = session_state.lock().unwrap();
    let mut sound_engine = sound_state.lock().unwrap();

    if let Some(item_index) = session_state.selected_item {
        if let Some(mark_index) = session_state.selected_mark[item_index as usize] {
            let item = &session_state.items[item_index as usize];
            if let Some(wav_file) = &item.wav_file {
                if sound_engine.decoded_file.as_path() != wav_file.as_path() {
                    sound_engine.decoded_file = item.wav_file.clone().unwrap();
                    sound_engine.source = Some(Sound::load(item.wav_file.as_ref().unwrap()).map_err(|e| e.to_string())?);
                }
                if let Some(sound) = &sound_engine.source {
                    let tg = &item.tg_content;
                    let phones = &tg.items[1].intervals[item.found_mark_idxs[mark_index as usize]];

                    // Open the audio file
                    let source = fadeout(
                        sound.decoder()
                            .skip_duration(Duration::from_millis(max(0, (phones.xmin * 1000. - 300.) as u64)))
                            .take_duration(Duration::from_millis((300. + 600. + (phones.xmax - phones.xmin) * 1000.) as u64))
                            .fade_in(Duration::from_millis(50)),
                        Duration::from_millis(50),
                        Some(Duration::from_millis((300. + 600. + (phones.xmax - phones.xmin) * 1000.) as u64))
                    );

                    let sink_clone = Arc::clone(&sink);
                    let sink = sink_clone.lock().unwrap();
                    sink.clear();
                    sink.append(source);
                    sink.play();
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn list_audio_output_devices() -> Result<(String, Vec<String>), String> {
    let host = cpal::default_host();
    let devices = host.output_devices().map_err(|e| e.to_string())?;
    let default_device = host.default_output_device().unwrap();
    let mut res = Vec::new();
    for device in devices {
        res.push(device.name().map_err(|e| e.to_string())?);
    }
    Ok((default_device.name().map_err(|e| e.to_string())?, res))
}

#[tauri::command]
fn select_audio_output_device(device_name: String, app: tauri::AppHandle, sink: State<'_, Arc<Mutex<Sink>>>, stream_handle: State<'_, Arc<Mutex<(OutputStream, OutputStreamHandle)>>>, app_settings: State<'_, Mutex<AppSettings>>) -> Result<(), String> {
    let device = cpal::default_host().output_devices().map_err(|e| e.to_string())?.find(|d| d.name().unwrap() == device_name).unwrap();
    // let (_stream, stream_handle) =
    let sink_clone = Arc::clone(&sink);
    let stream_handle_clone = Arc::clone(&stream_handle);
    let mut sink = sink_clone.lock().unwrap();
    let mut stream_handle = stream_handle_clone.lock().unwrap();
    *stream_handle = OutputStream::try_from_device(&device).map_err(|e| e.to_string())?;
    *sink = Sink::try_new(&(Arc::clone(&stream_handle_clone).lock().unwrap().1)).map_err(|e| e.to_string()).unwrap();

    let mut app_settings = app_settings.lock().unwrap();
    app_settings.sound_device = Some(device_name);
    let stores = app.state::<StoreCollection<Wry>>();
    let _ = with_store(app.clone(), stores, PathBuf::from("settings.json"), |store| {
        app_settings.save_to_store(store).unwrap();
        Ok(())
    });
    let _ = app.emit("sync_settings", app_settings.clone()).unwrap();
    Ok(())
}

#[tauri::command]
fn update_settings(theme: String, volume_factor: f32, auto_save: bool, auto_next: bool, app: tauri::AppHandle, app_settings: State<'_, Mutex<AppSettings>>) -> Result<(), String> {
    let mut app_settings = app_settings.lock().unwrap();
    app_settings.theme = theme.clone();
    app_settings.volume_factor = volume_factor;
    app_settings.auto_save = auto_save;
    app_settings.auto_next = auto_next;
    let stores = app.state::<StoreCollection<Wry>>();
    let _ = with_store(app.clone(), stores, PathBuf::from("settings.json"), |store| {
        app_settings.save_to_store(store).unwrap();
        Ok(())
    });
    let _ = app.emit("sync_settings", app_settings.clone()).unwrap();
    Ok(())
}

#[tauri::command]
fn test_output_device(sink: State<'_, Arc<Mutex<Sink>>>) -> Result<(), String> {
    let sink_clone = Arc::clone(&sink);
    let sink = sink_clone.lock().unwrap();
    let source = fadeout(
        SineWave::new(440.0).take_duration(Duration::from_secs_f32(0.9)).amplify(0.20).fade_in(Duration::from_secs_f32(0.2)),
        Duration::from_secs_f32(0.2),
        Some(Duration::from_secs_f32(0.9))
    );
    sink.clear();
    sink.append(source);
    sink.play();
    Ok(())
}

pub struct Sound (Arc<Vec<u8>>);

impl AsRef<[u8]> for Sound {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Sound {
    pub fn load(filename: &PathBuf) -> io::Result<Sound> {
        use std::fs::File;
        let mut buf = Vec::new();
        let mut file = File::open(filename)?;
        file.read_to_end(&mut buf)?;
        Ok(Sound(Arc::new(buf)))
    }
    pub fn cursor(self: &Self) -> io::Cursor<Sound> {
        io::Cursor::new(Sound(self.0.clone()))
    }
    pub fn decoder(self: &Self) -> rodio::Decoder<io::Cursor<Sound>> {
        Decoder::new(self.cursor()).unwrap()
    }
}

#[derive(Default)]
struct AppSoundEngine {
    decoded_file: PathBuf,
    source: Option<Sound>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // let (_stream, stream_handle) = OutputStream::try_default().map_err(|e| e.to_string()).unwrap();
    let stream_handle = Arc::new(Mutex::new(OutputStream::try_default().map_err(|e| e.to_string()).unwrap()));
    let sink = Arc::new(Mutex::new(Sink::try_new(&(Arc::clone(&stream_handle).lock().unwrap().1)).map_err(|e| e.to_string()).unwrap()));
    let stream_handle_clone = Arc::clone(&stream_handle);
    let sink_clone = Arc::clone(&sink);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            println!("{}, {argv:?}, {cwd}", app.package_info().name);
            app.emit("single-instance", Payload { args: argv, cwd }).unwrap();
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let mut store = StoreBuilder::new("settings.json").build(app.handle().clone());
            let _ = store.load();
            let app_settings = AppSettings::load_from_store(&store);
            match app_settings {
                Ok(app_settings) => {
                    match app_settings.sound_device {
                        None => (),
                        Some(ref device_name) => {
                            // let device = cpal::default_host().output_devices().map_err(|e| e.to_string())?.find(|d| d.name().unwrap() == *device_name).unwrap();
                            // let mut stream_handle = stream_handle_clone.lock().unwrap();
                            // let mut sink = sink_clone.lock().unwrap();
                            // *stream_handle = OutputStream::try_from_device(&device).map_err(|e| e.to_string())?;
                            // *sink = Sink::try_new(&(Arc::clone(&stream_handle_clone).lock().unwrap().1)).map_err(|e| e.to_string()).unwrap();
                        },
                    };
                    app.manage(Mutex::new(app_settings));
                    Ok(())
                },
                Err(err) => {
                    eprintln!("Error loading settings: {}", err);
                    Err(err)
                }
            }
        })
        .manage(Mutex::new(AppProjectState::default()))
        .manage(Mutex::new(SessionItems::default()))
        .manage(Mutex::new(AppSoundEngine::default()))
        .manage(Arc::clone(&sink))
        .manage(Arc::clone(&stream_handle))
        .invoke_handler(tauri::generate_handler![
            init_state, load_state, save_state, save_textgrids,
            get_config_state, get_session_items, get_app_settings,
            open_folder, list_items, play_selected,
            add_rule, rename_rule, remove_rule, select_rule,
            add_search_term, remove_search_term, rename_search_term, select_search_term,
            add_replace_option, remove_replace_option, rename_replace_option, select_replace_option,
            prev_item, next_item, prev_mark, next_mark, select_item, select_mark, choose_a_replace_option,
            list_audio_output_devices, select_audio_output_device, test_output_device, update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
