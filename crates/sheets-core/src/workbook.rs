use crate::error::CoreError;
use crate::sheet::{Sheet, StructureEdit};

#[derive(Clone)]
pub struct Workbook {
    sheets: Vec<Sheet>,
    active_sheet: usize,
    next_sheet_id: u64,
}

impl Workbook {
    pub fn new() -> Self {
        Self {
            sheets: vec![Sheet::with_stable_id(1, "Sheet1")],
            active_sheet: 0,
            next_sheet_id: 2,
        }
    }

    pub fn sheets(&self) -> &[Sheet] {
        &self.sheets
    }

    pub fn sheet(&self, index: usize) -> Option<&Sheet> {
        self.sheets.get(index)
    }

    pub fn sheet_mut(&mut self, index: usize) -> Option<&mut Sheet> {
        self.sheets.get_mut(index)
    }

    pub fn sheet_index_by_name(&self, name: &str) -> Option<usize> {
        let normalized = normalize_sheet_name(name);
        self.sheets
            .iter()
            .position(|sheet| normalize_sheet_name(sheet.name()) == normalized)
    }

    pub fn sheet_by_stable_id(&self, stable_id: u64) -> Option<&Sheet> {
        self.sheets
            .iter()
            .find(|sheet| sheet.stable_id() == stable_id)
    }

    pub fn next_available_sheet_name(&self, preferred: &str) -> String {
        let preferred = preferred.trim();
        let preferred = if preferred.is_empty() {
            "Sheet1"
        } else {
            preferred
        };
        if self.sheet_index_by_name(preferred).is_none() {
            return preferred.to_string();
        }

        let digit_start = preferred
            .char_indices()
            .rev()
            .take_while(|(_, ch)| ch.is_ascii_digit())
            .last()
            .map(|(index, _)| index);
        let (base, mut suffix, separator) = if let Some(index) = digit_start {
            let suffix = preferred[index..].parse::<u64>().unwrap_or(1);
            (&preferred[..index], suffix.saturating_add(1), "")
        } else {
            (preferred, 2, " ")
        };

        loop {
            let candidate = format!("{base}{separator}{suffix}");
            if self.sheet_index_by_name(&candidate).is_none() {
                return candidate;
            }
            suffix = suffix.saturating_add(1);
        }
    }

    pub fn active_sheet(&self) -> usize {
        self.active_sheet
    }

    pub fn set_active_sheet(&mut self, index: usize) {
        if index < self.sheets.len() {
            self.active_sheet = index;
        }
    }

    pub fn add_sheet(&mut self, name: impl Into<String>) -> Result<usize, CoreError> {
        let name = name.into();
        self.validate_new_sheet_name(&name, None)?;
        let stable_id = self.allocate_sheet_id()?;
        let sheet = Sheet::with_stable_id(stable_id, name);
        self.sheets.push(sheet);
        Ok(self.sheets.len() - 1)
    }

    pub fn insert_sheet(&mut self, index: usize, name: impl Into<String>) -> Result<(), CoreError> {
        if index > self.sheets.len() {
            return Err(CoreError::SheetOutOfBounds(index));
        }
        let name = name.into();
        self.validate_new_sheet_name(&name, None)?;
        let stable_id = self.allocate_sheet_id()?;
        self.sheets
            .insert(index, Sheet::with_stable_id(stable_id, name));
        Ok(())
    }

    pub fn insert_existing_sheet(&mut self, index: usize, sheet: Sheet) -> Result<(), CoreError> {
        if index > self.sheets.len() {
            return Err(CoreError::SheetOutOfBounds(index));
        }
        self.validate_new_sheet_name(sheet.name(), None)?;
        if sheet.stable_id() == 0
            || self
                .sheets
                .iter()
                .any(|candidate| candidate.stable_id() == sheet.stable_id())
        {
            return Err(CoreError::InvalidStableSheetIds);
        }
        self.next_sheet_id = self.next_sheet_id.max(sheet.stable_id().saturating_add(1));
        self.sheets.insert(index, sheet);
        Ok(())
    }

    pub fn set_sheet_stable_id(&mut self, index: usize, stable_id: u64) -> bool {
        if stable_id == 0
            || self
                .sheets
                .iter()
                .enumerate()
                .any(|(candidate, sheet)| candidate != index && sheet.stable_id() == stable_id)
        {
            return false;
        }
        let Some(sheet) = self.sheets.get_mut(index) else {
            return false;
        };
        sheet.set_stable_id(stable_id);
        self.next_sheet_id = self.next_sheet_id.max(stable_id.saturating_add(1));
        true
    }

    pub fn replace_sheet_stable_ids(&mut self, stable_ids: &[u64]) -> Result<(), CoreError> {
        if stable_ids.len() != self.sheets.len()
            || stable_ids.contains(&0)
            || stable_ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != stable_ids.len()
        {
            return Err(CoreError::InvalidStableSheetIds);
        }
        for (sheet, stable_id) in self.sheets.iter_mut().zip(stable_ids.iter().copied()) {
            sheet.set_stable_id(stable_id);
        }
        self.next_sheet_id = stable_ids
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        Ok(())
    }

