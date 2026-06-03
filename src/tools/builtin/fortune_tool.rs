use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde_json::Value;
use std::collections::HashMap;

/// A collection of Rust programming wisdom quotes and proverbs.
struct FortuneTool;

#[derive(Debug, Clone)]
struct Fortune {
    quote: &'static str,
    author: &'static str,
    category: FortuneCategory,
}

#[derive(Debug, Clone)]
enum FortuneCategory {
    Safety,
    Performance,
    Philosophy,
    Humor,
    BestPractice,
}

impl std::fmt::Display for FortuneCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FortuneCategory::Safety => write!(f, "safety"),
            FortuneCategory::Performance => write!(f, "performance"),
            FortuneCategory::Philosophy => write!(f, "philosophy"),
            FortuneCategory::Humor => write!(f, "humor"),
            FortuneCategory::BestPractice => write!(f, "best_practice"),
        }
    }
}

const FORTUNES: &[Fortune] = &[
    Fortune {
        quote: "If it compiles, it probably works. If it doesn't compile, it definitely doesn't work.",
        author: "Every Rust Developer",
        category: FortuneCategory::Philosophy,
    },
    Fortune {
        quote: "The borrow checker is not your enemy. It's your friend that yells at you to keep you safe.",
        author: "Anonymous",
        category: FortuneCategory::Safety,
    },
    Fortune {
        quote: "Zero-cost abstractions don't mean zero effort. They mean the compiler does the hard work so you don't have to.",
        author: "Rust Community",
        category: FortuneCategory::Performance,
    },
    Fortune {
        quote: "Fearless concurrency is real, but fearless debugging is not.",
        author: "Senior Rustacean",
        category: FortuneCategory::Humor,
    },
    Fortune {
        quote: "Option and Result are not just types, they are a way of life. Embrace them.",
        author: "Rust Philosopher",
        category: FortuneCategory::BestPractice,
    },
    Fortune {
        quote: "unsafe doesn't mean dangerous. It means 'trust me, compiler, I know what I'm doing.' Use it wisely.",
        author: "Systems Programmer",
        category: FortuneCategory::Safety,
    },
    Fortune {
        quote: "A lifetime is not a duration. It's a promise that some data will outlive some scope.",
        author: "Rust Educator",
        category: FortuneCategory::Philosophy,
    },
    Fortune {
        quote: "Rust: the only language where you spend more time fighting the compiler than debugging your code.",
        author: "New Rustacean",
        category: FortuneCategory::Humor,
    },
    Fortune {
        quote: "Iterators are faster than for-loops. Not because they're magic, but because the compiler can optimize them better.",
        author: "Performance Engineer",
        category: FortuneCategory::Performance,
    },
    Fortune {
        quote: "Don't fight the borrow checker. Work with it. It knows more about your code than you do.",
        author: "Rust Mentor",
        category: FortuneCategory::BestPractice,
    },
    Fortune {
        quote: "String vs &str: owned vs borrowed. One allocates, one doesn't. Choose based on who owns the data.",
        author: "API Designer",
        category: FortuneCategory::BestPractice,
    },
    Fortune {
        quote: "Cargo is not just a build tool. It's a package manager, a test runner, a doc generator, and your best friend.",
        author: "DevOps Engineer",
        category: FortuneCategory::Philosophy,
    },
    Fortune {
        quote: "Clippy doesn't just find bugs. It teaches you to write idiomatic Rust.",
        author: "Code Reviewer",
        category: FortuneCategory::BestPractice,
    },
    Fortune {
        quote: "Rust has two speeds: compile time (slow) and runtime (fast). The trade-off is worth it.",
        author: "Performance Optimizer",
        category: FortuneCategory::Performance,
    },
    Fortune {
        quote: "Traits are Rust's answer to interfaces. But they're more powerful: they support generics, associated types, and default implementations.",
        author: "Type Theory Enthusiast",
        category: FortuneCategory::Philosophy,
    },
    Fortune {
        quote: "Macros are code that writes code. Use them when you need to reduce boilerplate, but prefer functions when you can.",
        author: "Metaprogramming Expert",
        category: FortuneCategory::BestPractice,
    },
    Fortune {
        quote: "The Rust community is one of the most welcoming and helpful in the open-source world.",
        author: "Open Source Contributor",
        category: FortuneCategory::Philosophy,
    },
    Fortune {
        quote: "Match statements are exhaustive by default. The compiler won't let you forget a case.",
        author: "Pattern Matching Fan",
        category: FortuneCategory::Safety,
    },
    Fortune {
        quote: "Send and Sync are the traits that make fearless concurrency possible. If you can't explain them, you don't understand Rust.",
        author: "Concurrency Expert",
        category: FortuneCategory::Safety,
    },
    Fortune {
        quote: "Rust doesn't have a garbage collector. It has a borrow checker. And that makes all the difference.",
        author: "Language Comparer",
        category: FortuneCategory::Philosophy,
    },
    Fortune {
        quote: "unwrap() is fine in examples. In production, use expect() with a meaningful message, or handle the error properly.",
        author: "Production Rustacean",
        category: FortuneCategory::BestPractice,
    },
    Fortune {
        quote: "Async Rust is powerful, but it's not free. Understand the runtime, the executor, and the cost of .await.",
        author: "Tokio Developer",
        category: FortuneCategory::Performance,
    },
    Fortune {
        quote: "Rust's type system is Turing complete at compile time. You can compute anything before the program even runs.",
        author: "Type System Hacker",
        category: FortuneCategory::Philosophy,
    },
    Fortune {
        quote: "If your code has more than three levels of nesting, it's time to refactor. Early returns are your friend.",
        author: "Clean Code Advocate",
        category: FortuneCategory::BestPractice,
    },
    Fortune {
        quote: "Rust: where 'it works on my machine' actually means something.",
        author: "Cross-Platform Developer",
        category: FortuneCategory::Humor,
    },
];

