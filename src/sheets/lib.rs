use rsheet_lib::cell_value::CellValue;
use crate::sheets::sheets::Sheet;

fn expand_sheet(sheet: &mut Sheet, row: u32, col: u32) {

}

pub fn set_cell_value(sheet: &mut Sheet, row: u32, col: u32, value: CellValue) {

}

pub fn get_cell_value(sheet: &Sheet, row: u32, col: u32) -> CellValue {
    CellValue::String("".to_string())
}