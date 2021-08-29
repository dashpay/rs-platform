use wasm_bindgen::prelude::*;

use js_sys;

use dpp::identifier::Identifier;

#[wasm_bindgen]
extern {
    fn alert(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn console_log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);
}

#[wasm_bindgen(js_name = Identifier)]
pub struct IdentifierWrapper {
    wrapped: Identifier
}

impl IdentifierWrapper {
    fn new(buffer: &js_sys::Uint8Array) {}
    fn from() {}
    fn toBuffer(&self) {}
    fn toJSON(&self) {}
    fn toString(&self) {}
    fn encodeCBOR(&self) {}
}