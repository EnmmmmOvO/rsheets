use rsheet_lib::cell_value::CellValue;

#[derive(Debug, Clone)]
pub struct Cell {
    pub value: CellValue,
    pub formula: String,
    pub dependencies: Vec<(u32, u32)>,
}

impl Cell {
    pub fn new() -> Cell {
        Cell {
            value: CellValue::None,
            formula: String::new(),
            dependencies: Vec::new(),
        }
    }
}
