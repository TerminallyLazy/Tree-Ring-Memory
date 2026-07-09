use serde_json::json;

use crate::actions::export_import::{import_json_payload, ExportActionReport, ImportActionReport};
use crate::actions::recall::RecallReport;

pub fn print_recall_report(report: RecallReport, json_output: bool) -> Result<(), String> {
    if json_output {
        let payload: Vec<_> = report
            .results
            .into_iter()
            .map(|result| {
                json!({
                    "memory": result.memory,
                    "score": result.score,
                    "ranking": result.ranking,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string(&payload).map_err(|err| err.to_string())?
        );
    } else {
        for result in report.results {
            println!(
                "{} [{}] {} score={:.3}",
                result.memory.id, result.memory.ring, result.memory.summary, result.score
            );
        }
    }
    Ok(())
}

pub fn print_export_report(report: ExportActionReport, json_output: bool) -> Result<(), String> {
    if let Some(jsonl) = report.jsonl {
        print!("{jsonl}");
        return Ok(());
    }
    let Some(output) = report.output else {
        return Err("export action did not return output path or JSONL".to_string());
    };
    if json_output {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "ok": true,
                "path": output,
                "memory_count": report.report.memory_count,
                "sensitive_included": report.report.sensitive_included,
                "superseded_included": report.report.superseded_included,
            }))
            .map_err(|err| err.to_string())?
        );
    } else {
        println!(
            "Tree Ring Memory export complete: {} memories -> {}",
            report.report.memory_count,
            output.display()
        );
    }
    Ok(())
}

pub fn print_import_report(report: ImportActionReport, json_output: bool) -> Result<(), String> {
    if json_output {
        println!("{}", import_json_payload(&report));
    } else {
        println!(
            "Tree Ring Memory import complete: valid={} inserted={} replaced={} skipped_duplicates={} dry_run={}",
            report.report.valid_count,
            report.report.inserted_count,
            report.report.replaced_count,
            report.report.skipped_duplicate_count,
            report.report.dry_run
        );
    }
    Ok(())
}
