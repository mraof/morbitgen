extern crate serde_json;
extern crate morbitgen;

use morbitgen::Template;

fn main() {
    let base_template = Template::new("base", None);
    let obj_template = Template::new("obj", Some(&base_template));
    /*    println!(
        "Base: {}\nOBJ: {}",
        serde_json::to_string_pretty(&base_template).unwrap(),
        serde_json::to_string_pretty(&obj_template).unwrap()
    );*/
    println!("{:#?}", obj_template.order);
    let presets = vec![
        "flavor:normal".parse().unwrap(),
        "roll head casing color:yes".parse().unwrap(),
    ];
    let generated = obj_template.generate(presets);
    println!("{}", obj_template.format(&generated, "json").unwrap());
    println!("{}", obj_template.format(&generated, "full").unwrap());
}
