// compile-valid: true
// 测试关联函数消歧：同模块中两个类型各有同名 build() 方法，
// 按 type_name 过滤 impl_target 后应各自解析到正确的关联函数

pub struct DataProcessor {
    name: String,
}

impl DataProcessor {
    pub fn build(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    pub fn process(&self) -> String {
        format!("processed: {}", self.name)
    }
}

pub struct RequestHandler {
    path: String,
}

impl RequestHandler {
    pub fn build(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    pub fn handle(&self) -> String {
        format!("handled: {}", self.path)
    }
}

pub fn run() -> String {
    let dp = DataProcessor::build("data");
    let rh = RequestHandler::build("/api");
    format!("{} | {}", dp.process(), rh.handle())
}