    pub fn delete_sheet(&mut self, index: usize) -> bool {
        if self.sheets.len() <= 1 {
            return false;
        }
        if index >= self.sheets.len() {
            return false;
        }
        let deleted_name = self.sheets[index].name().to_string();
        for (candidate, sheet) in self.sheets.iter_mut().enumerate() {
            if candidate != index {
                sheet.rewrite_sheet_references(&deleted_name, None);
            }
        }
        self.sheets.remove(index);
        if self.active_sheet >= self.sheets.len() {
            self.active_sheet = self.sheets.len() - 1;
        }
        true
    }

    pub fn rename_sheet(&mut self, index: usize, name: impl Into<String>) -> Result<(), CoreError> {
        if index >= self.sheets.len() {
            return Err(CoreError::SheetOutOfBounds(index));
        }
        let name = name.into();
        self.validate_new_sheet_name(&name, Some(index))?;
        let old_name = self.sheets[index].name().to_string();
        for sheet in &mut self.sheets {
            sheet.rewrite_sheet_references(&old_name, Some(&name));
        }
        self.sheets[index].rename(name);
        Ok(())
    }

    pub fn apply_sheet_structure_edit(&mut self, index: usize, edit: StructureEdit) -> bool {
        let Some(target) = self.sheets.get(index) else {
            return false;
        };
        let target_name = target.name().to_string();
        for (candidate, sheet) in self.sheets.iter_mut().enumerate() {
            if candidate == index {
                sheet.apply_structure_edit_scoped(edit, true, Some(&target_name));
            } else {
                sheet.rewrite_structure_references(edit, &target_name);
            }
        }
        true
    }

    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    fn validate_new_sheet_name(
        &self,
        name: &str,
        excluding: Option<usize>,
    ) -> Result<(), CoreError> {
        let normalized = normalize_sheet_name(name);
        if normalized.is_empty() {
            return Err(CoreError::InvalidSheetName);
        }
        if self.sheets.iter().enumerate().any(|(index, sheet)| {
            Some(index) != excluding && normalize_sheet_name(sheet.name()) == normalized
        }) {
            return Err(CoreError::DuplicateSheetName(name.to_string()));
        }
        Ok(())
    }

    fn allocate_sheet_id(&mut self) -> Result<u64, CoreError> {
        let start = self.next_sheet_id.max(1);
        let mut candidate = start;
        loop {
            if self
                .sheets
                .iter()
                .all(|sheet| sheet.stable_id() != candidate)
            {
                self.next_sheet_id = candidate.checked_add(1).unwrap_or(1);
                return Ok(candidate);
            }
            candidate = candidate.checked_add(1).unwrap_or(1);
            if candidate == start {
                return Err(CoreError::InvalidStableSheetIds);
            }
        }
    }
}

