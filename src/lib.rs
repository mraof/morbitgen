extern crate rand;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[cfg(any(target_arch = "wasm32", target_arch = "asmjs"))]
#[macro_use]
extern crate stdweb;
#[cfg(any(target_arch = "wasm32", target_arch = "asmjs"))]
extern crate yew;

#[cfg(any(target_arch = "wasm32", target_arch = "asmjs"))]
mod web;

mod serde_support;

use std::collections::{BTreeMap, HashMap};
use std::ops::AddAssign;
use std::str::FromStr;
use std::fmt::{Display, Formatter};
use rand::Rng;
use rand::distributions::{WeightedChoice, Weighted, Distribution};

type Denied = BTreeMap<String, Vec<String>>;
type Attributes = BTreeMap<String, Attribute>;
type Generated = HashMap<String, String>;

#[derive(Debug, Deserialize, Serialize)]
pub struct Template {
    pub order: Vec<String>,
    pub attributes: Attributes,
    #[serde(default)]
    pub rename: BTreeMap<String, String>,
    #[serde(default)]
    pub formatting: BTreeMap<String, String>,
}

#[derive(Debug, Default, Clone)]
pub struct Attribute {
    generator: Generator,
    ///completely replace parent attribute
    replace: bool,
    ///replace parent chances with this
    chance: Option<Chance>,
    ///requires for entire attribute
    requires: Vec<Requirement>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum Generator {
    ///Choose a key from the map using the value for the chance and requirements
    Choose(Attributes),
    ///Reuse generator for attribute
    Reuse(String),
    ///Copy result of generator for attribute
    Same(String),
    ///Don't choose anything
    Nothing,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
enum Chance {
    Never,
    ExtremelyRare,
    VeryRare,
    Rare,
    Uncommon,
    Standard,
    Common,
    VeryCommon,
    ExtremelyCommon,
    Always,
}

#[derive(Clone, Debug, Default)]
pub struct Requirement {
    pub possibilities: Vec<(String, String, bool)>,
}

impl FromStr for Requirement {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut possibilities = Vec::new();
        for possibility in s.split('|') {
            let mut split: Vec<&str> = possibility.split(':').collect();
            if split.len() == 1 {
                split.push("*");
            }
            let key = split[0].trim();
            let value = split[1].trim();
            possibilities.push(if key.starts_with('!') {
                (key[1..].to_string(), value.to_string(), true)
            } else {
                (key.to_string(), value.to_string(), false)
            });
        }
        Ok(Requirement { possibilities })
    }
}

impl Display for Requirement {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let mut count = 0;
        for &(ref key, ref value, ref not) in &self.possibilities {
            let prefix = if count > 0 { "|" } else { "" };
            let deny = if *not { "!" } else { "" };
            write!(f, "{}{}{}:{}", prefix, deny, key, value)?;
            count += 1;
        }
        Ok(())
    }
}

impl Default for Generator {
    fn default() -> Self {
        Generator::Nothing
    }
}

impl Default for Chance {
    fn default() -> Self {
        Chance::Standard
    }
}

impl Chance {
    pub fn chance(&self) -> u32 {
        use Chance::*;
        match *self {
            ExtremelyRare => 1,
            VeryRare => 3,
            Rare => 9,
            Uncommon => 18,
            Standard => 30,
            Common => 45,
            VeryCommon => 60,
            ExtremelyCommon => 100,
            _ => 0,
        }
    }
}

impl Template {
    pub fn new(name: &str, parent: Option<&Template>) -> Template {
        use std::fs::File;
        use std::io::Read;
        let mut json_file = File::open(&format!("assets/{}.json", name)).unwrap();
        let mut contents = String::new();
        json_file.read_to_string(&mut contents).expect(
            "Unable to read file",
        );
        Template::new_from_string(&contents, parent)
    }

