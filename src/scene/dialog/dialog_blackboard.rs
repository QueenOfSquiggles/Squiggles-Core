use core::fmt;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fmt::Display,
    hash::{Hash, Hasher},
};

use godot::{engine::Json, prelude::*};

#[derive(Debug, PartialEq, Clone)]
enum Entry {
    Number(f32),
    String(String),
    Bool(bool),
    None,
}
impl Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entry::Number(val) => f.write_fmt(format_args!("{:.2}", val)),
            Entry::String(val) => f.write_fmt(format_args!("{}", val)),
            Entry::Bool(val) => f.write_fmt(format_args!("{}", val)),
            Entry::None => f.write_str("nil"),
        }
    }
}

#[derive(Default)]
pub struct Blackboard {
    entries: HashMap<String, Entry>,
}

impl Blackboard {
    const RECOGNIZED_COMMANDS: [&'static str; 3] = ["set", "add", "sub"];
    /// Parses the action string
    pub fn parse_action(&mut self, code: String) {
        // godot_print!("Running action(s): {}", code);
        for action in code.split(';') {
            // godot_print!("Running sub-action: {}", action);
            let parts = Vec::from_iter(action.trim().splitn(3, ' '));
            if parts.len() != 3 {
                godot_warn!("Improperly formed dialog code \"{}\". Ignoring", action);
                continue;
            }
            let (command, key, value) = (parts[0].trim(), parts[1].trim(), parts[2].trim());
            if !Self::RECOGNIZED_COMMANDS.contains(&command) {
                godot_warn!(
                    "Unrecognized command! \"{}\" in line \"{}\"",
                    command,
                    action
                );
                continue;
            }
            match command {
				"set" => self.set(key, value),
                "add" => self.add(key, value),
                "sub" => self.sub(key, value),
                _ => unreachable!("If you're seeing this, you forgot to add a new command to the match statement. But I still love you XOXO"),
            }
        }
    }

    pub fn parse_query(&mut self, code: String) -> bool {
        // godot_print!("Running quer(y/ies): {}", code);
        for query in code.split("and") {
            let mut chunk_val = false;
            for options in query.split("or") {
                // godot_print!("Running sub-query: {}", options);

                let parts = Vec::from_iter(options.split_whitespace());
                if parts.len() != 3 {
                    if query.contains('\"') {
                        // TODO if someone want's to make this support space strings, go right ahead, I'll accept the PR. But I'm not writing it myself lol
                        godot_error!("Strings with spaces are not supported for queries! Only used for storage!")
                    }
                    godot_warn!("Malformed query {}, in code {}", options, code);
                    continue;
                }
                chunk_val = chunk_val || self.parse_query_value((parts[0], parts[1], parts[2]));
            }
            if !chunk_val {
                return false;
            }
        }
        true
    }

    fn parse_query_value(&mut self, query: (&str, &str, &str)) -> bool {
        let arg1 = self.get_numeric_value(query.0);
        let arg2 = self.get_numeric_value(query.2);
        // godot_print!("Running internal comparison: {} {} {}", arg1, query.1, arg2);
        match query.1 {
            "==" => arg1 == arg2,
            "!=" => arg1 != arg2,
            ">=" => arg1 >= arg2,
            "<=" => arg1 <= arg2,
            ">" => arg1 > arg2,
            "<" => arg1 < arg2,
            _ => {
                godot_warn!(
                    "Unrecognized operator in query: {} {} {}",
                    query.0,
                    query.1,
                    query.2
                );
                false
            }
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let entry = Self::get_entry_for(value);
        if entry == Entry::None {
            godot_warn!("Failed to find valid entry for setting to \"{}\"", key);
            return;
        };
        self.entries.insert(key.to_string(), entry.clone());
        // godot_print!("Set value: {} = {}. Enum value: {}", key, value, entry);
    }

    pub fn add(&mut self, key: &str, value: &str) {
        if !self.entries.contains_key(&key.to_string()) {
            godot_warn!("Cannot add to \"\"! Does not exist yet!");
            return;
        }
        let entry = Self::get_entry_for(value);
        if entry == Entry::None {
            godot_warn!("Failed to find valid entry for setting to \"{}\"", key);
            return;
        };
        let Some(prev) = self.entries.get(&key.to_string()) else {
            unreachable!()
        };
        let nval = match prev {
            Entry::Number(val) => match entry {
                Entry::Number(entry_val) => Entry::Number(*val + entry_val),
                Entry::String(_) => Entry::None,
                Entry::Bool(entry_val) => Entry::Number(
                    *val + match entry_val {
                        true => 1f32,
                        false => 0f32,
                    },
                ),
                Entry::None => Entry::Number(*val),
            },
            Entry::String(val) => match entry {
                Entry::Number(entry_val) => {
                    Entry::String(val.clone() + entry_val.to_string().as_str())
                }
                Entry::String(entry_val) => Entry::String(val.clone() + entry_val.as_str()),
                Entry::Bool(entry_val) => {
                    Entry::String(val.clone() + entry_val.to_string().as_str())
                }
                Entry::None => Entry::String(val.clone()),
            },
            _ => Entry::None,
        };
        self.entries.insert(key.to_string(), nval);
    }
    pub fn sub(&mut self, key: &str, value: &str) {
        if !self.entries.contains_key(&key.to_string()) {
            godot_warn!("Cannot add to \"\"! Does not exist yet!");
            return;
        }
        let entry = Self::get_entry_for(value);
        if entry == Entry::None {
            godot_warn!("Failed to find valid entry for setting to \"{}\"", key);
            return;
        };
        let Some(prev) = self.entries.get(&key.to_string()) else {
            unreachable!()
        };
        let nval = match prev {
            Entry::Number(val) => match entry {
                Entry::Number(entry_val) => Entry::Number(*val - entry_val),
                Entry::String(_) => Entry::None,
                Entry::Bool(entry_val) => Entry::Number(
                    *val - match entry_val {
                        true => 1f32,
                        false => 0f32,
                    },
                ),
                Entry::None => Entry::Number(*val),
            },
            _ => Entry::None,
        };
        self.entries.insert(key.to_string(), nval);
    }

    fn get_entry_for(value: &str) -> Entry {
        let var = Json::parse_string(value.to_godot());
        match var.get_type() {
            VariantType::Nil => {
                godot_warn!("Failed to parse \"{}\" into a handled type!", value,);
                Entry::None
            }
            VariantType::Bool => Entry::Bool(var.booleanize()),
            VariantType::Int => Entry::Number(i32::from_variant(&var) as f32),
            VariantType::Float => Entry::Number(f32::from_variant(&var)),
            VariantType::String => Entry::String(String::from_variant(&var)),
            _ => {
                godot_warn!("Fail! \"{}\" is not a handled type!", value,);
                Entry::None
            }
        }
    }
    fn get_numeric_value(&self, key: &str) -> i32 {
        // load entry
        let entry: Entry = if self.entries.contains_key(&key.to_string()) {
            // from variable name
            self.entries.get(&key.to_string()).unwrap().clone() // we should be safe to unwrap here???
        } else {
            // from constant
            let var = Json::parse_string(key.to_godot());
            match var.get_type() {
                VariantType::Float => Entry::Number(f32::from_variant(&var)),
                VariantType::Int => Entry::Number(i32::from_variant(&var) as f32),
                VariantType::Bool => Entry::Bool(bool::from_variant(&var)),
                VariantType::String => Entry::String(String::from_variant(&var)),
                _ => Entry::None,
            }
        };
        match entry {
            Entry::Number(val) => f32::floor(val * 100f32) as i32, // this does force accuracy to only 0.01, but then again, this system is not designed for
            Entry::String(val) => {
                let mut s = DefaultHasher::new();
                val.hash(&mut s);
                s.finish() as i32
            }
            Entry::Bool(val) => match val {
                true => 1,
                false => 0,
            },
            Entry::None => i32::MIN,
        }
    }

    pub fn get_variant_entry(&self, key: &str) -> Variant {
        let Some(entry) = self.entries.get(key) else {
            godot_warn!("Entry not found \"{}\", returning nil", key);
            return Variant::nil();
        };
        match entry {
            Entry::Number(val) => val.to_variant(),
            Entry::String(val) => val.to_variant(),
            Entry::Bool(val) => val.to_variant(),
            Entry::None => Variant::nil(),
        }
    }

    pub fn debug_print(&self) {
        let mappings: Vec<String> = self
            .entries
            .iter()
            .map(|pair| format!("{} = {}, ", pair.0, pair.1))
            .collect();
        let mut buffer: String = "Blackboard { ".into();
        for e in mappings {
            buffer += e.as_str();
            buffer += "\n";
        }
        buffer += " }";
        // godot_print!("{}", buffer);
    }
}

impl fmt::Debug for Blackboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.entries.iter()).finish()
    }
}
