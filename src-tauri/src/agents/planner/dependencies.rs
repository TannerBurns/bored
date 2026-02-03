//! Dependency management and topological sorting for epic execution.

use crate::db::PlanEpic;
use std::collections::{HashMap, HashSet};

/// Topologically sort epics so dependencies come before dependents.
/// Returns an error if there's a cycle in the dependency graph.
pub fn topological_sort_epics(epics: &[PlanEpic]) -> Result<Vec<&PlanEpic>, String> {
    // Build title -> epic reference map
    let title_to_epic: HashMap<&str, &PlanEpic> =
        epics.iter().map(|e| (e.title.as_str(), e)).collect();

    // Track visited and in-current-path for cycle detection
    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_path: HashSet<&str> = HashSet::new();
    let mut result: Vec<&PlanEpic> = Vec::new();

    fn visit<'a>(
        title: &'a str,
        title_to_epic: &HashMap<&str, &'a PlanEpic>,
        visited: &mut HashSet<&'a str>,
        in_path: &mut HashSet<&'a str>,
        result: &mut Vec<&'a PlanEpic>,
    ) -> Result<(), String> {
        if in_path.contains(title) {
            return Err(format!(
                "Circular dependency detected involving epic '{}'",
                title
            ));
        }

        if visited.contains(title) {
            return Ok(());
        }

        in_path.insert(title);

        if let Some(epic) = title_to_epic.get(title) {
            // Visit all dependencies first
            for dep_title in &epic.depends_on {
                visit(dep_title, title_to_epic, visited, in_path, result)?;
            }

            visited.insert(title);
            in_path.remove(title);
            result.push(epic);
        }

        Ok(())
    }

    // Visit all epics
    for epic in epics {
        visit(
            &epic.title,
            &title_to_epic,
            &mut visited,
            &mut in_path,
            &mut result,
        )?;
    }

    Ok(result)
}

/// Calculate execution phases based on dependencies.
/// Returns a vector of phases, where each phase contains epics that can run in parallel.
pub fn calculate_execution_phases(epics: &[PlanEpic]) -> Vec<Vec<&PlanEpic>> {
    let title_to_epic: HashMap<&str, &PlanEpic> =
        epics.iter().map(|e| (e.title.as_str(), e)).collect();

    let mut levels: HashMap<&str, usize> = HashMap::new();

    fn get_level<'a>(
        epic: &'a PlanEpic,
        title_to_epic: &HashMap<&str, &'a PlanEpic>,
        levels: &mut HashMap<&'a str, usize>,
    ) -> usize {
        if let Some(&level) = levels.get(epic.title.as_str()) {
            return level;
        }

        if epic.depends_on.is_empty() {
            levels.insert(&epic.title, 0);
            return 0;
        }

        let mut max_dep_level = 0;
        for dep_title in &epic.depends_on {
            if let Some(dep_epic) = title_to_epic.get(dep_title.as_str()) {
                let dep_level = get_level(dep_epic, title_to_epic, levels);
                max_dep_level = max_dep_level.max(dep_level + 1);
            }
        }
        levels.insert(&epic.title, max_dep_level);
        max_dep_level
    }

    // Calculate levels for all epics
    for epic in epics {
        get_level(epic, &title_to_epic, &mut levels);
    }

    // Group by level
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut phases: Vec<Vec<&PlanEpic>> = vec![vec![]; max_level + 1];

    for epic in epics {
        let level = levels.get(epic.title.as_str()).copied().unwrap_or(0);
        phases[level].push(epic);
    }

    phases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_sort_no_dependencies() {
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 2);
    }

    #[test]
    fn test_topological_sort_with_dependencies() {
        // B depends on A, so A should come first
        let epics = vec![
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].title, "A");
        assert_eq!(sorted[1].title, "B");
    }

    #[test]
    fn test_topological_sort_forward_reference_works() {
        // This is the bug case: C depends on D, but D appears after C in the list
        // The topological sort should handle this correctly
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: vec!["D".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "D".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 4);

        // Build a position map
        let positions: std::collections::HashMap<_, _> = sorted
            .iter()
            .enumerate()
            .map(|(i, e)| (e.title.as_str(), i))
            .collect();

        // A should come before B
        assert!(positions["A"] < positions["B"]);
        // D should come before C
        assert!(positions["D"] < positions["C"]);
    }

    #[test]
    fn test_topological_sort_detects_cycle() {
        // A -> B -> A (cycle)
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec!["B".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
        ];

        let result = topological_sort_epics(&epics);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular dependency"));
    }

    #[test]
    fn test_topological_sort_chain() {
        // C -> B -> A (chain)
        let epics = vec![
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: vec!["B".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].title, "A");
        assert_eq!(sorted[1].title, "B");
        assert_eq!(sorted[2].title, "C");
    }

    #[test]
    fn test_topological_sort_multiple_dependencies() {
        // C depends on both A and B
        let epics = vec![
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string(), "B".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 3);

        // Build a position map
        let positions: std::collections::HashMap<_, _> = sorted
            .iter()
            .enumerate()
            .map(|(i, e)| (e.title.as_str(), i))
            .collect();

        // Both A and B should come before C
        assert!(positions["A"] < positions["C"]);
        assert!(positions["B"] < positions["C"]);
    }

    #[test]
    fn test_calculate_execution_phases_single_root() {
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
        ];

        let phases = calculate_execution_phases(&epics);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].len(), 1); // A
        assert_eq!(phases[1].len(), 1); // B
    }

    #[test]
    fn test_calculate_execution_phases_parallel() {
        // A and B can run in parallel, C depends on both
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string(), "B".to_string()],
                tickets: vec![],
            },
        ];

        let phases = calculate_execution_phases(&epics);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].len(), 2); // A and B in parallel
        assert_eq!(phases[1].len(), 1); // C
    }
}