    pub fn new_from_string(string: &str, parent: Option<&Template>) -> Template {
        let mut template: Template = serde_json::from_str(string).unwrap();
        if let Some(parent) = parent {
            let mut order = parent.order.clone();
            order.append(&mut template.order);
            template.order = order;
            let mut rename = parent.rename.clone();
            rename.append(&mut template.rename);
            template.rename = rename;
            for (name, parent_attribute) in parent.attributes.clone() {
                use std::collections::btree_map::Entry;
                let name = template.rename.get(&name).map_or(name, |name| name.clone());
                match template.attributes.entry(name) {
                    Entry::Vacant(v) => {
                        v.insert(parent_attribute);
                    }
                    Entry::Occupied(mut o) => {
                        *o.get_mut() += parent_attribute;
                    }
                }
            }
        }
        template
    }
    pub fn generate<I>(&self, presets: I) -> Generated
    where
        I: IntoIterator<Item = Requirement>,
    {
        let order = &self.order;
        let attributes = &self.attributes;
        let rename = &self.rename;
        let mut generated = Default::default();
        let mut denied = Default::default();
        add_requirements(
            &presets.into_iter().collect(),
            &mut generated,
            &mut denied,
            attributes,
        );

        for name in order {
            let name = rename.get(name).unwrap_or(name);
            if let Some(attribute) = attributes.get(name) {
                attribute.generate(name, &mut generated, &mut denied, attributes);
            } else {
                println!("{} doesn't exist", name);
            }
        }

        generated
    }
    
    pub fn format(&self, generated: &Generated, formatting: &str) -> Result<String, String> {
        if formatting == "json" {
            Ok(format!("{:#?}", generated))
        } else {
            let formatting: Formatting = self.formatting.get(formatting).unwrap_or(&formatting.to_string()).parse()?;
            Ok(formatting.format(generated))
        }
    }
    
    pub fn always(&self, name: &str, value: &str) -> bool {
        self.attributes.get(name).map_or(false, |attribute| attribute.generator.always(value, &self.attributes))
    }
}

impl Attribute {
    pub fn generate(
        &self,
        name: &str,
        generated: &mut Generated,
        denied: &mut Denied,
        attributes: &Attributes,
    ) {
        let mut valid = true;
        for requirement in &self.requires {
            valid &= meets_requirement(requirement, &generated, &denied);
        }
        if !valid {
            return;
        }

        self.generator.generate(name, generated, denied, attributes);
    }

    fn get_requirements(&self, name: &str, attributes: &Attributes) -> Vec<Requirement> {
        let mut requirements = self.requires.clone();
        requirements.append(&mut self.generator.get_requirements(name, attributes));
        requirements
    }
}

impl AddAssign for Attribute {
    fn add_assign(&mut self, rhs: Attribute) {
        use Generator::*;
        //The parent's just getting replaced, nothing is changed
        if self.replace {
            return;
        }
        let chance = &self.chance;

        self.requires.append(&mut rhs.requires.clone());
        match (&mut self.generator, rhs.generator) {
            (&mut Choose(ref mut choices), Choose(ref parent_choices)) => {
                for (key, parent_value) in parent_choices {
                    if choices.contains_key(key) {
                        let value = choices.get_mut(key).unwrap();
                        if !value.replace {
                            value.requires.append(&mut parent_value.requires.clone());
                        }
                    } else {
                        let mut value = parent_value.clone();
                        if let Some(chance) = self.chance {
                            value.chance = Some(chance);
                        }
                        choices.insert(key.clone(), value);
                    }
                }
            }
            (&mut Choose(ref mut choices), ref parent) => {
                let mut generator_name = "gen0".to_string();
                for i in 1.. {
                    if choices.contains_key(&generator_name) {
                        generator_name = format!("gen{}", i);
                    } else {
                        break;
                    }
                }
                choices.insert(
                    generator_name,
                    Attribute {
                        generator: parent.clone(),
                        chance: *chance,
                        ..Default::default()
                    },
                );
            }
            (&mut Nothing, Nothing) => {}
            (generator, parent) => {
                let mut choices = BTreeMap::new();
                choices.insert(
                    "gen0".to_string(),
                    Attribute {
                        generator: generator.clone(),
                        ..Default::default()
                    },
                );
                choices.insert(
                    "gen1".to_string(),
                    Attribute {
                        generator: parent,
                        chance: *chance,
                        ..Default::default()
                    },
                );
                *generator = Choose(choices);
            }
        }
    }
}

