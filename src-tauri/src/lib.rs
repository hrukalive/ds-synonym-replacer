use std::cmp::{max, min};
use std::collections::HashSet;
use std::default::Default;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex};
use pest::Parser;
use pest_derive::Parser;
use chardetng::EncodingDetector;
use rodio::{Decoder, OutputStream, Sink, Source, Sample};
use std::time::Duration;


#[derive(Clone, serde::Serialize)]
struct Payload {
    args: Vec<String>,
    cwd: String,
}

#[derive(Parser)]
#[grammar = "textgrid.pest"]
struct TextGridParser;

#[derive(Clone, Default, serde::Serialize)]
struct TextGridInterval {
    xmin: f32,
    xmax: f32,
    text: String,
}

#[derive(Clone, Default, serde::Serialize)]
struct TextGridIntervals {
    name: String,
    intervals: Vec<TextGridInterval>,
}

#[derive(Clone, Default, serde::Serialize)]
struct TextGrid {
    items: Vec<TextGridIntervals>,
}

impl TextGrid {
    fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str("File type = \"ooTextFile\"\n");
        result.push_str("Object class = \"TextGrid\"\n");
        result.push_str("xmin = 0\n");
        result.push_str(&format!("xmax = {}\n", self.items.iter().map(|item| item.intervals.iter().map(|interval| interval.xmax).fold(0.0/0.0, f32::max)).fold(0.0/0.0, f32::max)));
        result.push_str("tiers? <exists>\n");
        result.push_str(&format!("size = {}\n", self.items.len()));
        result.push_str("item []:\n");

        for (i, item) in self.items.iter().enumerate() {
            result.push_str(&format!("    item [{}]:\n", i + 1));
            result.push_str(&format!("        class = \"IntervalTier\"\n"));
            result.push_str(&format!("        name = \"{}\"\n", item.name));
            result.push_str(&format!("        xmin = 0\n"));
            result.push_str(&format!("        xmax = {}\n", item.intervals.iter().map(|interval| interval.xmax).fold(0.0/0.0, f32::max)));
            result.push_str(&format!("        intervals: size = {}\n", item.intervals.len()));

            for (j, interval) in item.intervals.iter().enumerate() {
                result.push_str(&format!("        intervals [{}]:\n", j + 1));
                result.push_str(&format!("            xmin = {}\n", interval.xmin));
                result.push_str(&format!("            xmax = {}\n", interval.xmax));
                result.push_str(&format!("            text = \"{}\"\n", interval.text));
            }
        }

        result
    }
}

#[derive(Clone, Default, serde::Serialize)]
struct ReplaceRule {
    rule_name: String,
    search_terms: Vec<String>,
    replace_options: Vec<String>,
}

#[derive(Clone, Default, serde::Serialize)]
struct AppProjectState {
    tg_folder: Option<PathBuf>,
    wav_folder: Option<PathBuf>,
    rules: Vec<ReplaceRule>,
    active_rule: Option<i32>,
    selected_word: Option<i32>,
    selected_replacement: Option<i32>,
}

#[derive(Clone, serde::Serialize)]
struct ItemRecord {
    tg_file: PathBuf,
    tg_content: TextGrid,
    wav_file: Option<PathBuf>,
    found_tiers: Vec<usize>,
    found_tiers_title: Vec<String>,
}

#[derive(Clone, Default, serde::Serialize)]
struct SessionItems {
    items: Vec<ItemRecord>
}

#[derive(Clone, Default, serde::Serialize)]
struct ItemSelectionState {
    selected_item: Option<i32>,
    selected_found: Option<i32>,
}

