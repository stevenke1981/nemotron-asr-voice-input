fn main() {
    match ferrous_opencc::OpenCC::from_config(ferrous_opencc::config::BuiltinConfig::S2t) {
        Ok(opencc) => {
            let result = opencc.convert("这是测试");
            println!("S2T result: {}", result);
        }
        Err(e) => {
            println!("S2T error: {}", e);
        }
    }
    match ferrous_opencc::OpenCC::from_config(ferrous_opencc::config::BuiltinConfig::T2s) {
        Ok(opencc) => {
            let result = opencc.convert("這是測試");
            println!("T2S result: {}", result);
        }
        Err(e) => {
            println!("T2S error: {}", e);
        }
    }
}
