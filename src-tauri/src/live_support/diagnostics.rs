use std::path::PathBuf;

pub struct Diagnostics {
    pub stage: &'static str,
    pub output_directory: Option<PathBuf>,
}

impl Diagnostics {
    pub fn record_failure(&self, error: &(dyn std::error::Error + 'static)) -> bool {
        let Some(output) = &self.output_directory else {
            return false;
        };
        // Only static categories and stage instructions are retained; error text may contain private data.
        let category = if error.is::<std::io::Error>() {
            "local_io"
        } else if error.is::<serde_json::Error>() {
            "manifest_json"
        } else {
            "validation_or_service"
        };
        let report = serde_json::json!({"result":"FAIL", "stage":self.stage, "category":category,
            "nextAction":"Follow the stage instruction; inspect the dedicated local job if a transfer started. Do not reset or rerun blindly."});
        std::fs::write(output.join("failure.json"), report.to_string()).is_ok()
    }
}