/// Internal function that builds a `FadeOut` object.
pub fn fadeout<I: Source>(input: I, duration: Duration) -> FadeOut<I> where <I as Iterator>::Item: rodio::Sample {
    let duration = duration.as_secs() * 1000000000 + duration.subsec_nanos() as u64;
    let input_dur = input.total_duration().expect("Cannot get input duration.");
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
pub struct FadeOut<I: Source> where <I as Iterator>::Item: rodio::Sample {
    input: I,
    current_ns: f32,
    start_fade: f32,
    total_ns: f32,
}

impl<I: Source> FadeOut<I> where <I as Iterator>::Item: rodio::Sample {
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

fn check_rule(rule: &ReplaceRule, tg: &TextGrid) -> (Vec<usize>, Vec<String>) {
    let mut found_tiers = Vec::new();
    let mut found_tiers_title = Vec::new();
    let find_word_set = rule.search_terms.iter().collect::<HashSet<_>>();
    let tg_intervals = &tg.items.last().unwrap().intervals;
    for (i, item) in tg_intervals.iter().enumerate() {
        if find_word_set.contains(&item.text) {
            found_tiers.push(i);
            let mut title = String::new();
            for j in min(0usize, i - 2) .. min(0usize, i - 1) {
                title.push_str((tg_intervals[j].text.clone() + " ").as_str());
            }
            title.push_str(("<b>".to_owned() + &tg_intervals[i].text.clone() + "</b> ").as_str());
            for j in max(i + 1, tg_intervals.len() - 1) .. max(i + 2, tg_intervals.len() - 1) {
                title.push_str((tg_intervals[j].text.clone() + " ").as_str());
            }
            found_tiers_title.push(title);
        }
    }
    (found_tiers, found_tiers_title)
}


#[tauri::command]
fn add_rule(rule_name: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if proj_state.rules.iter().any(|rule| rule.rule_name == rule_name) {
        let selected_rule = proj_state.rules.iter().position(|rule| rule.rule_name == rule_name).unwrap();
        if proj_state.active_rule != Some(selected_rule as i32) {
            proj_state.active_rule = Some(selected_rule as i32);
            proj_state.selected_word = None;
            proj_state.selected_replacement = None;
            let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
        }
    } else {
        proj_state.rules.push(ReplaceRule {
            rule_name,
            search_terms: Vec::new().into_iter().collect(),
            replace_options: Vec::new(),
        });
        proj_state.active_rule = Some((proj_state.rules.len() - 1) as i32);
        proj_state.selected_word = None;
        proj_state.selected_replacement = None;
        let _ = app.emit("sync_app_state", proj_state.clone());
    }
    Ok(())
}

#[tauri::command]
fn rename_rule(rule_index: i32, rule_name: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if proj_state.rules.iter().any(|rule| rule.rule_name == rule_name) {
        return Ok(());
    }
    if rule_index >= 0 && rule_index < proj_state.rules.len() as i32 {
        proj_state.rules.get_mut(rule_index as usize).unwrap().rule_name = rule_name;
        let _ = app.emit("sync_app_state", proj_state.clone());
    }
    Ok(())
}

#[tauri::command]
fn remove_rule(rule_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 {
        return Ok(());
    }
    proj_state.rules.remove(rule_index as usize);
    if let Some(active_rule) = proj_state.active_rule {
        if active_rule as usize >= proj_state.rules.len() {
            proj_state.active_rule = if proj_state.rules.is_empty() { None } else { Some((proj_state.rules.len() - 1) as i32) };
            proj_state.selected_word = None;
            proj_state.selected_replacement = None;
        }
    }
    let _ = app.emit("sync_app_state", proj_state.clone());
    Ok(())
}

#[tauri::command]
fn select_rule(rule_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 {
        proj_state.active_rule = None;
        proj_state.selected_word = None;
        proj_state.selected_replacement = None;
    } else if proj_state.active_rule == Some(rule_index) {
    } else {
        proj_state.active_rule = Some(rule_index);
        proj_state.selected_word = if proj_state.rules.get(rule_index as usize).unwrap().search_terms.is_empty() { None } else { Some(0) };
        proj_state.selected_replacement = if proj_state.rules.get(rule_index as usize).unwrap().replace_options.is_empty() { None } else { Some(0) };
    }
    let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
    Ok(())
}

#[tauri::command]
fn add_find_phoneme(rule_index: i32, word: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 {
        return Ok(());
    }
    if proj_state.active_rule != Some(rule_index) {
        proj_state.active_rule = Some(rule_index);
    }
    let mut words = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().search_terms;
    if words.iter().any(|w| w == &word) {
        proj_state.selected_word = Some(words.iter().position(|w| w == &word).unwrap() as i32);
        let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
        return Ok(());
    }
    words.push(word);
    proj_state.selected_word = Some(words.len() as i32 - 1);
    let _ = app.emit("sync_app_state", proj_state.clone());
    Ok(())
}

#[tauri::command]
fn remove_find_phoneme(rule_index: i32, word_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 || word_index < 0 || word_index >= proj_state.rules.get(rule_index as usize).unwrap().search_terms.len() as i32 {
        return Ok(());
    }
    if proj_state.active_rule != Some(rule_index) {
        proj_state.active_rule = Some(rule_index);
    }
    if proj_state.selected_word == Some(word_index) {
        let mut words = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().search_terms;
        words.remove(word_index as usize);
        if word_index >= words.len() as i32 {
            proj_state.selected_word = if words.is_empty() { None } else { Some(words.len() as i32 - 1) };
        }
        let _ = app.emit("sync_app_state", proj_state.clone());
    } else {
        let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
    }
    Ok(())
}

#[tauri::command]
fn rename_find_phoneme(rule_index: i32, word_index: i32, new_word: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 || word_index < 0 || word_index >= proj_state.rules.get(rule_index as usize).unwrap().search_terms.len() as i32 {
        return Ok(());
    }
    if proj_state.active_rule != Some(rule_index) {
        proj_state.active_rule = Some(rule_index);
    }
    let mut words = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().search_terms;
    words[word_index as usize] = new_word;
    let _ = app.emit("sync_app_state", proj_state.clone());
    Ok(())
}

#[tauri::command]
fn select_find_phoneme(rule_index: i32, word_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 || word_index < 0 || word_index >= proj_state.rules.get(rule_index as usize).unwrap().search_terms.len() as i32 {
        proj_state.selected_word = None;
    } else {
        if proj_state.active_rule != Some(rule_index) {
            proj_state.active_rule = Some(rule_index);
        }
        proj_state.selected_word = Some(word_index);
    }
    let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
    Ok(())
}

#[tauri::command]
fn add_replacement(rule_index: i32, replacement: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 {
        return Ok(());
    }
    if proj_state.active_rule != Some(rule_index) {
        proj_state.active_rule = Some(rule_index);
    }
    let mut replace_options = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().replace_options;
    if replace_options.iter().any(|w| w == &replacement) {
        proj_state.selected_replacement = Some(replace_options.iter().position(|w| w == &replacement).unwrap() as i32);
        let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
    } else {
        replace_options.push(replacement);
        let _ = app.emit("sync_app_state", proj_state.clone());
    }
    Ok(())
}

#[tauri::command]
fn remove_replacement(rule_index: i32, replacement_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 || replacement_index < 0 || replacement_index >= proj_state.rules.get(rule_index as usize).unwrap().replace_options.len() as i32 {
        return Ok(());
    }
    if proj_state.active_rule != Some(rule_index) {
        proj_state.active_rule = Some(rule_index);
    }
    if proj_state.selected_replacement == Some(replacement_index) {
        let mut replace_options = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().replace_options;
        replace_options.remove(replacement_index as usize);
        if replacement_index >= replace_options.len() as i32 {
            proj_state.selected_replacement = if replace_options.is_empty() { None } else { Some(replace_options.len() as i32 - 1) };
        }
        let _ = app.emit("sync_app_state", proj_state.clone());
    } else {
        let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
    }
    Ok(())
}

#[tauri::command]
fn rename_replacement(rule_index: i32, replacement_index: i32, new_replacement: String, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 || replacement_index < 0 || replacement_index >= proj_state.rules.get(rule_index as usize).unwrap().replace_options.len() as i32 {
        return Ok(());
    }
    if proj_state.active_rule != Some(rule_index) {
        proj_state.active_rule = Some(rule_index);
    }
    let mut replace_options = &mut proj_state.rules.get_mut(rule_index as usize).unwrap().replace_options;
    replace_options[replacement_index as usize] = new_replacement;
    let _ = app.emit("sync_app_state", proj_state.clone());
    Ok(())
}

#[tauri::command]
fn select_replacement(rule_index: i32, replacement_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let mut proj_state = state.lock().unwrap();
    if rule_index < 0 || rule_index >= proj_state.rules.len() as i32 || replacement_index < 0 || replacement_index >= proj_state.rules.get(rule_index as usize).unwrap().replace_options.len() as i32 {
        proj_state.selected_replacement = None;
    } else {
        if proj_state.active_rule != Some(rule_index) {
            proj_state.active_rule = Some(rule_index);
        }
        proj_state.selected_replacement = Some(replacement_index);
    }
    let _ = app.emit("sync_app_selection_state", (proj_state.active_rule, proj_state.selected_word, proj_state.selected_replacement));
    Ok(())
}

fn selection_changed(item_index: i32, found_index: i32) {}

#[tauri::command]
fn select_item(item_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<ItemSelectionState>>, session: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut selection_state = state.lock().unwrap();
    let session_items = session.lock().unwrap();
    if item_index < 0 || item_index >= session_items.items.len() as i32 {
        selection_state.selected_item = None;
    } else {
        selection_state.selected_item = Some(item_index);
    }
    let _ = app.emit("sync_item_selection_state", selection_state.clone());
    Ok(())
}

#[tauri::command]
fn select_found_tier(item_index: i32, found_index: i32, app: tauri::AppHandle, state: State<'_, Mutex<ItemSelectionState>>, session: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut selection_state = state.lock().unwrap();
    let session_items = session.lock().unwrap();
    if item_index < 0 || item_index >= session_items.items.len() as i32 || found_index < 0 || found_index >= session_items.items[item_index as usize].found_tiers.len() as i32 {
        selection_state.selected_found = None;
    } else {
        if selection_state.selected_item != Some(item_index) {
            selection_state.selected_item = Some(item_index);
        }
        selection_state.selected_found = Some(found_index);
    }
    let _ = app.emit("sync_item_selection_state", selection_state.clone());
    Ok(())
}

#[tauri::command]
fn next_found(app: tauri::AppHandle, state: State<'_, Mutex<ItemSelectionState>>, session: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut selection_state = state.lock().unwrap();
    let session_items = session.lock().unwrap();
    if selection_state.selected_item == None || selection_state.selected_found == None {
        return Ok(());
    }
    if selection_state.selected_found == Some(session_items.items[selection_state.selected_item.unwrap() as usize].found_tiers.len() as i32 - 1) && selection_state.selected_item < Some(session_items.items.len() as i32 - 1) {
        selection_state.selected_item = Some(selection_state.selected_item.unwrap() + 1);
        selection_state.selected_found = Some(0);
    } else if selection_state.selected_found < Some(session_items.items[selection_state.selected_item.unwrap() as usize].found_tiers.len() as i32 - 1) {
        selection_state.selected_found = Some(selection_state.selected_found.unwrap() + 1);
    }
    let _ = app.emit("sync_item_selection_state", selection_state.clone());
    Ok(())
}

#[tauri::command]
fn prev_found(app: tauri::AppHandle, state: State<'_, Mutex<ItemSelectionState>>, session: State<'_, Mutex<SessionItems>>) -> Result<(), String> {
    let mut selection_state = state.lock().unwrap();
    let session_items = session.lock().unwrap();
    if selection_state.selected_item == None || selection_state.selected_found == None {
        return Ok(());
    }
    if selection_state.selected_found == Some(0) && selection_state.selected_item > Some(0) {
        selection_state.selected_item = Some(selection_state.selected_item.unwrap() - 1);
        selection_state.selected_found = Some(session_items.items[selection_state.selected_item.unwrap() as usize].found_tiers.len() as i32 - 1);
    } else if selection_state.selected_found > Some(0) {
        selection_state.selected_found = Some(selection_state.selected_found.unwrap() - 1);
    }
    let _ = app.emit("sync_item_selection_state", selection_state.clone());
    Ok(())
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
fn open_folder(target: &str, app_handle: tauri::AppHandle, state: State<'_, Mutex<AppProjectState>>) -> Result<(), String> {
    let ans = app_handle.dialog()
        .file()
        .set_directory("G:\\Diffsinger\\datasets\\en\\AlemonEN_raw\\TextGrid")
        .blocking_pick_folder();
    let mut proj_state = state.lock().unwrap();
    match ans {
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
    let mut proj_state = project_state.lock().unwrap();
    let mut sess_state = session_state.lock().unwrap();
    if proj_state.tg_folder == None {
        return Err("TextGrid folder must be set.".into());
    }
    if proj_state.active_rule == None || proj_state.rules.len() <= proj_state.active_rule.unwrap() as usize {
        // return Err("Active rule must be set correctly.".into());
        proj_state.rules.push(ReplaceRule {
            rule_name: "test".into(),
            search_terms: vec!["d".into()].into_iter().collect(),
            replace_options: vec!["dx_d".into(), "dx_t".into()],
        });
        proj_state.active_rule = Some(0);
    }
    sess_state.items = Vec::new();
    let tg_folder = proj_state.tg_folder.clone().unwrap();
    let wav_folder = &proj_state.wav_folder;
    let active_rule = proj_state.rules.get(proj_state.active_rule.unwrap() as usize).unwrap();
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
                            let (found_tiers, found_tiers_title) = check_rule(active_rule, &tg);
                            if found_tiers.len() > 0 {
                                let item_record = ItemRecord {
                                    tg_file: path.clone(),
                                    tg_content: tg,
                                    wav_file: if let Some(wav_folder_unwrap) = wav_folder {
                                        path.clone().with_extension("wav")
                                            .file_name()
                                            .and_then(
                                                |wav_fn| Some(wav_folder_unwrap.join(wav_fn))
                                            )
                                    } else { None },
                                    found_tiers,
                                    found_tiers_title,
                                };
                                sess_state.items.push(item_record);
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            let _ = app.emit("sync_session_state", sess_state.items.clone());
            Ok(())
        },
        Err(_) => Err("Failed to read the directory".into()),
    }
}

#[tauri::command]
fn play_test(app_handle: tauri::AppHandle, sink_global: State<'_, Mutex<rodio::Sink>>) -> Result<(), String> {
    // Open the audio file
    let file = File::open("N:\\test.mp3").map_err(|e| e.to_string())?;
    let source = Decoder::new(BufReader::new(file.try_clone().unwrap())).map_err(|e| e.to_string())?;

    // Take the middle part of the file from 30s to 34s
    let source = source.skip_duration(Duration::from_secs(15)).take_duration(Duration::from_millis(4000 - 500));

    // Apply fade in and fade out
    let source = source.fade_in(Duration::from_millis(200));
    let source = fadeout(source, Duration::from_millis(200));

    // Play the sound
    let sink = sink_global.lock().unwrap();
    sink.clear();
    sink.append(source);
    sink.play();

    Ok(())
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (_stream, stream_handle) = OutputStream::try_default().map_err(|e| e.to_string()).unwrap();
    let sink = Sink::try_new(&stream_handle).map_err(|e| e.to_string()).unwrap();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            println!("{}, {argv:?}, {cwd}", app.package_info().name);
            app.emit("single-instance", Payload { args: argv, cwd }).unwrap();
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(Mutex::new(AppProjectState::default()))
        .manage(Mutex::new(SessionItems::default()))
        .manage(Mutex::new(ItemSelectionState::default()))
        .manage(Mutex::new(sink))
        .invoke_handler(tauri::generate_handler![
            get_config_state, get_session_items,
            open_folder, list_items, play_test,
            add_rule, rename_rule, remove_rule, select_rule,
            add_find_phoneme, remove_find_phoneme, rename_find_phoneme, select_find_phoneme,
            add_replacement, remove_replacement, rename_replacement, select_replacement,
            prev_found, next_found, select_item, select_found_tier
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
