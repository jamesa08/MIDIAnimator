// utils/mod.rs
pub mod gm_instrument_map;
pub mod ui;
pub mod animation;

use regex::Regex;
use std::f64::consts::PI;

use gm_instrument_map::GM_INST;

pub fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

pub fn note_to_name(n_val: i32) -> String {
    assert!(n_val >= 0 && n_val <= 127, "MIDI note number out of range!");

    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    return format!("{}{}", names[n_val as usize % 12], n_val / 12 - 2)
}

pub fn name_to_note(n_str: &str) -> i32 {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let mut note = names.iter().position(|&s| s == &n_str[..1]).unwrap() as i32;
    let offset = if n_str.len() > 1 && &n_str[1..2] == "#" {
        note += 1;
        2
    } else {
        1
    };
    return note + (n_str[offset..].parse::<i32>().unwrap() + 2) * 12;
}

pub fn convert_note_numbers(input_str: &str) -> Result<Vec<i32>, String> {
    let re_digit = Regex::new(r"^[0-9]+$").unwrap();
    let re_note = Regex::new(r"^[A-Ga-g]-?#?-?[0-8]+$").unwrap();

    if re_digit.is_match(input_str) {
        Ok(vec![input_str.parse::<i32>().unwrap()])
    } else if re_note.is_match(input_str) {
        Ok(vec![name_to_note(input_str)])
    } else if input_str.contains(',') {
        let nums: Vec<i32> = input_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| convert_note_numbers(s.trim()).unwrap()[0])
            .collect();
        Ok(nums)
    } else {
        Err(String::from("'{input_str}' has an invalid note number or name."))
    }
}

pub fn type_of_note_number(input_str: &str) -> Result<Vec<&str>, String> {
    let re_digit = Regex::new(r"^[0-9]+$").unwrap();
    let re_note = Regex::new(r"^[A-Ga-g]-?#?-?[0-8]+$").unwrap();

    if re_digit.is_match(input_str) {
        Ok(vec!["note"])
    } else if re_note.is_match(input_str) {
        Ok(vec!["name"])
    } else if input_str.contains(',') {
        let types: Vec<&str> = input_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| type_of_note_number(s.trim()).unwrap()[0])
            .collect();
        Ok(types)
    } else {
        Err(String::from("'{input_str}' has an invalid note number or name."))
    }
}

pub fn gm_program_to_name(pc_num: i32) -> String {
    assert!(pc_num >= 0 && pc_num <= 127, "Program change number out of range!");
    return GM_INST.get(&(pc_num)).unwrap().to_string()
}

pub fn closest_tempo(vals: &Vec<(f64, f64)>, t: f64, sort_list: bool) -> (f64, f64) {
    /*
    takes a list of tempos and times and returns the one closest to n

    :param list vals: list of values to compare to, List[(time, tempo)]
    :param float n: the time to compare to
    :param bool sortList: have the tempo list sorted, defaults to False
    :return tuple: returns the closest time value's tempo & time as a tuple, e.g. (2.50 sec, 500000 ticks)
     */
    let mut vals = vals.to_vec();

    if sort_list {
        vals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }

    if vals.len() == 1 {
        return vals[0];
    }

    // go through each tempo tuple and find the one that is closest to t
    for i in 0..vals.len() {
        if vals[i].0 <= t && t < vals[(i + 1) % vals.len()].0 {
            return vals[i];
        }
    }
    // return last tempo tuple
    return vals[vals.len() - 1]
}

pub fn remove_duplicates(vals: &[i32]) -> Vec<i32> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for &val in vals {
        if !seen.contains(&val) {
            seen.insert(val);
            result.push(val);
        }
    }

    result.sort();
    result
}

pub fn rotate_around_circle(radius: f64, angle: f64) -> (f64, f64) {
    let x = angle.cos() * radius;
    let y = angle.sin() * radius;
    (x, y)
}

pub fn animate_along_two_points(first_point: (f64, f64), second_point: (f64, f64), x_component: f64) -> (f64, f64) {
    let (x1, y1) = first_point;
    let (x2, y2) = second_point;

    let angle = 2.0 * ((y2 - y1) / (x2 - x1 + ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt())).atan();

    let x_cos = angle.cos() * x_component + x1;
    let y_sin = angle.sin() * x_component + y1;

    (x_cos, y_sin)
}

pub fn map_range_linear(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    out_min + ((value - in_min) / (in_max - in_min)) * (out_max - out_min)
}

pub fn map_range_sin(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    -((out_max - out_min) / 2.0) * ((PI * (in_min - value)) / (in_min - in_max)).cos() + ((out_max + out_min) / 2.0)
}

pub fn map_range_arc_sin(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    ((out_max - out_min) / PI) * ((2.0 / (in_max - in_min)) * (value - ((in_min + in_max) / 2.0))).asin() + ((out_max + out_min) / 2.0)
}

pub fn map_range_exp(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    let s = if out_min <= out_max { 1.0 } else { -1.0 };
    -s * (out_min - out_max - s).abs().powf((value - in_max) / (in_min - in_max)) + out_max + s
}

pub fn map_range_log(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    let p = if in_min <= in_max { 1.0 } else { -1.0 };
    ((out_max - out_min) * (value - in_min + p).abs().ln()) / (in_max - in_min + p).abs().ln() + out_min
}

pub fn map_range_para(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    (((out_min - out_max) * (value - in_max).powi(2)) / (in_max - in_min).powi(2)) + out_max
}

pub fn map_range_root(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    (((out_max - out_min) / (in_max - in_min).sqrt()) * (value - in_min).sqrt()) + out_min
}