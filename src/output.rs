use crate::config::OutputFormat;
use colored::Colorize;
use rust_xlsxwriter::{Format, FormatAlign, Workbook};
use serde::Serialize;

#[derive(Serialize, Default)]
pub struct Result {
    pub name: String,
    pub qps: f64,
    #[serde(rename = "avg(ms)")]
    pub avg: f64,
    #[serde(rename = "min(ms)")]
    pub min: f64,
    #[serde(rename = "p50(ms)")]
    pub p50: f64,
    #[serde(rename = "p90(ms)")]
    pub p90: f64,
    #[serde(rename = "p99(ms)")]
    pub p99: f64,
    #[serde(rename = "max(ms)")]
    pub max: f64,
    #[serde(rename = "memory(GiB)")]
    pub memory: f64,
    // test arguments
    pub connections: u64,
    pub pipeline: u64,
    pub count: u64,
    #[serde(rename = "duration(s)")]
    pub duration: f64,
}

pub(crate) struct Results {
    results: Vec<Result>,
}

impl Results {
    pub fn new() -> Results {
        Results { results: Vec::new() }
    }

    pub fn add(&mut self, result: Result) {
        self.results.push(result);
    }
    pub fn save(&self, output_formats: Vec<OutputFormat>) {
        let filename = std::format!("{}", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
        for output_format in output_formats {
            match output_format {
                OutputFormat::XLSX => {
                    self.save_xlsx(filename.clone());
                }
                OutputFormat::JSON => {
                    self.save_json("output".to_string());
                }
            }
        }
    }

    fn save_json(&self, filename: String) {
        let filename = std::format!("{}.json", filename);
        let json = serde_json::to_string_pretty(&self.results).unwrap();
        std::fs::write(&filename, json).unwrap();
        println!("{} {}", "Saved to".bold().yellow(), filename);
    }

    fn save_xlsx(&self, filename: String) {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        assert!(self.results.len() > 0);
        let header_format = Format::new().set_bold().set_align(FormatAlign::Center);
        worksheet.serialize_headers_with_format(0, 0, &self.results[0], &header_format).unwrap();
        for result in &self.results {
            worksheet.serialize(result).unwrap();
        }
        let float_format = Format::new().set_num_format("0.00").set_align(FormatAlign::Center);
        let int_format = Format::new().set_num_format("0").set_align(FormatAlign::Center);
        // command
        worksheet.set_column_width(0, 50).unwrap();
        // qps
        worksheet.set_column_width(1, 15).unwrap();
        worksheet.set_column_format(1, &int_format).unwrap();
        // avg
        worksheet.set_column_width(2, 10).unwrap();
        worksheet.set_column_format(2, &float_format).unwrap();
        // min
        worksheet.set_column_width(3, 10).unwrap();
        worksheet.set_column_format(3, &float_format).unwrap();
        // p50
        worksheet.set_column_width(4, 10).unwrap();
        worksheet.set_column_format(4, &float_format).unwrap();
        // p90
        worksheet.set_column_width(5, 10).unwrap();
        worksheet.set_column_format(5, &float_format).unwrap();
        // p99
        worksheet.set_column_width(6, 10).unwrap();
        worksheet.set_column_format(6, &float_format).unwrap();
        // max
        worksheet.set_column_width(7, 10).unwrap();
        worksheet.set_column_format(7, &float_format).unwrap();
        // memory
        worksheet.set_column_width(8, 15).unwrap();
        worksheet.set_column_format(8, &float_format).unwrap();
        // connections
        worksheet.set_column_width(9, 15).unwrap();
        worksheet.set_column_format(9, &int_format).unwrap();
        // pipeline
        worksheet.set_column_width(10, 15).unwrap();
        worksheet.set_column_format(10, &int_format).unwrap();
        // count
        worksheet.set_column_width(11, 15).unwrap();
        worksheet.set_column_format(11, &int_format).unwrap();
        // duration
        worksheet.set_column_width(12, 15).unwrap();
        worksheet.set_column_format(12, &float_format).unwrap();

        let filename = std::format!("{}.xlsx", filename);
        workbook.save(&filename).unwrap();
        println!("{} {}", "Saved to".bold().yellow(), filename);
    }
}
