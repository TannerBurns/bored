//! Utility functions for prompt generation.

/// Convert a title to a URL-friendly slug
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(50)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_simple_title() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn slugify_special_characters() {
        assert_eq!(slugify("Add user@auth feature!"), "add-user-auth-feature");
    }

    #[test]
    fn slugify_multiple_spaces() {
        assert_eq!(slugify("Fix   multiple   spaces"), "fix-multiple-spaces");
    }

    #[test]
    fn slugify_long_title_truncates() {
        let long_title = "A".repeat(100);
        let result = slugify(&long_title);
        assert!(result.len() <= 50);
    }
}