/// Returns a random fortune from the collection.
fn random_fortune() -> &'static Fortune {
    let mut rng = thread_rng();
    FORTUNES.choose(&mut rng).unwrap_or(&FORTUNES[0])
}

/// Searches fortunes by keyword in quote, author, or category.
fn search_fortunes(keyword: &str) -> Vec<&Fortune> {
    let keyword_lower = keyword.to_lowercase();
    FORTUNES
        .iter()
        .filter(|f| {
            f.quote.to_lowercase().contains(&keyword_lower)
                || f.author.to_lowercase().contains(&keyword_lower)
                || f.category.to_string().to_lowercase().contains(&keyword_lower)
        })
        .collect()
}

/// Gets fortunes filtered by category.
fn fortunes_by_category(category: &str) -> Vec<&Fortune> {
    FORTUNES
        .iter()
        .filter(|f| f.category.to_string().to_lowercase() == category.to_lowercase())
        .collect()
}

#[async_trait::async_trait]
impl Tool for FortuneTool {
    fn name(&self) -> &str {
        "fortune"
    }

    fn description(&self) -> &str {
        "Rust programming wisdom and quotes. Get random fortunes, search by keyword, or filter by category (safety, performance, philosophy, humor, best_practice)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".into(),
                description: "Action: random (default), search, category, list".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "keyword".into(),
                description: "Keyword to search for (used with action=search)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "category".into(),
                description: "Filter by category: safety, performance, philosophy, humor, best_practice (used with action=category)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "limit".into(),
                description: "Max results for search/category (default: 5)".into(),
                required: false,
                parameter_type: "number".into(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("random");

        match action {
            "random" => {
                let fortune = random_fortune();
                Ok(serde_json::json!({
                    "action": "random",
                    "quote": fortune.quote,
                    "author": fortune.author,
                    "category": fortune.category.to_string()
                }))
            }
            "search" => {
                let keyword = params
                    .get("keyword")
                    .and_then(|v| v.as_str())
                    .ok_or("keyword parameter is required for search action")?;
                let limit = params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5) as usize;
                let results = search_fortunes(keyword);
                let results: Vec<_> = results.into_iter().take(limit).collect();

                if results.is_empty() {
                    Ok(serde_json::json!({
                        "action": "search",
                        "keyword": keyword,
                        "count": 0,
                        "message": format!("No fortunes found matching '{}'", keyword)
                    }))
                } else {
                    let fortunes: Vec<Value> = results
                        .iter()
                        .map(|f| {
                            serde_json::json!({
                                "quote": f.quote,
                                "author": f.author,
                                "category": f.category.to_string()
                            })
                        })
                        .collect();
                    Ok(serde_json::json!({
                        "action": "search",
                        "keyword": keyword,
                        "count": fortunes.len(),
                        "results": fortunes
                    }))
                }
            }
            "category" => {
                let category = params
                    .get("category")
                    .and_then(|v| v.as_str())
                    .ok_or("category parameter is required for category action. Available: safety, performance, philosophy, humor, best_practice")?;
                let limit = params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5) as usize;
                let results = fortunes_by_category(category);
                let results: Vec<_> = results.into_iter().take(limit).collect();

                if results.is_empty() {
                    Ok(serde_json::json!({
                        "action": "category",
                        "category": category,
                        "count": 0,
                        "message": format!("Unknown category '{}'. Available: safety, performance, philosophy, humor, best_practice", category)
                    }))
                } else {
                    let fortunes: Vec<Value> = results
                        .iter()
                        .map(|f| {
                            serde_json::json!({
                                "quote": f.quote,
                                "author": f.author,
                                "category": f.category.to_string()
                            })
                        })
                        .collect();
                    Ok(serde_json::json!({
                        "action": "category",
                        "category": category,
                        "count": fortunes.len(),
                        "results": fortunes
                    }))
                }
            }
            "list" => {
                let categories: Vec<Value> = FORTUNES
                    .iter()
                    .map(|f| f.category.to_string())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .map(|c| {
                        serde_json::json!({
                            "category": c,
                            "count": FORTUNES.iter().filter(|f| f.category.to_string() == c).count()
                        })
                    })
                    .collect();
                Ok(serde_json::json!({
                    "action": "list",
                    "total_fortunes": FORTUNES.len(),
                    "categories": categories
                }))
            }
            unknown => Err(format!(
                "Unknown action '{}'. Available actions: random, search, category, list",
                unknown
            )),
        }
    }
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(FortuneTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_fortune_returns_valid_fortune() {
        let fortune = random_fortune();
        assert!(!fortune.quote.is_empty());
        assert!(!fortune.author.is_empty());
    }

    #[test]
    fn test_search_fortunes_by_quote() {
        let results = search_fortunes("borrow checker");
        assert!(!results.is_empty());
        assert!(results.iter().all(|f| {
            f.quote.to_lowercase().contains("borrow checker")
                || f.author.to_lowercase().contains("borrow checker")
        }));
    }

    #[test]
    fn test_search_fortunes_no_match() {
        let results = search_fortunes("nonexistent_keyword_xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_fortunes_case_insensitive() {
        let lower = search_fortunes("rust");
        let upper = search_fortunes("RUST");
        assert_eq!(lower.len(), upper.len());
    }

    #[test]
    fn test_fortunes_by_category() {
        let safety = fortunes_by_category("safety");
        assert!(!safety.is_empty());
        assert!(safety.iter().all(|f| matches!(f.category, FortuneCategory::Safety)));
    }

    #[test]
    fn test_fortunes_by_category_unknown() {
        let results = fortunes_by_category("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_all_fortunes_have_valid_data() {
        for f in FORTUNES {
            assert!(!f.quote.is_empty(), "Empty quote found");
            assert!(!f.author.is_empty(), "Empty author found");
            assert!(!f.category.to_string().is_empty(), "Empty category found");
        }
    }

    #[test]
    fn test_fortune_count() {
        assert!(FORTUNES.len() >= 20, "Should have at least 20 fortunes, got {}", FORTUNES.len());
    }
}
