use crate::sheet::Sheet;

pub struct Workbook {
    sheets: Vec<Sheet>,
    active_sheet: usize,
}

impl Workbook {
    pub fn new() -> Self {
        Self {
            sheets: vec![Sheet::new("Sheet1")],
            active_sheet: 0,
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
        let sheet = Sheet::new(name);
        self.sheets.push(sheet);
        self.sheets.len() - 1
    }

    pub fn insert_sheet(&mut self, index: usize, name: impl Into<String>) {
        if index <= self.sheets.len() {
            self.sheets.insert(index, Sheet::new(name));
        }
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
}
