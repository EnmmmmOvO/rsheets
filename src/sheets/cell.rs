use rsheet_lib::cell_value::CellValue;
use rsheet_lib::command_runner::CommandRunner;

pub struct Cell {
    value: CellValue,
    formula: String,
    dependencies: Vec<(u32, u32)>,
}

impl Cell {
    pub fn new(value: CellValue, formula: String) -> Self {
        Self {
            value,
            formula,
            dependencies: Vec::new(),
        }
    }

    pub fn new_blank() -> Self {
        Self {
            value: CellValue::None,
            formula: String::new(),
            dependencies: Vec::new(),
        }
    }


}