fn normalize_sheet_name(name: &str) -> String {
    name.trim().to_lowercase()
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_workbook() {
        let wb = Workbook::new();
        assert_eq!(wb.sheet_count(), 1);
        assert_eq!(wb.sheets()[0].name(), "Sheet1");
    }

    #[test]
    fn test_add_sheet() {
        let mut wb = Workbook::new();
        let idx = wb.add_sheet("Sheet2").unwrap();
        assert_eq!(idx, 1);
        assert_eq!(wb.sheet_count(), 2);
        assert_eq!(wb.sheets()[1].name(), "Sheet2");
    }

    #[test]
    fn test_delete_sheet() {
        let mut wb = Workbook::new();
        wb.add_sheet("Sheet2").unwrap();
        assert_eq!(wb.sheet_count(), 2);
        assert!(wb.delete_sheet(1));
        assert_eq!(wb.sheet_count(), 1);
    }

    #[test]
    fn test_cannot_delete_last_sheet() {
        let mut wb = Workbook::new();
        assert!(!wb.delete_sheet(0));
    }

    #[test]
    fn test_rename_sheet() {
        let mut wb = Workbook::new();
        wb.rename_sheet(0, "Data").unwrap();
        assert_eq!(wb.sheets()[0].name(), "Data");
    }

    #[test]
    fn sheet_names_are_case_insensitively_unique_and_failed_rename_is_non_mutating() {
        let mut workbook = Workbook::new();
        let data = workbook.add_sheet("Data").unwrap();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "=Data!A1".into());

        assert!(matches!(
            workbook.add_sheet(" data "),
            Err(CoreError::DuplicateSheetName(_))
        ));
        assert!(matches!(
            workbook.rename_sheet(data, "sheet1"),
            Err(CoreError::DuplicateSheetName(_))
        ));
        assert_eq!(workbook.sheet(data).unwrap().name(), "Data");
        assert_eq!(
            workbook.sheet(0).unwrap().cell_value(0, 0),
            Some("=Data!A1".into())
        );
        assert!(matches!(
            workbook.insert_sheet(1, "DATA"),
            Err(CoreError::DuplicateSheetName(_))
        ));
        workbook.add_sheet("ÅRS").unwrap();
        assert!(matches!(
            workbook.add_sheet("års"),
            Err(CoreError::DuplicateSheetName(_))
        ));
    }

    #[test]
    fn stable_sheet_ids_cannot_collide() {
        let mut workbook = Workbook::new();
        workbook.add_sheet("Second").unwrap();
        let first_id = workbook.sheet(0).unwrap().stable_id();
        assert!(!workbook.set_sheet_stable_id(1, first_id));
        assert!(workbook.replace_sheet_stable_ids(&[10, 10]).is_err());
        assert_eq!(workbook.sheet(0).unwrap().stable_id(), first_id);
        assert_ne!(workbook.sheet(1).unwrap().stable_id(), first_id);
    }

    #[test]
    fn next_available_sheet_name_is_deterministic_for_numbered_and_named_bases() {
        let mut workbook = Workbook::new();
        workbook.add_sheet("Sheet2").unwrap();
        workbook.add_sheet("Sheet4").unwrap();
        assert_eq!(workbook.next_available_sheet_name("Sheet2"), "Sheet3");
        assert_eq!(workbook.next_available_sheet_name("Sheet4"), "Sheet5");

        workbook.add_sheet("Pivot (Data)").unwrap();
        workbook.add_sheet("Pivot (Data) 2").unwrap();
        assert_eq!(
            workbook.next_available_sheet_name("Pivot (Data)"),
            "Pivot (Data) 3"
        );
    }

    #[test]
    fn sheet_stable_ids_survive_index_changes() {
        let mut workbook = Workbook::new();
        let first_id = workbook.sheet(0).unwrap().stable_id();
        let second = workbook.add_sheet("Second").unwrap();
        let second_id = workbook.sheet(second).unwrap().stable_id();
        workbook.delete_sheet(0);
        assert_ne!(first_id, second_id);
        assert_eq!(workbook.sheet(0).unwrap().stable_id(), second_id);
    }

    #[test]
    fn rename_rewrites_qualified_formula_references_and_quotes_new_name() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Data").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook
            .sheet_mut(report)
            .unwrap()
            .set_cell_value(0, 0, "=SUM(Data!A1:A2)".into());

        workbook.rename_sheet(0, "Annual Budget").unwrap();
        assert_eq!(
            workbook.sheet(report).unwrap().cell_value(0, 0),
            Some("=SUM('Annual Budget'!A1:A2)".into())
        );
    }

    #[test]
    fn delete_rewrites_qualified_formula_references_to_ref_error() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Data").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook
            .sheet_mut(report)
            .unwrap()
            .set_cell_value(0, 0, "=Data!A1+1".into());

        assert!(workbook.delete_sheet(0));
        assert_eq!(
            workbook.sheet(0).unwrap().cell_value(0, 0),
            Some("=#REF!+1".into())
        );
    }

    #[test]
    fn structure_edit_rewrites_only_references_to_edited_sheet() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Data").unwrap();
        let other = workbook.add_sheet("Other").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook.sheet_mut(report).unwrap().set_cell_value(
            0,
            0,
            "=SUM(Data!A1:B2)+Other!A1+A1".into(),
        );

        assert!(workbook.apply_sheet_structure_edit(0, StructureEdit::InsertRow(0)));
        assert_eq!(
            workbook.sheet(report).unwrap().cell_value(0, 0),
            Some("=SUM(Data!A2:B3)+Other!A1+A1".into())
        );
        assert_eq!(workbook.sheet(other).unwrap().name(), "Other");
    }

    #[test]
    fn delete_rewrites_explicitly_qualified_range_to_single_ref_error() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Data").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook
            .sheet_mut(report)
            .unwrap()
            .set_cell_value(0, 0, "=SUM(Data!A1:Data!B2)+1".into());

        workbook.delete_sheet(0);
        assert_eq!(
            workbook.sheet(0).unwrap().cell_value(0, 0),
            Some("=SUM(#REF!)+1".into())
        );
    }

    #[test]
    fn rename_is_utf8_safe_and_handles_doubled_apostrophes() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Års Data").unwrap();
        workbook.add_sheet("Sam's Data").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook.sheet_mut(report).unwrap().set_cell_value(
            0,
            0,
            "='Års Data'!A1+'Sam''s Data'!A1+\"Månad\"".into(),
        );

        workbook.rename_sheet(0, "Översikt").unwrap();
        assert_eq!(
            workbook.sheet(report).unwrap().cell_value(0, 0),
            Some("='Översikt'!A1+'Sam''s Data'!A1+\"Månad\"".into())
        );
    }
}
