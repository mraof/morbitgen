use super::{Requirement, Attribute, Generator};
use serde::{Deserialize, Deserializer, de, Serialize, Serializer};

impl<'de> Deserialize<'de> for Requirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?.parse().map_err(
            de::Error::custom,
        )
    }
}

impl Serialize for Requirement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Attribute {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use std::fmt;
        use serde::de::{Visitor, MapAccess};
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Choose,
            Reuse,
            Nothing,
            Replace,
            Chance,
            Requires,
        }

        struct AttributeVisitor;
        impl<'de> Visitor<'de> for AttributeVisitor {
            type Value = Attribute;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Attribute")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut generator = None;
                let mut replace = None;
                let mut chance = None;
                let mut requires = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Replace => {
                            if replace.is_some() {
                                return Err(de::Error::duplicate_field("replace"));
                            }
                            replace = Some(map.next_value()?);
                        }
                        Field::Chance => {
                            if chance.is_some() {
                                return Err(de::Error::duplicate_field("chance"));
                            }
                            chance = Some(map.next_value()?);
                        }
                        Field::Requires => {
                            if requires.is_some() {
                                return Err(de::Error::duplicate_field("requires"));
                            }
                            requires = Some(map.next_value()?);
                        }
                        Field::Choose => {
                            if generator.is_some() {
                                return Err(de::Error::duplicate_field("generator"));
                            }
                            generator = Some(Generator::Choose(map.next_value()?));
                        }
                        Field::Reuse => {
                            if generator.is_some() {
                                return Err(de::Error::duplicate_field("generator"));
                            }
                            generator = Some(Generator::Reuse(map.next_value()?));
                        }
                        Field::Nothing => {
                            if generator.is_some() {
                                return Err(de::Error::duplicate_field("generator"));
                            }
                            let _: () = map.next_value()?;
                            generator = Some(Generator::Nothing);
                        }
                    }
                }

                let generator = generator.unwrap_or(Generator::Nothing);
                let replace = replace.unwrap_or(false);
                let chance = chance.unwrap_or(None);
                let requires = requires.unwrap_or_else(Vec::new);
                Ok(Attribute {
                    generator,
                    replace,
                    chance,
                    requires,
                })
            }
        }

        const FIELDS: &'static [&'static str] =
            &["choose", "nothing", "replace", "chance", "requires"];
        deserializer.deserialize_struct("Attribute", FIELDS, AttributeVisitor)
    }
}

impl Serialize for Attribute {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(4))?;
        match &self.generator {
            &Generator::Choose(ref choices) => {
                map.serialize_entry("choose", choices)?;
            }
            &Generator::Reuse(ref attribute_name) => {
                map.serialize_entry("reuse", attribute_name)?;
            }
            &Generator::Same(ref attribute_name) => {
                map.serialize_entry("copy", attribute_name)?;
            }
            &Generator::Nothing => {}
        }
        if self.replace {
            map.serialize_entry("replace", &self.replace)?;
        }
        if self.chance.is_some() {
            map.serialize_entry("chance", &self.chance)?;
        }
        if !self.requires.is_empty() {
            map.serialize_entry("requires", &self.requires)?;
        }
        map.end()
    }
}
