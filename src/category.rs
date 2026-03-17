use std::fmt;

pub const BUILTIN_CATEGORIES: &[&str] = &["work", "personal", "shopping", "health"];

#[derive(Clone, PartialEq)]
pub enum Category {
    Work,
    Personal,
    Shopping,
    Health,
    Custom(String),
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Category::Work => write!(f, "work"),
            Category::Personal => write!(f, "personal"),
            Category::Shopping => write!(f, "shopping"),
            Category::Health => write!(f, "health"),
            Category::Custom(name) => write!(f, "{name}"),
        }
    }
}

/// Parse a category name. Accepts built-in names and registered custom categories.
pub fn parse_category(name: &str, custom_categories: &[String]) -> Result<Category, String> {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "work" => Ok(Category::Work),
        "personal" => Ok(Category::Personal),
        "shopping" => Ok(Category::Shopping),
        "health" => Ok(Category::Health),
        _ => {
            if let Some(stored) = custom_categories.iter().find(|c| c.to_lowercase() == lower) {
                Ok(Category::Custom(stored.clone()))
            } else {
                Err(format!("Unknown category: {name}"))
            }
        }
    }
}