impl Generator {
    pub fn generate(
        &self,
        name: &str,
        generated: &mut Generated,
        denied: &mut Denied,
        attributes: &Attributes,
    ) {
        use Generator::*;
        match *self {
            Choose(ref options) => {
                let mut random = rand::thread_rng();
                if generated.contains_key(name) {
                    return;
                }
                let mut choices: BTreeMap<Chance, Vec<String>> = BTreeMap::new();
                for (option, value) in options.iter() {
                    if !value.requires.is_empty() {
                        let mut valid = true;
                        for requirement in &value.requires {
                            valid &= meets_requirement(requirement, &generated, &denied);
                        }
                        if !valid {
                            continue;
                        }
                    }
                    if let Some(denied_list) = denied.get(name) {
                        let mut invalid = false;
                        for denied in denied_list {
                            if option == denied {
                                invalid = true;
                                break;
                            }
                        }
                        if invalid {
                            continue;
                        }
                    }
                    let chance = value.chance.unwrap_or(Chance::Standard);
                    if chance == Chance::Always {
                        choices.clear();
                        choices.insert(Chance::Standard, vec![option.clone()]);
                        break;
                    } else if chance != Chance::Never {
                        choices
                            .entry(chance)
                            .or_insert_with(Default::default)
                            .push(option.clone());
                    }
                }
                if !choices.is_empty() {
                    let mut vec: Vec<_> = choices
                        .keys()
                        .map(|chance| {
                            Weighted {
                                weight: chance.chance(),
                                item: *chance,
                            }
                        })
                        .collect();
                    let wc = WeightedChoice::new(&mut vec);
                    let vec = choices.remove(&wc.sample(&mut random)).unwrap();
                    let option = &vec[random.gen_range(0, vec.len())];
                    match &options[option].generator {
                        &Generator::Nothing => {
                            generated.insert(name.to_string(), option.clone());
                        }
                        generator => {
                            generator.generate(name, generated, denied, attributes);
                        }
                    }
                }
            }
            Reuse(ref attribute_name) => {
                if let Some(attribute) = attributes.get(attribute_name) {
                    attribute.generator.generate(
                        name,
                        generated,
                        denied,
                        attributes,
                    );
                }
            }
            Same(ref attribute_name) => {
                if let Some(value) = generated.get(attribute_name).map(|value| value.clone()) {
                    generated.insert(name.to_string(), value);
                }
            }
            Nothing => (),
        }
    }

    fn get_requirements(&self, name: &str, attributes: &Attributes) -> Vec<Requirement> {
        match self {
            &Generator::Choose(ref options) => {
                if let Some(value) = options.get(name) {
                    value.get_requirements(name, attributes)
                } else {
                    for value in options.values() {
                        if value.generator.contains(name, attributes) {
                            return value.get_requirements(name, attributes);
                        }
                    }
                    Vec::new()
                }
            }
            &Generator::Reuse(ref attribute_name) => {
                if let Some(attribute) = attributes.get(attribute_name) {
                    attribute.generator.get_requirements(name, attributes)
                } else {
                    Vec::new()
                }
            }
            &Generator::Same(ref attribute_name) => {
                vec![
                    Requirement {
                        possibilities: vec![(attribute_name.to_string(), name.to_string(), false)],
                    },
                ]
            }
            &Generator::Nothing => Vec::new(),
        }
    }

    fn contains(&self, name: &str, attributes: &Attributes) -> bool {
        match self {
            &Generator::Choose(ref options) => {
                if let Some(value) = options.get(name) {
                    match &value.generator {
                        &Generator::Nothing => true,
                        generator => generator.contains(name, attributes),
                    }
                } else {
                    for value in options.values() {
                        if value.generator.contains(name, attributes) {
                            return true;
                        }
                    }
                    false
                }
            }
            &Generator::Reuse(ref attribute_name) |
            &Generator::Same(ref attribute_name) => {
                if let Some(attribute) = attributes.get(attribute_name) {
                    attribute.generator.contains(name, attributes)
                } else {
                    false
                }
            }
            &Generator::Nothing => false,
        }
    }
    
    fn always(&self, name: &str, attributes: &Attributes) -> bool {
        match self {
            &Generator::Choose(ref options) => {
                if let Some(value) = options.get(name) {
                    match &value.generator {
                        &Generator::Nothing => value.chance.map_or(false, |chance| chance == Chance::Always) || options.len() == 1,
                        generator => generator.always(name, attributes),
                    }
                } else {
                    for value in options.values() {
                        if (value.chance.map_or(false, |chance| chance == Chance::Always) || options.len() == 1)
                            && value.generator.contains(name, attributes) {
                            return value.generator.always(name, attributes);
                        }
                    }
                    false
                }
            }
            &Generator::Reuse(ref attribute_name) => {
                if let Some(attribute) = attributes.get(attribute_name) {
                    attribute.generator.always(name, attributes)
                } else {
                    false
                }
            }
            &Generator::Same(_) => true,
            &Generator::Nothing => false,
        }
    }
}

