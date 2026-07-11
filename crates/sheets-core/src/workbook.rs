use crate::sheet::Sheet;

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

    pub fn active_sheet(&self) -> usize {
        self.active_sheet
    }

    pub fn set_active_sheet(&mut self, index: usize) {
        if index < self.sheets.len() {
            self.active_sheet = index;
        }
    }

    pub fn add_sheet(&mut self, name: impl Into<String>) -> usize {
        let name = name.into();
        let sheet = Sheet::with_stable_id(self.next_sheet_id, name);
        self.next_sheet_id = self.next_sheet_id.saturating_add(1);
        self.sheets.push(sheet);
        self.sheets.len() - 1
    }

    pub fn insert_sheet(&mut self, index: usize, name: impl Into<String>) {
        if index <= self.sheets.len() {
            self.sheets
                .insert(index, Sheet::with_stable_id(self.next_sheet_id, name));
            self.next_sheet_id = self.next_sheet_id.saturating_add(1);
        }
    }

    pub fn set_sheet_stable_id(&mut self, index: usize, stable_id: u64) -> bool {
        let Some(sheet) = self.sheets.get_mut(index) else {
            return false;
        };
        sheet.set_stable_id(stable_id);
        self.next_sheet_id = self.next_sheet_id.max(stable_id.saturating_add(1));
        true
    }

    pub fn delete_sheet(&mut self, index: usize) -> bool {
        if self.sheets.len() <= 1 {
            return false;
        }
        if index >= self.sheets.len() {
            return false;
        }
        self.sheets.remove(index);
        if self.active_sheet >= self.sheets.len() {
            self.active_sheet = self.sheets.len() - 1;
        }
        true
    }

    pub fn rename_sheet(&mut self, index: usize, name: impl Into<String>) -> bool {
        if let Some(sheet) = self.sheets.get_mut(index) {
            sheet.rename(name);
            true
        } else {
            false
        }
    }

    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }
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
        let idx = wb.add_sheet("Sheet2");
        assert_eq!(idx, 1);
        assert_eq!(wb.sheet_count(), 2);
        assert_eq!(wb.sheets()[1].name(), "Sheet2");
    }

    #[test]
    fn test_delete_sheet() {
        let mut wb = Workbook::new();
        wb.add_sheet("Sheet2");
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
        assert!(wb.rename_sheet(0, "Data"));
        assert_eq!(wb.sheets()[0].name(), "Data");
    }

    #[test]
    fn sheet_stable_ids_survive_index_changes() {
        let mut workbook = Workbook::new();
        let first_id = workbook.sheet(0).unwrap().stable_id();
        let second = workbook.add_sheet("Second");
        let second_id = workbook.sheet(second).unwrap().stable_id();
        workbook.delete_sheet(0);
        assert_ne!(first_id, second_id);
        assert_eq!(workbook.sheet(0).unwrap().stable_id(), second_id);
    }
}
