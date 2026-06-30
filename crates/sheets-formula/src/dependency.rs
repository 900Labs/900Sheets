use crate::parser::Parser;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct DependencyGraph {
    deps: HashMap<(u32, u32), HashSet<(u32, u32)>>,
    dependents: HashMap<(u32, u32), HashSet<(u32, u32)>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            deps: HashMap::new(),
            dependents: HashMap::new(),
        }
    }

    pub fn set_formula(&mut self, row: u32, col: u32, formula: &str) -> Result<(), String> {
        let expr = Parser::parse_formula(formula).map_err(|e| e.to_string())?;
        let refs: HashSet<(u32, u32)> = expr.references().into_iter().collect();

        if let Some(old_deps) = self.deps.remove(&(row, col)) {
            for dep in &old_deps {
                if let Some(deps_set) = self.dependents.get_mut(dep) {
                    deps_set.remove(&(row, col));
                }
            }
        }

        for dep in &refs {
            self.dependents.entry(*dep).or_default().insert((row, col));
        }

        self.deps.insert((row, col), refs);
        Ok(())
    }

    pub fn clear_cell(&mut self, row: u32, col: u32) {
        if let Some(old_deps) = self.deps.remove(&(row, col)) {
            for dep in &old_deps {
                if let Some(deps_set) = self.dependents.get_mut(dep) {
                    deps_set.remove(&(row, col));
                }
            }
        }
    }

    pub fn get_dependencies(&self, row: u32, col: u32) -> Option<&HashSet<(u32, u32)>> {
        self.deps.get(&(row, col))
    }

    pub fn get_dependents(&self, row: u32, col: u32) -> Option<&HashSet<(u32, u32)>> {
        self.dependents.get(&(row, col))
    }

    pub fn get_all_dependents(&self, row: u32, col: u32) -> HashSet<(u32, u32)> {
        let mut result = HashSet::new();
        let mut queue = vec![(row, col)];
        let mut visited = HashSet::new();
        visited.insert((row, col));

        while let Some(cell) = queue.pop() {
            if let Some(deps) = self.dependents.get(&cell) {
                for dep in deps {
                    if visited.insert(*dep) {
                        result.insert(*dep);
                        queue.push(*dep);
                    }
                }
            }
        }
        result
    }

    pub fn has_circular_ref(&self, row: u32, col: u32) -> bool {
        let mut visited = HashSet::new();
        self.detect_cycle(row, col, &mut visited)
    }

    fn detect_cycle(&self, row: u32, col: u32, visited: &mut HashSet<(u32, u32)>) -> bool {
        if visited.contains(&(row, col)) {
            return true;
        }
        visited.insert((row, col));

        if let Some(deps) = self.deps.get(&(row, col)) {
            for dep in deps {
                if self.detect_cycle(dep.0, dep.1, visited) {
                    return true;
                }
            }
        }

        visited.remove(&(row, col));
        false
    }

    pub fn topological_sort(&self) -> Vec<(u32, u32)> {
        let mut in_degree: HashMap<(u32, u32), usize> = HashMap::new();

        for (cell, deps) in &self.deps {
            in_degree.entry(*cell).or_insert(0);
            for dep in deps {
                in_degree.entry(*dep).or_insert(0);
                *in_degree.entry(*cell).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<(u32, u32)> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&c, _)| c)
            .collect();
        let mut sorted: Vec<(u32, u32)> = queue.iter().copied().collect();
        sorted.sort();
        queue = sorted.into_iter().collect();

        let mut result = Vec::new();

        while let Some(cell) = queue.pop_front() {
            result.push(cell);
            if let Some(dependents) = self.dependents.get(&cell) {
                for dep in dependents {
                    if let Some(d) = in_degree.get_mut(dep) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(*dep);
                        }
                    }
                }
            }
        }

        result
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_formula_deps() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 0, "B1+C1").unwrap();
        let deps = graph.get_dependencies(0, 0).unwrap();
        assert!(deps.contains(&(0, 1)));
        assert!(deps.contains(&(0, 2)));
    }

    #[test]
    fn test_dependents() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 0, "B1").unwrap();
        let deps = graph.get_dependents(0, 1).unwrap();
        assert!(deps.contains(&(0, 0)));
    }

    #[test]
    fn test_clear_cell() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 0, "B1").unwrap();
        assert!(graph.get_dependencies(0, 0).is_some());
        graph.clear_cell(0, 0);
        assert!(graph.get_dependencies(0, 0).is_none());
        assert!(
            graph.get_dependents(0, 1).is_none() || graph.get_dependents(0, 1).unwrap().is_empty()
        );
    }

    #[test]
    fn test_no_circular_ref() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 0, "B1").unwrap();
        graph.set_formula(0, 1, "C1").unwrap();
        assert!(!graph.has_circular_ref(0, 0));
    }

    #[test]
    fn test_circular_ref_detected() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 0, "B1").unwrap();
        graph.set_formula(0, 1, "A1").unwrap();
        assert!(graph.has_circular_ref(0, 0));
    }

    #[test]
    fn test_all_dependents() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 1, "A1").unwrap();
        graph.set_formula(0, 2, "B1").unwrap();
        let all = graph.get_all_dependents(0, 0);
        assert!(all.contains(&(0, 1)));
        assert!(all.contains(&(0, 2)));
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 1, "A1").unwrap();
        graph.set_formula(0, 2, "B1").unwrap();
        let sorted = graph.topological_sort();
        let a_pos = sorted.iter().position(|&c| c == (0, 0)).unwrap();
        let b_pos = sorted.iter().position(|&c| c == (0, 1)).unwrap();
        let c_pos = sorted.iter().position(|&c| c == (0, 2)).unwrap();
        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }
}