fn meets_requirement(requirement: &Requirement, generated: &Generated, denied: &Denied) -> bool {
    let mut matches = requirement.possibilities.is_empty();
    for &(ref key, ref value, not) in &requirement.possibilities {
        if generated.contains_key(key) {
            matches |= not ^ (value == "*" || &generated[key] == value);
        } else if not {
            if value == "*" {
                matches = true;
            }
            else if let Some(values) = denied.get(key) {
                matches |= values.contains(value);
            }
        }
    }
    matches
}

fn add_requirements(
    requires: &Vec<Requirement>,
    generated: &mut Generated,
    denied: &mut Denied,
    attributes: &Attributes,
) {
    let mut requires = requires.clone();
    let mut delayed = Vec::new();
    let mut random = rand::thread_rng();
    while let Some(requirement) = requires.pop().or_else(|| delayed.pop()) {
        if !meets_requirement(&requirement, generated, denied) {
            if requirement.possibilities.len() > 1 && !requires.is_empty() {
                delayed.push(requirement);
                continue;
            }
            let mut possibilities = requirement.possibilities.clone();
            let mut finding = true;
            while finding && possibilities.len() > 0 {
                let index = random.gen_range(0, possibilities.len());
                let (key, value, not) = possibilities.remove(index);
                if not && (!generated.contains_key(&key) || generated[&key] != value) {
                    denied.entry(key).or_insert_with(Default::default).push(
                        value,
                    );
                    finding = false;
                } else if !generated.contains_key(&key) {
                    if let Some(attribute) = attributes.get(&key) {
                        requires.append(&mut attribute.get_requirements(&value, attributes));
                    }
                    generated.insert(key, value);
                    finding = false;
                }
            }
            if finding {
                println!("Unable to find valid possibility for {:#?}", requirement);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Formatting {
    requirement: Requirement,
    contents: Vec<SubFormatting>,
}

#[derive(Clone, Debug)]
enum SubFormatting {
    Text(String),
    Formatted(Formatting),
    Variable(String),
}

impl Formatting {
    pub fn format(&self, generated: &Generated) -> String {
        if meets_requirement(&self.requirement, generated, &Default::default()) {
            let mut output = String::new();
            for formatting in &self.contents {
                let string = match formatting {
                    &SubFormatting::Text(ref text) => text.clone(),
                    &SubFormatting::Variable(ref variable) => {
                        let value = generated.get(&variable.to_lowercase()).map_or(
                            "".to_string(),
                            |value| value.clone(),
                        );
                        if &variable.to_uppercase() == variable {
                            value.to_uppercase()
                        } else if !value.is_empty() &&
                                   variable.chars().next().map_or(false, |c| c.is_uppercase())
                        {
                            let mut chars = value.chars();
                            chars.next().unwrap().to_uppercase().collect::<String>() +
                                &chars.collect::<String>()
                        } else {
                            value
                        }
                    }
                    &SubFormatting::Formatted(ref formatting) => formatting.format(generated),
                };
                if !string.is_empty() {
                    output += &string;
                }
            }
            for splitter in vec!["\n", " "].into_iter() {
                output = output
                    .split(splitter)
                    .filter(|split| !split.is_empty())
                    .map(|split| split.trim())
                    .collect::<Vec<_>>()
                    .join(splitter);
            }
            output.replace(" .", ".")
        } else {
            String::new()
        }
    }
}

impl FromStr for Formatting {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut requirement = Default::default();
        let mut contents = Vec::new();
        let mut depth = 0;
        let mut current = String::new();
        let mut escape = false;
        for c in s.chars() {
            if escape {
                escape = false;
                current.push(c);
                continue;
            }
            match c {
                '?' if depth == 0 => {
                    requirement = current.parse()?;
                    current = String::new();
                }
                '[' => {
                    if depth == 0 {
                        contents.push(SubFormatting::Text(current));
                        current = String::new();
                    } else {
                        current.push(c);
                    }
                    depth += 1;
                }
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        if current.contains('?') {
                            contents.push(SubFormatting::Formatted(current.parse()?));
                        } else {
                            contents.push(SubFormatting::Variable(current));
                        }
                        current = String::new();
                    } else {
                        current.push(c);
                    }
                }
                '\\' => escape = true,
                c => current.push(c),
            }
        }
        contents.push(SubFormatting::Text(current));
        Ok(Formatting {
            requirement,
            contents,
        })
    }
}

#[test]
fn test_formatting() {
    println!("{:#?}", "They have a [head casing] head casing [!eye shape:no?with [eye shape] eyes and [pupil] pupils]".parse::<Formatting>().unwrap());
}
