use super::{Template, Requirement};
use std::os::raw::c_char;
use std::ffi::{CString, CStr};
use std::collections::HashMap;
use serde_json;

#[no_mangle]
pub fn generate(template: *mut c_char, presets: *mut c_char) -> *mut c_char {
    let template = unsafe { CStr::from_ptr(template).to_string_lossy().to_string() };
    let presets = unsafe { CStr::from_ptr(presets).to_string_lossy().to_owned() };
    let generated = if let Some(template) = TEMPLATES.get(&template) {
        let presets: Vec<Requirement> = serde_json::from_str(&presets).unwrap();
        serde_json::to_string(&template.generate(presets)).unwrap()
    } else {
        "{species:unknown}".to_string()
    };
    CString::new(generated.as_str()).unwrap().into_raw()
}



lazy_static! {
    static ref TEMPLATES: HashMap<String, Template> = {
        let mut m = HashMap::new();
        let base_template = Template::new_from_string(include_str!("../assets/base.json"), None);
        let obj_template = Template::new_from_string(include_str!("../assets/obj.json"), Some(&base_template));
        m.insert("base".to_string(), base_template);
        m.insert("obj".to_string(), obj_template);
        m
    };
}
