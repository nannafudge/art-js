use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn _info(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    pub fn _error(s: &str);
}

#[macro_export]
macro_rules! info {
    ($msg : expr) => {
        _info($msg);
    };
    ($($arg : tt)*) => {
        _info(&format_args!($($arg)*).to_string());
    };
}

#[macro_export]
macro_rules! error {
    ($msg : expr) => {
        _error($msg);
    };
    ($($arg: tt)*) => {
        _error(&format_args!($($arg)*).to_string());
    };
}