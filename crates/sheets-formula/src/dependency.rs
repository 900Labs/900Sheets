use crate::ast::MAX_EXPANDED_REFERENCES;
use crate::parser::Parser;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellKey {
    pub sheet_id: u64,
    pub row: u32,
    pub col: u32,
}

impl CellKey {
    pub const fn new(sheet_id: u64, row: u32, col: u32) -> Self {
        Self { sheet_id, row, col }
    }
}

#[derive(Clone, Default)]
pub struct DependencyGraph {
    deps: HashMap<CellKey, HashSet<CellKey>>,
    dependents: HashMap<CellKey, HashSet<CellKey>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.deps.clear();
        self.dependents.clear();
    }

    pub fn set_formula(&mut self, row: u32, col: u32, formula: &str) -> Result<(), String> {
        self.set_formula_on_sheet(0, row, col, formula, |_| None)
    }

    pub fn set_formula_on_sheet<F>(
        &mut self,
        sheet_id: u64,
        row: u32,
        col: u32,
        formula: &str,
        resolve_sheet: F,
    ) -> Result<(), String>
    where
        F: Fn(&str) -> Option<u64>,
    {
        let expr = Parser::parse_formula(formula).map_err(|error| error.to_string())?;
        let mut refs = HashSet::new();
        expr.for_each_qualified_reference(MAX_EXPANDED_REFERENCES, &mut |reference| {
            if let Some(target_sheet) = reference
                .sheet
                .as_deref()
                .and_then(&resolve_sheet)
                .or_else(|| reference.sheet.is_none().then_some(sheet_id))
            {
                refs.insert(CellKey::new(target_sheet, reference.row, reference.col));
            }
        })
        .map_err(|error| error.to_string())?;
        let cell = CellKey::new(sheet_id, row, col);

        let old_deps = self.remove_formula_edges(cell);
        self.insert_formula_edges(cell, refs);

        if self.has_circular_ref_key(cell) {
            self.remove_formula_edges(cell);
            if let Some(old_deps) = old_deps {
                self.insert_formula_edges(cell, old_deps);
            }
            return Err(format!(
                "Circular reference detected at sheet {sheet_id}, ({row}, {col})"
            ));
        }

        Ok(())
    }

    fn remove_formula_edges(&mut self, cell: CellKey) -> Option<HashSet<CellKey>> {
        let old_deps = self.deps.remove(&cell)?;
        for dep in &old_deps {
            if let Some(dependents) = self.dependents.get_mut(dep) {
                dependents.remove(&cell);
                if dependents.is_empty() {
                    self.dependents.remove(dep);
                }
            }
        }
        Some(old_deps)
    }

    fn insert_formula_edges(&mut self, cell: CellKey, refs: HashSet<CellKey>) {
        for dep in &refs {
            self.dependents.entry(*dep).or_default().insert(cell);
        }
        self.deps.insert(cell, refs);
    }

    pub fn clear_cell(&mut self, row: u32, col: u32) {
        self.clear_cell_key(CellKey::new(0, row, col));
    }

    pub fn clear_cell_key(&mut self, cell: CellKey) {
        self.remove_formula_edges(cell);
    }

    pub fn get_dependencies(&self, row: u32, col: u32) -> Option<&HashSet<CellKey>> {
        self.get_dependencies_key(CellKey::new(0, row, col))
    }

    pub fn get_dependencies_key(&self, cell: CellKey) -> Option<&HashSet<CellKey>> {
        self.deps.get(&cell)
    }

    pub fn get_dependents(&self, row: u32, col: u32) -> Option<&HashSet<CellKey>> {
        self.get_dependents_key(CellKey::new(0, row, col))
    }

    pub fn get_dependents_key(&self, cell: CellKey) -> Option<&HashSet<CellKey>> {
        self.dependents.get(&cell)
    }

    pub fn get_all_dependents(&self, row: u32, col: u32) -> HashSet<CellKey> {
        self.get_all_dependents_key(CellKey::new(0, row, col))
    }

    pub fn get_all_dependents_key(&self, cell: CellKey) -> HashSet<CellKey> {
        let mut result = HashSet::new();
        let mut queue = vec![cell];
        let mut visited = HashSet::from([cell]);

        while let Some(cell) = queue.pop() {
            if let Some(dependents) = self.dependents.get(&cell) {
                for dependent in dependents {
                    if visited.insert(*dependent) {
                        result.insert(*dependent);
                        queue.push(*dependent);
                    }
                }
            }
        }
        result
    }

    pub fn has_circular_ref(&self, row: u32, col: u32) -> bool {
        self.has_circular_ref_key(CellKey::new(0, row, col))
    }

    pub fn has_circular_ref_key(&self, cell: CellKey) -> bool {
        self.detect_cycle(cell, &mut HashSet::new())
    }

    fn detect_cycle(&self, cell: CellKey, visiting: &mut HashSet<CellKey>) -> bool {
        if !visiting.insert(cell) {
            return true;
        }
        if let Some(deps) = self.deps.get(&cell) {
            for dep in deps {
                if self.detect_cycle(*dep, visiting) {
                    return true;
                }
            }
        }
        visiting.remove(&cell);
        false
    }

    pub fn topological_sort(&self) -> Vec<CellKey> {
        let mut in_degree: HashMap<CellKey, usize> = HashMap::new();
        for (cell, deps) in &self.deps {
            in_degree.entry(*cell).or_insert(0);
            for dep in deps {
                in_degree.entry(*dep).or_insert(0);
                *in_degree.entry(*cell).or_insert(0) += 1;
            }
        }

        let mut ready: Vec<_> = in_degree
            .iter()
            .filter_map(|(cell, degree)| (*degree == 0).then_some(*cell))
            .collect();
        ready.sort();
        let mut queue: VecDeque<_> = ready.into();
        let mut result = Vec::new();
        while let Some(cell) = queue.pop_front() {
            result.push(cell);
            if let Some(dependents) = self.dependents.get(&cell) {
                let mut dependents: Vec<_> = dependents.iter().copied().collect();
                dependents.sort();
                for dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(&dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent);
                        }
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_sheet_ids_are_part_of_dependency_keys() {
        let mut graph = DependencyGraph::new();
        graph
            .set_formula_on_sheet(20, 0, 0, "Data!B1+C1", |name| {
                (name == "Data").then_some(10)
            })
            .unwrap();
        let deps = graph.get_dependencies_key(CellKey::new(20, 0, 0)).unwrap();
        assert!(deps.contains(&CellKey::new(10, 0, 1)));
        assert!(deps.contains(&CellKey::new(20, 0, 2)));
    }

    #[test]
    fn cross_sheet_cycle_is_rejected_and_rolled_back() {
        let mut graph = DependencyGraph::new();
        let resolve = |name: &str| match name {
            "First" => Some(1),
            "Second" => Some(2),
            _ => None,
        };
        graph
            .set_formula_on_sheet(1, 0, 0, "Second!A1", resolve)
            .unwrap();
        assert!(graph
            .set_formula_on_sheet(2, 0, 0, "First!A1", resolve)
            .is_err());
        assert!(graph.get_dependencies_key(CellKey::new(2, 0, 0)).is_none());
    }

    #[test]
    fn workbook_wide_dependents_and_order_cross_sheet_boundaries() {
        let mut graph = DependencyGraph::new();
        graph
            .set_formula_on_sheet(2, 0, 0, "First!A1", |name| (name == "First").then_some(1))
            .unwrap();
        graph
            .set_formula_on_sheet(3, 0, 0, "Second!A1", |name| (name == "Second").then_some(2))
            .unwrap();
        let all = graph.get_all_dependents_key(CellKey::new(1, 0, 0));
        assert_eq!(
            all,
            HashSet::from([CellKey::new(2, 0, 0), CellKey::new(3, 0, 0)])
        );
        let sorted = graph.topological_sort();
        let positions = |key| {
            sorted
                .iter()
                .position(|candidate| *candidate == key)
                .unwrap()
        };
        assert!(positions(CellKey::new(1, 0, 0)) < positions(CellKey::new(2, 0, 0)));
        assert!(positions(CellKey::new(2, 0, 0)) < positions(CellKey::new(3, 0, 0)));
    }

    #[test]
    fn formula_replacement_rolls_back_on_cycle() {
        let mut graph = DependencyGraph::new();
        graph.set_formula(0, 0, "C1").unwrap();
        graph.set_formula(0, 1, "A1").unwrap();
        assert!(graph.set_formula(0, 0, "B1").is_err());
        let deps = graph.get_dependencies(0, 0).unwrap();
        assert!(deps.contains(&CellKey::new(0, 0, 2)));
    }

    #[test]
    fn huge_range_dependency_is_rejected_without_materialization() {
        let mut graph = DependencyGraph::new();
        let error = graph.set_formula(0, 0, "SUM(A1:XFD1000000)").unwrap_err();
        assert!(error.contains("safe reference limit"));
        assert!(graph.get_dependencies(0, 0).is_none());
    }
}
